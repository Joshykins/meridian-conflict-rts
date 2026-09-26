//! A single-producer, single-consumer ring of stereo frames: the music's render
//! thread writes ahead into it, the audio callback reads out of it. Lock-free and
//! allocation-free on both sides after `new`: samples are f32 bits in atomics, and
//! the two positions are ever-growing frame counters (wrapping), so the fill is
//! `write - read`.

use std::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

pub struct Ring {
    /// Interleaved left/right, `capacity` frames.
    buf: Box<[AtomicU32]>,
    mask: usize,
    write: AtomicUsize,
    read: AtomicUsize,
}

impl Ring {
    /// Holds `frames` rounded up to a power of two.
    pub fn new(frames: usize) -> Ring {
        let cap = frames.max(2).next_power_of_two();
        Ring {
            buf: (0..cap * 2).map(|_| AtomicU32::new(0)).collect(),
            mask: cap - 1,
            write: AtomicUsize::new(0),
            read: AtomicUsize::new(0),
        }
    }

    pub fn capacity(&self) -> usize {
        self.mask + 1
    }

    /// Frames written and not yet read.
    pub fn len(&self) -> usize {
        self.write
            .load(Ordering::Acquire)
            .wrapping_sub(self.read.load(Ordering::Acquire))
    }

    /// Producer: appends as much of interleaved stereo `frames` as fits; returns frames taken.
    pub fn push(&self, frames: &[f32]) -> usize {
        let w = self.write.load(Ordering::Relaxed);
        let r = self.read.load(Ordering::Acquire);
        let free = self.capacity() - w.wrapping_sub(r);
        let n = (frames.len() / 2).min(free);
        for i in 0..n {
            let at = (w.wrapping_add(i) & self.mask) * 2;
            self.buf[at].store(frames[i * 2].to_bits(), Ordering::Relaxed);
            self.buf[at + 1].store(frames[i * 2 + 1].to_bits(), Ordering::Relaxed);
        }
        self.write.store(w.wrapping_add(n), Ordering::Release);
        n
    }

    /// Consumer: hands up to `max` frames, oldest first, to `f(index, left, right)`;
    /// returns how many there were.
    pub fn pop(&self, max: usize, mut f: impl FnMut(usize, f32, f32)) -> usize {
        let r = self.read.load(Ordering::Relaxed);
        let w = self.write.load(Ordering::Acquire);
        let n = w.wrapping_sub(r).min(max);
        for i in 0..n {
            let at = (r.wrapping_add(i) & self.mask) * 2;
            f(
                i,
                f32::from_bits(self.buf[at].load(Ordering::Relaxed)),
                f32::from_bits(self.buf[at + 1].load(Ordering::Relaxed)),
            );
        }
        self.read.store(r.wrapping_add(n), Ordering::Release);
        n
    }
}
