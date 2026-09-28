//! What one renderer build computes on the CPU that the next build in the same
//! process can take as it is: texture fields, mip chains, a map's ground cover.
//! Off unless [`keep_between_builds`] is called, which only the shot server does
//! (it rebuilds its renderer after a shader or data edit); the game builds a
//! renderer rarely and would only hold the memory for nothing.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

static ON: AtomicBool = AtomicBool::new(false);

/// From now on, what renderer builds compute is kept for the next one.
pub fn keep_between_builds() {
    ON.store(true, Ordering::Relaxed);
}

/// Values made by one function, by key.
pub(crate) type Kept<T> = Mutex<Vec<(u64, T)>>;

/// `make()`, or a copy of what it made for `key` before, when keeping is on.
pub(crate) fn kept<T: Clone>(slot: &Kept<T>, key: u64, make: impl FnOnce() -> T) -> T {
    if !ON.load(Ordering::Relaxed) {
        return make();
    }
    if let Some((_, v)) = slot
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .iter()
        .find(|(k, _)| *k == key)
    {
        return v.clone();
    }
    let v = make();
    slot.lock()
        .unwrap_or_else(|p| p.into_inner())
        .push((key, v.clone()));
    v
}

/// A key for bytes: FNV-1a over the whole of them, then their length.
pub(crate) fn key_of(bytes: &[u8], salt: u64) -> u64 {
    let mut h = 0xcbf2_9ce4_8422_2325u64 ^ salt;
    for chunk in bytes.as_chunks::<8>().0 {
        h = (h ^ u64::from_le_bytes(*chunk)).wrapping_mul(0x100_0000_01b3);
    }
    for &b in bytes.as_chunks::<8>().1 {
        h = (h ^ b as u64).wrapping_mul(0x100_0000_01b3);
    }
    h ^ bytes.len() as u64
}
