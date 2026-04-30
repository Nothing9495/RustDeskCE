// Global hotkey listener for opening the main window via Shift+Alt+R.
// Only compiled on Windows (non-Android, non-iOS).
//
// Uses `rdev::listen()` — a low-level keyboard hook (WH_KEYBOARD_LL on Windows)
// to detect the chord system-wide.
// When triggered, directly finds and activates the Flutter main window (or
// launches it if not running), using proper Win32 foreground-stealing techniques.

use hbb_common::log;
use rdev::{listen, Event, EventType, Key};
#[cfg(windows)]
use winapi::um::winuser::{
    BringWindowToTop, FindWindowW, IsIconic, SetForegroundWindow,
    SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOPMOST, SW_RESTORE,
    SWP_NOMOVE, SWP_NOSIZE,
};

/// Start a background thread that listens for Shift+Alt+R globally
/// and shows or launches the main application window.
#[cfg(windows)]
pub fn start_hotkey_listener() {
    std::thread::spawn(move || {
        let mut shift = false;
        let mut alt = false;

        let callback = move |event: Event| {
            match event.event_type {
                EventType::KeyPress(key) => match key {
                    Key::ShiftLeft | Key::ShiftRight => shift = true,
                    Key::Alt | Key::AltGr => alt = true,
                    Key::KeyR => {
                        if shift && alt {
                            // log::info!("global_hotkey: Shift+Alt+R triggered");
                            show_or_launch_main_window();
                        }
                    }
                    _ => {}
                },
                EventType::KeyRelease(key) => match key {
                    Key::ShiftLeft | Key::ShiftRight => shift = false,
                    Key::Alt | Key::AltGr => alt = false,
                    _ => {}
                },
                _ => {}
            }
        };

        log::info!("global_hotkey: Shift+Alt+R listener started");
        if let Err(e) = listen(callback) {
            log::error!("global_hotkey: rdev::listen failed: {:?}", e);
        }
        log::info!("global_hotkey: listener exited");
    });
}

/// Find the existing Flutter main window and bring it to the foreground.
/// If no window exists, launch a new instance via `run_me()`.
#[cfg(windows)]
fn show_or_launch_main_window() {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    let class: Vec<u16> = OsStr::new(crate::platform::FLUTTER_RUNNER_WIN32_WINDOW_CLASS)
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let title: Vec<u16> = OsStr::new(&crate::get_app_name())
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    unsafe {
        let hwnd = FindWindowW(class.as_ptr(), title.as_ptr());

        if !hwnd.is_null() {
            // Window exists — restore + force-foreground.
            // 1. Restore from minimized state.
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }

            // 2. Briefly make topmost so SetForegroundWindow succeeds
            //    and the window reliably comes to the foreground.
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);

            // 3. Remove topmost so it doesn't stay pinned above all other windows.
            SetWindowPos(
                hwnd,
                HWND_NOTOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE,
            );

            // log::info!("global_hotkey: main window activated");
        } else {
            // No existing window — launch a new instance.
            // log::info!("global_hotkey: no existing window, launching via run_me");
            crate::run_me::<&str>(vec![]).ok();
        }
    }
}
