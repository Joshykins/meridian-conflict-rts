//! Native faults: an access violation or an illegal instruction, usually in a
//! graphics driver, which no panic hook sees and which otherwise closes the game
//! without a word. The unhandled-exception filter writes `crash-<secs>.log` and a
//! minidump (`crash-<secs>.dmp`, every thread's stack; open it in Visual Studio
//! or WinDbg beside the build's `.pdb`), shows the crash window, and lets Windows
//! end the process as it would have.

use std::ffi::c_void;
use std::os::windows::io::AsRawHandle;

#[repr(C)]
struct ExceptionRecord {
    code: u32,
    flags: u32,
    record: *const ExceptionRecord,
    address: *const c_void,
    parameter_count: u32,
    information: [usize; 15],
}

#[repr(C)]
struct ExceptionPointers {
    record: *const ExceptionRecord,
    context: *const c_void,
}

/// `MINIDUMP_EXCEPTION_INFORMATION`, which dbghelp.h packs to 4 bytes.
#[repr(C, packed(4))]
struct DumpException {
    thread_id: u32,
    pointers: *const ExceptionPointers,
    client_pointers: i32,
}

type Filter = unsafe extern "system" fn(*const ExceptionPointers) -> i32;
type WriteDump = unsafe extern "system" fn(
    process: isize,
    process_id: u32,
    file: isize,
    kind: u32,
    exception: *const DumpException,
    user: *const c_void,
    callback: *const c_void,
) -> i32;

#[link(name = "kernel32")]
extern "system" {
    fn SetUnhandledExceptionFilter(filter: Option<Filter>) -> Option<Filter>;
    fn LoadLibraryW(name: *const u16) -> isize;
    fn GetProcAddress(module: isize, name: *const u8) -> *const c_void;
    fn GetCurrentProcess() -> isize;
    fn GetCurrentProcessId() -> u32;
    fn GetCurrentThreadId() -> u32;
}

/// Lets Windows go on to its own handling (Windows Error Reporting), which ends
/// the process.
const CONTINUE_SEARCH: i32 = 0;

pub(super) fn install() {
    // SAFETY: `filter` has the signature Windows calls it with, for the life of
    // the process.
    unsafe { SetUnhandledExceptionFilter(Some(filter)) };
}

/// The exception a `--crash-test native` drill raises.
pub(super) fn raise_access_violation() {
    #[link(name = "kernel32")]
    extern "system" {
        fn RaiseException(code: u32, flags: u32, count: u32, arguments: *const usize);
    }
    // SAFETY: no arguments are passed; the exception is not continuable and goes
    // to the unhandled-exception filter like a real fault.
    unsafe { RaiseException(ACCESS_VIOLATION, 1, 0, std::ptr::null()) };
}

const ACCESS_VIOLATION: u32 = 0xC000_0005;

unsafe extern "system" fn filter(pointers: *const ExceptionPointers) -> i32 {
    // Only once: a second fault while reporting the first goes straight on.
    static ENTERED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
    if ENTERED.swap(true, std::sync::atomic::Ordering::SeqCst) {
        return CONTINUE_SEARCH;
    }
    // SAFETY: Windows passes the filter a valid EXCEPTION_POINTERS whose record
    // lives until the filter returns.
    let record = unsafe { &*(*pointers).record };
    let what = describe(record);
    let thread = std::thread::current();
    let mut report = format!(
        "Meridian Conflict {} crashed (native fault)\n{}\nthread: {}\n{what}\n\nstack:\n{}",
        env!("MERIDIAN_BUILD"),
        super::system_line(),
        thread.name().unwrap_or("<unnamed>"),
        super::stack::here(),
    );
    let dump = write_dump(pointers);
    if let Some(dump) = &dump {
        report.push_str(&format!("\nminidump: {}\n", dump.display()));
    }
    report.push_str(&super::session::kept_lines());
    let path = super::save("crash", "log", report.as_bytes());
    if let Some(path) = &path {
        super::session::note(&format!("crash report written to {}", path.display()));
    }
    if super::claim_window() {
        super::dialog::show(&super::dialog::Shown {
            heading: "Meridian Conflict crashed",
            content: &format!(
                "The game hit a fault it could not recover from and has to close.\n\n{what}"
            ),
            hint: driver_hint(&what),
            report: &report,
            path: path.as_deref(),
        });
    }
    CONTINUE_SEARCH
}

/// What faulted where, in a line or two.
fn describe(record: &ExceptionRecord) -> String {
    let at = super::stack::locate(record.address);
    let name = match record.code {
        ACCESS_VIOLATION if record.parameter_count >= 2 => {
            let verb = match record.information[0] {
                0 => "reading",
                1 => "writing",
                _ => "running",
            };
            return format!(
                "Access violation {verb} {:#x}, in {at}",
                record.information[1]
            );
        }
        ACCESS_VIOLATION => "Access violation",
        0xC000_001D => "Illegal instruction",
        0xC000_00FD => "Stack overflow",
        0xC000_0094 => "Integer division by zero",
        0xC000_0409 => "Stack buffer overrun",
        0xC000_0374 => "Heap corruption",
        _ => "Exception",
    };
    format!("{name} ({:#010x}), in {at}", record.code)
}

/// Graphics drivers are where most native faults happen: say so when the fault
/// is in one.
fn driver_hint(what: &str) -> Option<&'static str> {
    const DRIVERS: [&str; 6] = ["nvoglv", "nvwgf", "amdvlk", "amdxx", "igvk", "nvgpucomp"];
    let lower = what.to_ascii_lowercase();
    DRIVERS.iter().any(|d| lower.contains(d)).then_some(
        "The fault is inside the graphics driver. Updating the graphics driver usually \
         fixes this; if not, please send us the details.",
    )
}

/// Writes the minidump through dbghelp.dll, which every Windows has; `None` if it
/// cannot.
fn write_dump(pointers: *const ExceptionPointers) -> Option<std::path::PathBuf> {
    let name: Vec<u16> = "dbghelp.dll\0".encode_utf16().collect();
    // SAFETY: a NUL-terminated UTF-16 name.
    let module = unsafe { LoadLibraryW(name.as_ptr()) };
    if module == 0 {
        return None;
    }
    // SAFETY: a NUL-terminated ASCII name in a loaded module.
    let proc = unsafe { GetProcAddress(module, c"MiniDumpWriteDump".as_ptr().cast()) };
    if proc.is_null() {
        return None;
    }
    // SAFETY: MiniDumpWriteDump has this signature in every dbghelp.dll.
    let write: WriteDump = unsafe { std::mem::transmute::<*const c_void, WriteDump>(proc) };
    let path = super::save("crash", "dmp", &[])?;
    let file = std::fs::OpenOptions::new().write(true).open(&path).ok()?;
    let exception = DumpException {
        // SAFETY: plain query of the calling (faulting) thread's id.
        thread_id: unsafe { GetCurrentThreadId() },
        pointers,
        client_pointers: 0,
    };
    // Normal (every thread's stack) with thread times and unloaded modules: a few MB.
    const KIND: u32 = 0x1000 | 0x20;
    // SAFETY: a process pseudo-handle, its id, an open file handle and exception
    // information that live across the call; no user streams or callback.
    let ok = unsafe {
        write(
            GetCurrentProcess(),
            GetCurrentProcessId(),
            file.as_raw_handle() as isize,
            KIND,
            &exception,
            std::ptr::null(),
            std::ptr::null(),
        )
    };
    (ok != 0).then_some(path)
}
