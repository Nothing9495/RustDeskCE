# Global_Hotkey_Agent_Playbook.md

## Objective
Implement and reliably maintain the global hotkey `Shift+Alt+R` on Windows:
- Pressing it at any time brings up the RustDesk main window
- If the main window does not exist, start a new instance
- The hotkey must keep working even after the main window is closed

## Scope
- Platform: Windows only
- Non-target platforms: Android, iOS, macOS, Linux (no behavior changes)

---

## Core Implementation Rules
1. Hotkey listening must be initialized in the `core_main()` entry, before argument dispatch.
2. Use `rdev::listen()` (`WH_KEYBOARD_LL`) for listening; do not rely on `RegisterHotKey`.
3. On trigger, prioritize directly bringing an existing window to foreground (`FindWindowW + TOPMOST trick`), rather than depending only on spawning a new process.
4. On Windows, if IPC startup fails in the `--server` branch, do not `exit(-1)`; otherwise the hotkey thread dies with the process.
5. Use `Once` to guarantee one listener registration per process and avoid duplicate threads.

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
- Manually track `Shift` / `Alt` state
- Trigger when `KeyR` is pressed and modifier state matches
- Call `show_or_launch_main_window()`

Window activation strategy:
- If window is found:
  - `FindWindowW("FLUTTER_RUNNER_WIN32_WINDOW", app_name)`
  - If minimized, call `ShowWindow(SW_RESTORE)` first
  - `SetWindowPos(HWND_TOPMOST)`
  - `SetForegroundWindow`
  - `BringWindowToTop`
  - `SetWindowPos(HWND_NOTOPMOST)`
- If window is not found: use `crate::run_me::<&str>(vec![])` as fallback

### 3) Entry Initialization Timing
In `src/core_main.rs`, initialize after `init_log()` and before argument dispatch:

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
- Windows: log `warn` only and continue running

Goal: prevent `--server` from exiting due to IPC conflicts, which would kill the hotkey thread.

---

## Why This Approach (Short)
- `RegisterHotKey` is unstable in this runtime shape; historically it often "registered successfully but never triggered".
- `rdev::listen` uses a low-level keyboard hook and does not require a window message pump; it is more reliable in this target scenario.
- Activating via a spawned process is often blocked by Windows foreground focus policy; direct in-process window activation works better.
- The hotkey thread must live in a long-running user-session process; `--server` must not exit due to IPC contention.

---

## Agent Execution Checklist
- [ ] `src/global_hotkey.rs` added successfully
- [ ] Windows-gated module declared in `src/lib.rs`
- [ ] Listener startup via `Once` added at entry in `src/core_main.rs`
- [ ] Windows IPC failure in `src/server.rs` no longer exits process
- [ ] Non-Windows build paths remain unaffected (isolated by `cfg`)

---

## Acceptance Criteria
1. After startup, when a `--server` process exists, pressing `Shift+Alt+R` brings up the main window.
2. If the main window is open but covered/minimized, the hotkey reliably brings it to front.
3. After closing the main window, pressing the hotkey still reopens it.
4. Logs contain listener startup messages.
5. Windows build passes (including flutter-related feature combinations).

---

## Common Failures and Fixes
1. Hotkey does not respond and no logs appear
- Cause: listener code is not executed in the real entry path.
- Fix: ensure startup is in `core_main()` before argument dispatch, and remind user to check RustDesk logs in `%APPDATA%\RustDesk`

2. Hotkey logs appear but window is not brought to front
- Cause: only `run_me()` is used; focus steal fails from a newly spawned process.
- Fix: perform direct Win32 foregrounding in current process; keep `run_me()` only as fallback when no window exists.

3. Hotkey stops working after main window is closed
- Cause: `--server` exits on IPC startup failure, and the listener thread disappears.
- Fix: on Windows, do not exit on IPC failure; log warning and continue.

4. Listener registered in service Session 0 is completely ineffective
- Cause: Session 0 cannot intercept keyboard input from interactive user sessions.
- Fix: listener must start in a user-session process (`core_main()` entry path).

---

## Minimal Regression Steps
1. Start the program and verify `--server` is alive.
2. Press `Shift+Alt+R` from any desktop-focused application.
3. After opening the main window, minimize it and press the hotkey again to verify foregrounding.
4. Close the main window, wait 2-3 seconds, then press the hotkey to verify reopen behavior.
5. Check logs for listener startup and trigger records.

---

## Do Not Do These (For Agents)
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
### Resuable Global_Hotkey Module
```rust
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
```