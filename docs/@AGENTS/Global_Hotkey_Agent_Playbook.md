# Global_Hotkey_Agent_Playbook.md

## Objective
Implement the Windows global hotkey `Shift+Alt+R` and keep it stable:
- Bring up the RustDesk main window at any time
- Start a new instance when no window exists
- Keep the hotkey working after the main window is closed

## Scope
- Platform: Windows only
- Non-target platforms: Android, iOS, macOS, Linux remain unchanged

---

## Core Implementation Rules
1. Initialize hotkey listening in `core_main()` before argument dispatch.
2. Use `rdev::listen()` (`WH_KEYBOARD_LL`) for listening; do not rely on `RegisterHotKey`.
3. On trigger, bring an existing window to foreground first (`FindWindowW + TOPMOST trick`); do not rely only on a spawned process.
4. On Windows, do not `exit(-1)` when IPC startup fails in the `--server` branch.
5. Use `Once` to register the listener once per process.

---

## Standard Implementation Layout

### 1) Module Declaration
Add a Windows-gated module declaration in `src/lib.rs`:

```rust
#[cfg(all(windows, not(any(target_os = "android", target_os = "ios"))))]
mod global_hotkey;
```

### 2) Global Hotkey Module
Create `src/global_hotkey.rs`:
- `start_hotkey_listener()`: spawn a new thread + `rdev::listen`
- Track `Shift` / `Alt` state manually
- Trigger when `KeyR` matches the modifier state
- Call `show_or_launch_main_window()`

Window activation strategy:
- If window is found:
  - `FindWindowW("FLUTTER_RUNNER_WIN32_WINDOW", app_name)`
    - Restore first when minimized with `ShowWindow(SW_RESTORE)`
  - `SetWindowPos(HWND_TOPMOST)`
  - `SetForegroundWindow`
  - `BringWindowToTop`
  - `SetWindowPos(HWND_NOTOPMOST)`
- If window is not found, use `crate::run_me::<&str>(vec![])` as fallback

### 3) Entry Initialization Timing
In `src/core_main.rs`, start the listener after `init_log()` and before argument dispatch:

```rust
#[cfg(windows)]
{
    use std::sync::Once;
    static HOTKEY_START: Once = Once::new();
    HOTKEY_START.call_once(|| {
        crate::global_hotkey::start_hotkey_listener();
    });
}
```

### 4) Keep `--server` Alive on Windows
In the IPC startup failure branch in `src/server.rs`:
- Non-Windows: keep existing `exit(-1)` behavior
- Windows: log `warn` and continue running

Goal: keep `--server` alive so the hotkey thread remains active.

---

## Why This Approach (Short)
- `RegisterHotKey` is unreliable in this runtime shape.
- `rdev::listen` avoids the window message pump and is more stable here.
- Spawned-process activation is limited by Windows foreground policy.
- The listener must stay in a long-running user-session process.

---

## Agent Execution Checklist
- [ ] `src/global_hotkey.rs` added
- [ ] Windows-gated module declared in `src/lib.rs`
- [ ] Listener startup via `Once` added in `src/core_main.rs`
- [ ] Windows IPC failure in `src/server.rs` no longer exits
- [ ] Non-Windows build paths remain isolated by `cfg`

---

## Acceptance Criteria
1. When a `--server` process exists, pressing `Shift+Alt+R` opens the main window.
2. If the main window is covered or minimized, the hotkey brings it to front.
3. After closing the main window, the hotkey reopens it.
4. Logs contain listener startup output.
5. Windows build passes (including flutter-related feature combinations).

---

## Common Failures and Fixes
1. Hotkey does not respond and no logs appear
- Cause: listener code does not run in the real entry path.
- Fix: start it in `core_main()` before argument dispatch. Check RustDesk logs in `%APPDATA%\RustDesk`.

2. Hotkey logs appear but window is not brought to front
- Cause: only `run_me()` runs; a spawned process cannot reliably take focus.
- Fix: foreground the existing window in the current process. Keep `run_me()` only as fallback.

3. Hotkey stops working after main window is closed
- Cause: `--server` exits on IPC startup failure, which kills the listener.
- Fix: on Windows, keep running and log a warning.

4. Listener registered in service Session 0 is completely ineffective
- Cause: Session 0 cannot intercept keyboard input from interactive user sessions.
- Fix: start the listener in a user-session process via `core_main()`.

---

## Minimal Regression Steps
1. Start the program and verify `--server` is running.
2. Press `Shift+Alt+R` from any desktop-focused application.
3. Minimize the main window and press the hotkey again.
4. Close the main window, wait 2-3 seconds, then press the hotkey again.
5. Check logs for listener startup and trigger records.

---

## Do Not Do These
- Do not revert to `RegisterHotKey` + message-window implementation.
- Do not register the listener in Session 0 service threads.
- Do not call `process::exit(-1)` on Windows IPC startup failure.
- Do not rely entirely on newly spawned processes for window activation.

---

## Reusable Implementation Snippet
### `src/core_main.rs` entry after init_log:
```rust
// existing code...
hbb_common::init_log(false, &log_name);

#[cfg(windows)]
{
    use std::sync::Once;
    static HOTKEY_START: Once = Once::new();
    HOTKEY_START.call_once(|| {
        crate::global_hotkey::start_hotkey_listener();
    });
}
// existing code...
```
### `src/lib.rs`, after `clipboard_file` module
```rust
// existing code...
#[cfg(any(target_os = "windows", target_os = "linux", target_os = "macos"))]
pub mod clipboard_file;

#[cfg(all(windows, not(any(target_os = "android", target_os = "ios"))))]
mod global_hotkey;
// existing code...
```
### `src/server.rs` server won't fail if IPC failed to run.
```rust
// existing code...
std::thread::spawn(move || {
    if let Err(err) = crate::ipc::start("") {
        log::error!("Failed to start ipc: {}", err);
        // existing code...
        #[cfg(not(windows))]
        std::process::exit(-1);
        #[cfg(windows)]
        {
            log::warn!("ipc start failed on Windows, server continues without local IPC (--service may already provide it)");
        }
    }
});
```
### Reusable Global_Hotkey Module
```rust
// src/global_hotkey.rs
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
            // Window exists. Restore first, then force foreground.
            if IsIconic(hwnd) != 0 {
                ShowWindow(hwnd, SW_RESTORE);
            }

            // Make it topmost briefly so SetForegroundWindow succeeds.
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE,
            );
            SetForegroundWindow(hwnd);
            BringWindowToTop(hwnd);

            // Remove topmost so it does not stay pinned above all other windows.
            SetWindowPos(
                hwnd,
                HWND_NOTOPMOST,
                0, 0, 0, 0,
                SWP_NOMOVE | SWP_NOSIZE,
            );

            // log::info!("global_hotkey: main window activated");
        } else {
            // No existing window. Launch a new instance.
            // log::info!("global_hotkey: no existing window, launching via run_me");
            crate::run_me::<&str>(vec![]).ok();
        }
    }
}
```