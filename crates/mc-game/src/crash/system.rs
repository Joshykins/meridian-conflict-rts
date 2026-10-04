//! The machine a report came from: the system, the processor, memory (the whole
//! machine's and this process's), and the program and how it was started. Read
//! fresh for each report, so the memory figures are the ones at the failure.

use std::fmt::Write;

/// The report's `system:` section, one fact a line.
pub(super) fn describe() -> String {
    let mut text = String::from("system:\n");
    let _ = writeln!(text, "  build: {}{}", env!("MERIDIAN_BUILD"), profile());
    let _ = writeln!(text, "  os: {}", os());
    let threads = std::thread::available_parallelism().map_or(0, |n| n.get());
    let _ = writeln!(text, "  cpu: {}, {threads} threads", cpu());
    let _ = writeln!(text, "  memory: {}", memory());
    let _ = writeln!(text, "  this process: {}", process_memory());
    if let Some(gpu) = mc_render::gpu::device_line() {
        let _ = writeln!(text, "  gpu: {gpu}");
    }
    if let Ok(exe) = std::env::current_exe() {
        let _ = writeln!(text, "  exe: {}", exe.display());
    }
    let args: Vec<String> = std::env::args().skip(1).collect();
    let _ = writeln!(
        text,
        "  args: {}",
        if args.is_empty() {
            "(none)".into()
        } else {
            args.join(" ")
        }
    );
    if let Some(dir) = crate::settings::config_dir() {
        let _ = writeln!(text, "  settings dir: {}", dir.display());
    }
    text
}

fn profile() -> &'static str {
    if cfg!(debug_assertions) {
        " (debug build)"
    } else {
        ""
    }
}

fn gib(bytes: u64) -> String {
    format!("{:.1} GiB", bytes as f64 / f64::from(1u32 << 30))
}

fn mib(bytes: u64) -> String {
    format!("{} MiB", bytes >> 20)
}

#[cfg(windows)]
use windows::{cpu, memory, os, process_memory};

#[cfg(windows)]
mod windows {
    use super::{gib, mib};
    use std::ffi::c_void;

    #[repr(C)]
    struct OsVersionInfo {
        size: u32,
        major: u32,
        minor: u32,
        build: u32,
        platform: u32,
        service_pack: [u16; 128],
    }

    #[repr(C)]
    struct MemoryStatus {
        length: u32,
        load: u32,
        total_phys: u64,
        avail_phys: u64,
        total_page_file: u64,
        avail_page_file: u64,
        total_virtual: u64,
        avail_virtual: u64,
        avail_extended_virtual: u64,
    }

    #[repr(C)]
    struct ProcessMemory {
        size: u32,
        page_faults: u32,
        peak_working_set: usize,
        working_set: usize,
        peak_paged_pool: usize,
        paged_pool: usize,
        peak_nonpaged_pool: usize,
        nonpaged_pool: usize,
        pagefile: usize,
        peak_pagefile: usize,
    }

    #[link(name = "ntdll")]
    extern "system" {
        fn RtlGetVersion(info: *mut OsVersionInfo) -> i32;
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(status: *mut MemoryStatus) -> i32;
        fn GetCurrentProcess() -> isize;
        fn K32GetProcessMemoryInfo(process: isize, counters: *mut ProcessMemory, size: u32) -> i32;
    }

    #[link(name = "advapi32")]
    extern "system" {
        fn RegGetValueW(
            key: isize,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            kind: *mut u32,
            data: *mut c_void,
            len: *mut u32,
        ) -> i32;
    }

    /// `HKEY_LOCAL_MACHINE`: a sign-extended 32-bit handle.
    const LOCAL_MACHINE: isize = 0x8000_0002_u32 as i32 as isize;
    const ANY_STRING: u32 = 0x2;
    const DWORD: u32 = 0x10;

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    /// A string or number from under `HKEY_LOCAL_MACHINE`.
    fn registry(key: &str, value: &str, flags: u32) -> Option<Vec<u8>> {
        let (key, value) = (wide(key), wide(value));
        let mut data = [0u8; 512];
        let mut len = data.len() as u32;
        // SAFETY: the key and value names are NUL-terminated, `data` holds `len` bytes,
        // and the type out-pointer may be null.
        let status = unsafe {
            RegGetValueW(
                LOCAL_MACHINE,
                key.as_ptr(),
                value.as_ptr(),
                flags,
                std::ptr::null_mut(),
                data.as_mut_ptr().cast(),
                &mut len,
            )
        };
        (status == 0).then(|| data[..len as usize].to_vec())
    }

    fn registry_string(key: &str, value: &str) -> Option<String> {
        let bytes = registry(key, value, ANY_STRING)?;
        let units: Vec<u16> = bytes
            .chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]))
            .take_while(|&u| u != 0)
            .collect();
        Some(String::from_utf16_lossy(&units).trim().to_owned())
    }

    fn registry_number(key: &str, value: &str) -> Option<u32> {
        let bytes = registry(key, value, DWORD)?;
        Some(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?))
    }

    const CURRENT_VERSION: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";

    /// "Windows 11 Pro 23H2 (10.0.22631.4317)". The registry's product name says
    /// Windows 10 on Windows 11 too; the build number tells them apart.
    pub(in super::super) fn os() -> String {
        let mut info = OsVersionInfo {
            size: size_of::<OsVersionInfo>() as u32,
            major: 0,
            minor: 0,
            build: 0,
            platform: 0,
            service_pack: [0; 128],
        };
        // SAFETY: `info` is an OSVERSIONINFOW with its size set, as RtlGetVersion requires.
        unsafe { RtlGetVersion(&mut info) };
        let mut product =
            registry_string(CURRENT_VERSION, "ProductName").unwrap_or_else(|| "Windows".into());
        if info.build >= 22000 {
            product = product.replace("Windows 10", "Windows 11");
        }
        let release = registry_string(CURRENT_VERSION, "DisplayVersion").unwrap_or_default();
        let patch = registry_number(CURRENT_VERSION, "UBR").unwrap_or(0);
        format!(
            "{product} {release} ({}.{}.{}.{patch})",
            info.major, info.minor, info.build
        )
    }

    pub(in super::super) fn cpu() -> String {
        registry_string(
            r"HARDWARE\DESCRIPTION\System\CentralProcessor\0",
            "ProcessorNameString",
        )
        .unwrap_or_else(|| "unknown processor".into())
    }

    pub(in super::super) fn memory() -> String {
        let mut status = MemoryStatus {
            length: size_of::<MemoryStatus>() as u32,
            load: 0,
            total_phys: 0,
            avail_phys: 0,
            total_page_file: 0,
            avail_page_file: 0,
            total_virtual: 0,
            avail_virtual: 0,
            avail_extended_virtual: 0,
        };
        // SAFETY: `status` is a MEMORYSTATUSEX with its length set, as the call requires.
        if unsafe { GlobalMemoryStatusEx(&mut status) } == 0 {
            return "unknown".into();
        }
        format!(
            "{} installed, {} free ({}% in use), commit {} of {} free",
            gib(status.total_phys),
            gib(status.avail_phys),
            status.load,
            gib(status.avail_page_file),
            gib(status.total_page_file),
        )
    }

    pub(in super::super) fn process_memory() -> String {
        // SAFETY: zeroes are a valid PROCESS_MEMORY_COUNTERS (plain integers).
        let mut counters: ProcessMemory = unsafe { std::mem::zeroed() };
        let size = size_of::<ProcessMemory>() as u32;
        // SAFETY: `counters` is a PROCESS_MEMORY_COUNTERS of `size` bytes, and the
        // pseudo-handle of the current process needs no closing.
        if unsafe { K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, size) } == 0 {
            return "unknown".into();
        }
        format!(
            "{} in use (peak {}), {} committed (peak {})",
            mib(counters.working_set as u64),
            mib(counters.peak_working_set as u64),
            mib(counters.pagefile as u64),
            mib(counters.peak_pagefile as u64),
        )
    }
}

#[cfg(not(windows))]
fn os() -> String {
    let name = std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|text| {
            text.lines()
                .find_map(|l| l.strip_prefix("PRETTY_NAME="))
                .map(|v| v.trim_matches('"').to_owned())
        })
        .unwrap_or_else(|| std::env::consts::OS.to_owned());
    match std::fs::read_to_string("/proc/sys/kernel/osrelease") {
        Ok(kernel) => format!("{name}, kernel {}", kernel.trim()),
        Err(_) => name,
    }
}

#[cfg(not(windows))]
fn cpu() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|text| {
            text.lines()
                .find(|l| l.starts_with("model name"))
                .and_then(|l| l.split_once(':'))
                .map(|(_, v)| v.trim().to_owned())
        })
        .unwrap_or_else(|| std::env::consts::ARCH.to_owned())
}

/// A `Key:   123 kB` field of a /proc file, in bytes.
#[cfg(not(windows))]
fn proc_field(text: &str, key: &str) -> Option<u64> {
    let line = text.lines().find(|l| l.starts_with(key))?;
    let kb: u64 = line[key.len()..]
        .trim_start_matches(':')
        .trim()
        .trim_end_matches("kB")
        .trim()
        .parse()
        .ok()?;
    Some(kb << 10)
}

#[cfg(not(windows))]
fn memory() -> String {
    let Ok(text) = std::fs::read_to_string("/proc/meminfo") else {
        return "unknown".into();
    };
    match (
        proc_field(&text, "MemTotal"),
        proc_field(&text, "MemAvailable"),
    ) {
        (Some(total), Some(free)) => format!("{} installed, {} free", gib(total), gib(free)),
        _ => "unknown".into(),
    }
}

#[cfg(not(windows))]
fn process_memory() -> String {
    let Ok(text) = std::fs::read_to_string("/proc/self/status") else {
        return "unknown".into();
    };
    match (proc_field(&text, "VmRSS"), proc_field(&text, "VmHWM")) {
        (Some(now), Some(peak)) => format!("{} in use (peak {})", mib(now), mib(peak)),
        _ => "unknown".into(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_system_section_names_the_machine() {
        let text = super::describe();
        for field in ["build:", "os:", "cpu:", "memory:", "this process:", "args:"] {
            assert!(text.contains(field), "no {field} in:\n{text}");
        }
        assert!(!text.contains("memory: unknown"), "{text}");
    }
}
