//! The raw stack as `module+offset`, one frame a line. A player's copy of the game
//! has no `.pdb`, so the standard backtrace prints `<unknown>` for every frame;
//! these offsets still name the code, given the build's `.pdb`
//! (`scripts/package-windows.sh` keeps it beside the zip).

use std::ffi::c_void;

#[link(name = "kernel32")]
extern "system" {
    fn RtlCaptureStackBackTrace(
        skip: u32,
        count: u32,
        frames: *mut *mut c_void,
        hash: *mut u32,
    ) -> u16;
    fn GetModuleHandleExW(flags: u32, address: *const c_void, module: *mut isize) -> i32;
    fn GetModuleFileNameW(module: isize, name: *mut u16, size: u32) -> u32;
}

/// The calling thread's stack, from its caller outwards.
pub(super) fn here() -> String {
    let mut frames = [std::ptr::null_mut(); 62];
    // SAFETY: `frames` holds the 62 pointers asked for, and a null hash is allowed.
    let n = unsafe {
        RtlCaptureStackBackTrace(
            1,
            frames.len() as u32,
            frames.as_mut_ptr(),
            std::ptr::null_mut(),
        )
    };
    let mut text = String::new();
    for (i, frame) in frames[..n as usize].iter().enumerate() {
        text.push_str(&format!("  {i:2}: {}\n", locate(*frame)));
    }
    text
}

/// `module.dll+0x1234` for a code address, or the bare address outside any module.
pub(super) fn locate(address: *const c_void) -> String {
    const FROM_ADDRESS: u32 = 0x4;
    const UNCHANGED_REFCOUNT: u32 = 0x2;
    let mut module = 0isize;
    // SAFETY: with FROM_ADDRESS the "name" is an address inside the module, which
    // is only looked up, and UNCHANGED_REFCOUNT leaves nothing to release.
    let found =
        unsafe { GetModuleHandleExW(FROM_ADDRESS | UNCHANGED_REFCOUNT, address, &mut module) != 0 };
    if !found {
        return format!("{address:p}");
    }
    let mut name = [0u16; 260];
    // SAFETY: `name` is a buffer of the length passed, and `module` was just found.
    let len = unsafe { GetModuleFileNameW(module, name.as_mut_ptr(), name.len() as u32) };
    let path = String::from_utf16_lossy(&name[..len as usize]);
    let file = path.rsplit(['\\', '/']).next().unwrap_or(&path);
    format!("{file}+{:#x}", address as usize - module as usize)
}
