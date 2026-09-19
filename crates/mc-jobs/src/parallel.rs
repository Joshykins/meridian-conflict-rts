//! Deterministic data parallelism: chunk boundaries come from `len` and `chunk_size` alone.

use std::any::Any;
use std::ops::Range;
use std::panic::{self, AssertUnwindSafe};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::pool::{lock, AbortOnUnwind, Pool, Shared};

/// Number of chunks `parallel_for(len, chunk_size, ..)` runs. The last chunk may be short.
pub fn chunk_count(len: usize, chunk_size: usize) -> usize {
    assert!(chunk_size > 0, "chunk_size must be non-zero");
    len.div_ceil(chunk_size)
}

/// Index range covered by chunk `chunk_index`.
pub fn chunk_range(len: usize, chunk_size: usize, chunk_index: usize) -> Range<usize> {
    let start = chunk_index * chunk_size;
    start..len.min(start.saturating_add(chunk_size))
}

/// One `parallel_for` call in flight. Threads claim chunk indices from `next`; the queue only
/// ever holds the ticket, so a thousand chunks cost one queue operation.
pub(crate) struct ForState {
    /// Points into the owner's stack frame. Only dereferenced by a thread that holds a claimed,
    /// unfinished chunk, see `help`.
    body: *const (dyn Fn(usize) + Sync + 'static),
    chunks: usize,
    next: AtomicUsize,
    /// Chunks not yet finished. The owner does not return before this reaches zero.
    pending: AtomicUsize,
    cancelled: AtomicBool,
    panic: Mutex<Option<Box<dyn Any + Send>>>,
}

// SAFETY: `body` is the only field that is not already Send + Sync. Its pointee is `Sync`, so
// calling it from any thread is fine while it is alive, and `help` upholds the liveness rule.
unsafe impl Send for ForState {}
unsafe impl Sync for ForState {}

impl ForState {
    /// The caller must keep `*body` alive until `is_finished()`.
    pub(crate) fn new(chunks: usize, body: &(dyn Fn(usize) + Sync)) -> ForState {
        let raw: *const (dyn Fn(usize) + Sync + '_) = body;
        // SAFETY: only erases the lifetime of a fat pointer; the result is stored as a raw
        // pointer, which is allowed to dangle. Every dereference is justified in `help`.
        let body = unsafe {
            std::mem::transmute::<*const (dyn Fn(usize) + Sync + '_), *const (dyn Fn(usize) + Sync + 'static)>(raw)
        };
        ForState {
            body,
            chunks,
            next: AtomicUsize::new(0),
            pending: AtomicUsize::new(chunks),
            cancelled: AtomicBool::new(false),
            panic: Mutex::new(None),
        }
    }

    pub(crate) fn has_unclaimed(&self) -> bool {
        self.next.load(Ordering::SeqCst) < self.chunks
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.pending.load(Ordering::SeqCst) == 0
    }

    fn claim(&self) -> Option<usize> {
        self.next
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| (n < self.chunks).then_some(n + 1))
            .ok()
    }

    /// Runs unclaimed chunks until there are none left or `stop()` turns true.
    pub(crate) fn help(&self, shared: &Shared, stop: &dyn Fn() -> bool) {
        while !stop() {
            let Some(chunk) = self.claim() else { break };
            // After a panic the remaining chunks are claimed and skipped so the count drains.
            if !self.cancelled.load(Ordering::SeqCst) {
                // SAFETY: each chunk index is claimed exactly once and `pending` started at
                // `chunks`, so it stays non-zero until the decrement below. The owner keeps
                // `*body` alive while `pending` is non-zero, so the pointee is alive for the
                // whole call. Nothing touches `body` after the decrement.
                let body = unsafe { &*self.body };
                if let Err(payload) = panic::catch_unwind(AssertUnwindSafe(|| body(chunk))) {
                    self.cancelled.store(true, Ordering::SeqCst);
                    lock(&self.panic).get_or_insert(payload);
                }
            }
            if self.pending.fetch_sub(1, Ordering::SeqCst) == 1 {
                shared.notify_waiters();
            }
        }
    }
}

impl Shared {
    fn run_chunks(&self, chunks: usize, body: &(dyn Fn(usize) + Sync)) {
        let state = Arc::new(ForState::new(chunks, body));
        // From here until `pending` is zero other threads may call `body`; nothing may unwind
        // out of this frame in between. Panics in `body` are caught inside `help`.
        let guard = AbortOnUnwind;
        self.publish_ticket(state.clone(), chunks);
        state.help(self, &|| false);
        self.retire_ticket(&state);
        self.help_until(None, &|| state.is_finished());
        guard.disarm();
        let payload = lock(&state.panic).take();
        if let Some(payload) = payload {
            panic::resume_unwind(payload);
        }
    }
}

impl Pool {
    /// Calls `body(chunk_index, index_range)` once for every chunk of `0..len`, on this thread
    /// and any idle workers. Safe to call from inside jobs, chunks and background tasks.
    ///
    /// If a chunk panics, chunks that have not started are skipped and the first panic is
    /// rethrown here after all running chunks have finished.
    pub fn parallel_for<F>(&self, len: usize, chunk_size: usize, body: F)
    where
        F: Fn(usize, Range<usize>) + Sync,
    {
        let chunks = chunk_count(len, chunk_size);
        let run = |chunk: usize| body(chunk, chunk_range(len, chunk_size, chunk));
        if chunks <= 1 || self.shared.threads == 0 {
            (0..chunks).for_each(run);
        } else {
            self.shared.run_chunks(chunks, &run);
        }
    }

    /// `parallel_for` where every chunk returns a value; the values come back in chunk order.
    /// This is the shape of all parallel sim work: per-chunk outputs, merged in chunk order.
    pub fn parallel_map_chunks<T, F>(&self, len: usize, chunk_size: usize, body: F) -> Vec<T>
    where
        T: Send,
        F: Fn(usize, Range<usize>) -> T + Sync,
    {
        // One uncontended lock per chunk is noise next to a chunk's work and needs no unsafe.
        let slots: Vec<Mutex<Option<T>>> = (0..chunk_count(len, chunk_size)).map(|_| Mutex::new(None)).collect();
        self.parallel_for(len, chunk_size, |chunk, range| {
            let value = body(chunk, range);
            *lock(&slots[chunk]) = Some(value);
        });
        slots
            .into_iter()
            .map(|slot| slot.into_inner().unwrap_or_else(PoisonError::into_inner).expect("every chunk ran"))
            .collect()
    }

    /// Splits `data` into `chunk_size` pieces and calls `body(chunk_index, piece)` for each.
    /// Piece `i` starts at element `i * chunk_size`.
    pub fn parallel_chunks_mut<T, F>(&self, data: &mut [T], chunk_size: usize, body: F)
    where
        T: Send,
        F: Fn(usize, &mut [T]) + Sync,
    {
        assert!(chunk_size > 0, "chunk_size must be non-zero");
        let pieces: Vec<Mutex<&mut [T]>> = data.chunks_mut(chunk_size).map(Mutex::new).collect();
        self.parallel_for(pieces.len(), 1, |chunk, _| body(chunk, &mut lock(&pieces[chunk])));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicU32;

    const THREAD_COUNTS: [usize; 5] = [0, 1, 2, 8, 16];

    #[test]
    fn chunk_math() {
        assert_eq!(chunk_count(0, 4), 0);
        assert_eq!(chunk_count(1, 4), 1);
        assert_eq!(chunk_count(8, 4), 2);
        assert_eq!(chunk_count(9, 4), 3);
        assert_eq!(chunk_range(9, 4, 0), 0..4);
        assert_eq!(chunk_range(9, 4, 2), 8..9);
        assert_eq!(chunk_range(3, usize::MAX, 0), 0..3);
    }

    #[test]
    #[should_panic(expected = "chunk_size must be non-zero")]
    fn zero_chunk_size_is_rejected() {
        Pool::new(0).parallel_for(10, 0, |_, _| {});
    }

    #[test]
    fn every_index_exactly_once() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            for len in [0, 1, 2, 3, 7, 63, 64, 65, 100, 1000, 1023, 4097] {
                for chunk_size in [1, 2, 3, 7, 64, 1000, 5000] {
                    let hits: Vec<AtomicU32> = (0..len).map(|_| AtomicU32::new(0)).collect();
                    let chunks_seen = AtomicUsize::new(0);
                    pool.parallel_for(len, chunk_size, |chunk, range| {
                        assert_eq!(range, chunk_range(len, chunk_size, chunk));
                        assert!(!range.is_empty());
                        chunks_seen.fetch_add(1, Ordering::Relaxed);
                        for i in range {
                            hits[i].fetch_add(1, Ordering::Relaxed);
                        }
                    });
                    assert_eq!(chunks_seen.into_inner(), chunk_count(len, chunk_size));
                    assert!(hits.iter().all(|h| h.load(Ordering::Relaxed) == 1), "len {len} chunk {chunk_size}");
                }
            }
        }
    }

    /// Order-sensitive fold, so a merge in any order but chunk order changes the result.
    fn fold(acc: u64, v: u64) -> u64 {
        (acc ^ v).wrapping_mul(0x9E37_79B9_7F4A_7C15).rotate_left(23)
    }

    fn chunked_digest(pool: &Pool, data: &[u64]) -> u64 {
        let parts = pool.parallel_map_chunks(data.len(), 97, |chunk, range| {
            data[range].iter().fold(chunk as u64, |acc, &v| fold(acc, v))
        });
        parts.into_iter().fold(0, fold)
    }

    #[test]
    fn results_do_not_depend_on_thread_count() {
        let data: Vec<u64> = (0..10_007u64).map(|i| i.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 7).collect();
        let expected = chunked_digest(&Pool::new(0), &data);
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            for _ in 0..20 {
                assert_eq!(chunked_digest(&pool, &data), expected, "{threads} threads");
            }
        }
    }

    #[test]
    fn map_chunks_is_in_chunk_order() {
        let pool = Pool::new(8);
        let out = pool.parallel_map_chunks(1001, 10, |chunk, range| (chunk, range.start, range.end));
        assert_eq!(out.len(), 101);
        for (i, &(chunk, start, end)) in out.iter().enumerate() {
            assert_eq!((chunk, start, end), (i, i * 10, (i * 10 + 10).min(1001)));
        }
        assert!(pool.parallel_map_chunks(0, 10, |_, _| 0u8).is_empty());
    }

    #[test]
    fn chunks_mut_writes_in_place() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let mut data = vec![0usize; 1003];
            pool.parallel_chunks_mut(&mut data, 16, |chunk, piece| {
                for (i, v) in piece.iter_mut().enumerate() {
                    *v = chunk * 16 + i;
                }
            });
            assert!(data.iter().enumerate().all(|(i, &v)| v == i));
        }
    }

    #[test]
    fn nested_parallel_for() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let total = AtomicUsize::new(0);
            pool.parallel_for(32, 1, |_, _| {
                pool.parallel_for(32, 3, |_, inner| {
                    pool.parallel_for(inner.len(), 1, |_, leaf| {
                        total.fetch_add(leaf.len(), Ordering::Relaxed);
                    });
                });
            });
            assert_eq!(total.into_inner(), 32 * 32);
        }
    }

    #[test]
    fn panic_propagates_and_pool_survives() {
        for threads in THREAD_COUNTS {
            let pool = Pool::new(threads);
            let data = vec![1u32; 500];
            let result = panic::catch_unwind(AssertUnwindSafe(|| {
                pool.parallel_for(data.len(), 5, |chunk, range| {
                    if chunk == 37 {
                        panic!("chunk 37");
                    }
                    assert_eq!(data[range].iter().sum::<u32>(), 5);
                });
            }));
            let payload = result.expect_err("panic must reach the caller");
            assert_eq!(payload.downcast_ref::<&str>(), Some(&"chunk 37"));

            let sum = pool.parallel_map_chunks(data.len(), 5, |_, range| data[range].iter().sum::<u32>());
            assert_eq!(sum.iter().sum::<u32>(), 500);
        }
    }
}
