//! The crash window: what happened in a sentence, and two buttons that get the
//! details to us. Copy details puts the whole report on the clipboard (the window
//! stays open and says it was copied); Open folder shows the report's file in
//! Explorer. A Windows task dialog, drawn by the system, so it works when the
//! game's own renderer is what failed. Elsewhere it is a summary on stderr.

use std::path::Path;

pub(super) struct Shown<'a> {
    /// The window's large first line.
    pub heading: &'a str,
    /// What happened.
    pub content: &'a str,
    /// What the player can do about it, when we know.
    pub hint: Option<&'a str>,
    /// The whole report, for Copy details.
    pub report: &'a str,
    /// Where the report was saved, for Open folder.
    pub path: Option<&'a Path>,
}

/// No window outside Windows: the player started the game from somewhere that
/// shows stderr, so the summary goes there, under the report the hook printed.
#[cfg(not(windows))]
pub(super) fn show(shown: &Shown<'_>) {
    use std::io::Write;
    let mut text = format!("\n{}\n{}\n", shown.heading, shown.content);
    if let Some(hint) = shown.hint {
        text.push_str(&format!("{hint}\n"));
    }
    if let Some(path) = shown.path {
        text.push_str(&format!(
            "The full report ({} lines) is in {}; please send it to the developers.\n",
            shown.report.lines().count(),
            path.display()
        ));
    }
    let _ = std::io::stderr().write_all(text.as_bytes());
}

#[cfg(windows)]
pub(super) use windows::show;

#[cfg(windows)]
mod windows {
    use super::Shown;
    use std::ffi::c_void;
    use std::path::Path;

    /// `TASKDIALOGCONFIG`, which commctrl.h packs to 1 byte.
    #[repr(C, packed(1))]
    struct Config {
        size: u32,
        parent: isize,
        instance: isize,
        flags: i32,
        common_buttons: i32,
        window_title: *const u16,
        main_icon: *const u16,
        main_instruction: *const u16,
        content: *const u16,
        button_count: u32,
        buttons: *const Button,
        default_button: i32,
        radio_count: u32,
        radios: *const Button,
        default_radio: i32,
        verification: *const u16,
        expanded_information: *const u16,
        expanded_control: *const u16,
        collapsed_control: *const u16,
        footer_icon: *const u16,
        footer: *const u16,
        callback: Option<Callback>,
        callback_data: isize,
        width: u32,
    }

    #[repr(C, packed(1))]
    struct Button {
        id: i32,
        text: *const u16,
    }

    type Callback = unsafe extern "system" fn(
        window: isize,
        notification: u32,
        wparam: usize,
        lparam: isize,
        data: isize,
    ) -> i32;
    type TaskDialogIndirect =
        unsafe extern "system" fn(*const Config, *mut i32, *mut i32, *mut i32) -> i32;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryW(name: *const u16) -> isize;
        fn GetProcAddress(module: isize, name: *const u8) -> *const c_void;
    }
    #[link(name = "user32")]
    extern "system" {
        fn MessageBoxW(window: isize, text: *const u16, caption: *const u16, kind: u32) -> i32;
        fn SendMessageW(window: isize, message: u32, wparam: usize, lparam: isize) -> isize;
        fn SetWindowPos(
            window: isize,
            after: isize,
            x: i32,
            y: i32,
            w: i32,
            h: i32,
            flags: u32,
        ) -> i32;
        fn SetForegroundWindow(window: isize) -> i32;
    }

    const COPY: i32 = 100;
    const OPEN_FOLDER: i32 = 101;

    /// What the callback needs: the report to copy, the file to show, and the
    /// footer lines it switches between.
    struct State<'a> {
        report: &'a str,
        path: Option<&'a Path>,
        copied: Vec<u16>,
        not_copied: Vec<u16>,
    }

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain([0]).collect()
    }

    pub(in crate::crash) fn show(shown: &Shown<'_>) {
        let Some(task_dialog) = task_dialog() else {
            message_box(shown);
            return;
        };
        let mut content = shown.content.to_owned();
        if let Some(hint) = shown.hint {
            content.push_str("\n\n");
            content.push_str(hint);
        }
        content.push_str(
            "\n\nTo report it, press Copy details and paste them into a message to the \
             developers.",
        );
        let footer = match shown.path {
            Some(path) => format!("Saved as {}", path.display()),
            None => "The report could not be saved to a file; Copy details still has it.".into(),
        };
        let lines = shown.report.lines().count();
        let state = State {
            report: shown.report,
            path: shown.path,
            copied: wide(&format!(
                "Copied the full report ({lines} lines) to the clipboard. {footer}"
            )),
            not_copied: wide(&format!("Could not open the clipboard. {footer}")),
        };
        let (title, heading, content, footer) = (
            wide("Meridian Conflict"),
            wide(shown.heading),
            wide(&content),
            wide(&footer),
        );
        let labels = [wide("Copy details"), wide("Open folder")];
        let mut buttons = vec![Button {
            id: COPY,
            text: labels[0].as_ptr(),
        }];
        if shown.path.is_some() {
            buttons.push(Button {
                id: OPEN_FOLDER,
                text: labels[1].as_ptr(),
            });
        }
        const ALLOW_CANCEL: i32 = 0x8;
        const CLOSE_BUTTON: i32 = 0x20;
        // TD_ERROR_ICON: MAKEINTRESOURCEW(-2).
        let error_icon = 0xFFFE_usize as *const u16;
        // TD_INFORMATION_ICON: MAKEINTRESOURCEW(-3).
        let info_icon = 0xFFFD_usize as *const u16;
        let config = Config {
            size: std::mem::size_of::<Config>() as u32,
            parent: 0,
            instance: 0,
            flags: ALLOW_CANCEL,
            common_buttons: CLOSE_BUTTON,
            window_title: title.as_ptr(),
            main_icon: error_icon,
            main_instruction: heading.as_ptr(),
            content: content.as_ptr(),
            button_count: buttons.len() as u32,
            buttons: buttons.as_ptr(),
            default_button: COPY,
            radio_count: 0,
            radios: std::ptr::null(),
            default_radio: 0,
            verification: std::ptr::null(),
            expanded_information: std::ptr::null(),
            expanded_control: std::ptr::null(),
            collapsed_control: std::ptr::null(),
            footer_icon: info_icon,
            footer: footer.as_ptr(),
            callback: Some(callback),
            callback_data: &state as *const State<'_> as isize,
            // Dialog units: wide enough for a file path and a panic's message.
            width: 300,
        };
        let mut pressed = 0;
        // SAFETY: every string is NUL-terminated UTF-16 and, like `buttons` and
        // `state`, outlives the call, which returns only when the window closes.
        let result = unsafe {
            task_dialog(
                &config,
                &mut pressed,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
            )
        };
        if result < 0 {
            message_box(shown);
        }
    }

    /// `TaskDialogIndirect`, from the common controls version 6 that the manifest
    /// (`meridian.manifest`) asks for. A build without the manifest has only
    /// version 5, which lacks it: then the plain message box is shown.
    fn task_dialog() -> Option<TaskDialogIndirect> {
        let name = wide("comctl32.dll");
        // SAFETY: a NUL-terminated UTF-16 name.
        let module = unsafe { LoadLibraryW(name.as_ptr()) };
        if module == 0 {
            return None;
        }
        // SAFETY: a NUL-terminated ASCII name in a loaded module.
        let proc = unsafe { GetProcAddress(module, c"TaskDialogIndirect".as_ptr().cast()) };
        // SAFETY: TaskDialogIndirect has this signature in comctl32 6.
        (!proc.is_null())
            .then(|| unsafe { std::mem::transmute::<*const c_void, TaskDialogIndirect>(proc) })
    }

    unsafe extern "system" fn callback(
        window: isize,
        notification: u32,
        wparam: usize,
        _lparam: isize,
        data: isize,
    ) -> i32 {
        const CREATED: u32 = 0;
        const BUTTON_CLICKED: u32 = 2;
        const SET_ELEMENT_TEXT: u32 = 0x400 + 108;
        const FOOTER: usize = 2;
        const KEEP_OPEN: i32 = 1;
        // SAFETY: `data` is the `State` that `show` passed, alive while the window is.
        let state = unsafe { &*(data as *const State<'_>) };
        match notification {
            // Above the game's window, which may be full screen.
            CREATED => {
                const TOPMOST: isize = -1;
                const NO_MOVE_NO_SIZE: u32 = 0x2 | 0x1;
                // SAFETY: `window` is the dialog, just created.
                unsafe {
                    SetWindowPos(window, TOPMOST, 0, 0, 0, 0, NO_MOVE_NO_SIZE);
                    SetForegroundWindow(window);
                }
                0
            }
            BUTTON_CLICKED if wparam as i32 == COPY => {
                let line = match crate::clipboard::copy(state.report) {
                    Ok(()) => &state.copied,
                    Err(_) => &state.not_copied,
                };
                // SAFETY: the footer exists (it was set), and `line` is
                // NUL-terminated and lives as long as the window.
                unsafe { SendMessageW(window, SET_ELEMENT_TEXT, FOOTER, line.as_ptr() as isize) };
                KEEP_OPEN
            }
            BUTTON_CLICKED if wparam as i32 == OPEN_FOLDER => {
                if let Some(path) = state.path {
                    open_folder(path);
                }
                KEEP_OPEN
            }
            _ => 0,
        }
    }

    /// Explorer, with the report selected.
    fn open_folder(path: &Path) {
        use std::os::windows::process::CommandExt;
        // Explorer reads `/select,` and the quoted path as one argument, which the
        // standard quoting would split.
        let _ = std::process::Command::new("explorer.exe")
            .raw_arg(format!("/select,\"{}\"", path.display()))
            .spawn();
    }

    /// The fallback: a message box. Its text can be copied with Ctrl+C, which it
    /// says.
    fn message_box(shown: &Shown<'_>) {
        let mut text = format!("{}\n\n{}", shown.heading, shown.content);
        if let Some(hint) = shown.hint {
            text.push_str(&format!("\n\n{hint}"));
        }
        if let Some(path) = shown.path {
            text.push_str(&format!(
                "\n\nThe full report was saved to {}. Please send it to the developers.",
                path.display()
            ));
        }
        text.push_str("\n\n(Ctrl+C copies this message.)");
        const ERROR_ICON: u32 = 0x10;
        const TOPMOST: u32 = 0x4_0000;
        const FOREGROUND: u32 = 0x1_0000;
        let (text, caption) = (wide(&text), wide("Meridian Conflict"));
        // SAFETY: both strings are NUL-terminated UTF-16 buffers that outlive the
        // call, and a null owner window is allowed.
        unsafe {
            MessageBoxW(
                0,
                text.as_ptr(),
                caption.as_ptr(),
                ERROR_ICON | TOPMOST | FOREGROUND,
            )
        };
    }
}
