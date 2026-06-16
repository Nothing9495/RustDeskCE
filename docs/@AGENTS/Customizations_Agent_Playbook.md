# Customizations_Agent_Playbook.md

## Objective
Use this playbook to apply repository customizations in a controlled, confirm-first workflow. It contains complete implementation snippets and mandatory pre-execution confirmations.

## Restrictions
- This playbook is written for **RustDesk v1.4.6**, which is not always compatiable for newer versions. 
- For a agent who is performing this playbook, **all changes should be made according to Detailed Implementations sections**, and there's no alternative way to do it. 
- Note that this playbook might be outdated for newer versions of RustDesk, in that case, **ask the user for further instructions first**.
- Keep non-Windows behavior unchanged; use proper `cfg` guards.
- Do not commit or push without explicit user approval.
- Do not make any changes to unrelated code files.
- Do not try to examine code by `cargo` or other toolchain, which is very likely to be unavailable in current environment.


## Reference
For hotkey-related changes, use: [Global_Hotkey_Agent_Playbook.md](Global_Hotkey_Agent_Playbook.md)

---

## Changes Summary
|Function/Variables/Module|Location|Change|Details|Remark|
|:------|:-------------:|:------:|:------:|:------|
|RENDEZVOUS_SERVERS|libs/hbb_common/config.rs|Value|N/A|Change the default server address (user-provided)|
|RS_PUB_KEY|libs/hbb_common/config.rs|Value|N/A|Change the default secret key (user-provided)|
|get_auto_id()|libs/hbb_common/config.rs|Implementation|See Detailed Implementations section|Use custom ID generation logic|
|set_permanent_password()<br>get_permanent_password()|libs/hbb_common/config.rs|Implementation|See Detailed Implementations section|Set a fixed permanent password (user-provided)|
|start_listen_ipc_thread()|src/flutter.rs|Implementation|`start_listen_ipc(true)` -> `start_listen_ipc(false)`|Disable the Connection Manager panel|
|start_tray()|src/tray.rs|Implementation|`if crate... == "Y"` -> `if true`|Disable the system tray on Windows|
|global_hotkey|src/global_hotkey.rs<br>src/lib.rs<br>src/core_main.rs<br>src/server.rs|New module|See [Global_Hotkey_Agent_Playbook.md](Global_Hotkey_Agent_Playbook.md)|Add a hotkey to open the RustDesk main window|

---

## Detailed Implementations (full code snippets)

### get_auto_id()
New implementation:

```rust
fn get_auto_id() -> Option<String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        return Some(format!(
            "RDM-{:05X}",
            rand::thread_rng().gen_range(0x10000..0x100000)
        ));
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        #[cfg(target_os = "windows")]
        const OS_SUFFIX: &str = "W";
        #[cfg(target_os = "linux")]
        const OS_SUFFIX: &str = "L";
        #[cfg(target_os = "macos")]
        const OS_SUFFIX: &str = "X";

        let mut id = 0u32;
        if let Ok(Some(ma)) = mac_address::get_mac_address() {
            for x in &ma.bytes()[2..] {
                id = (id << 8) | (*x as u32);
            }
            id = (id % 0xF0000) + 0x10000;
            Some(format!("RDD-{:05X}{OS_SUFFIX}", id))
        } else {
            let id = rand::thread_rng().gen_range(0x10000..0x100000);
            Some(format!("RDD-{:05X}{OS_SUFFIX}", id))
        }
    }
}
```

Original implementation (for reference):

```rust
fn get_auto_id() -> Option<String> {
    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        return Some(
            rand::thread_rng()
                .gen_range(1_000_000_000..2_000_000_000)
                .to_string(),
        );
    }

    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    {
        let mut id = 0u32;
        if let Ok(Some(ma)) = mac_address::get_mac_address() {
            for x in &ma.bytes()[2..] {
                id = (id << 8) | (*x as u32);
            }
            id &= 0x1FFFFFFF;
            log::info!("Generated id {}", id);
            Some(id.to_string())
        } else {
            None
        }
    }
}
```

---

### set_permanent_password() and get_permanent_password()

New implementation (CAUTION: never use a password reused from online accounts):

```rust
pub fn set_permanent_password(password: &str) {
    return;
}

pub fn get_permanent_password() -> String {
    // PLEASE REPLACE <Custom Password> with the password you want to use.
    // The Agent will prompt you to provide the final value before applying code changes.
    const FIXED_PWD: &str = "<Custom Password>";
    FIXED_PWD.to_string()
}
```

Original implementation (for reference):

```rust
pub fn set_permanent_password(password: &str) {
    if Self::is_disable_change_permanent_password() {
        return;
    }
    if HARD_SETTINGS
        .read()
        .unwrap()
        .get("password")
        .map_or(false, |v| v == password)
    {
        if CONFIG.read().unwrap().password.is_empty() {
        return;
    }
    }
    let mut config = CONFIG.write().unwrap();
    if password == config.password {
        return;
    }
    config.password = password.into();
    config.store();
    Self::clear_trusted_devices();
}

pub fn get_permanent_password() -> String {
    let mut password = CONFIG.read().unwrap().password.clone();
    if password.is_empty() {
        if let Some(v) = HARD_SETTINGS.read().unwrap().get("password") {
            password = v.to_owned();
        }
    }
}
```

---

## Planned Operations (Agent MUST ask before executing)
Execute only confirmed items. Do not apply any code change until the user explicitly selects operations and provides required values.
The operations to ask are listed below (The text inside the **()** is **comment text** and shall **not be prompted** to the user.):

- [ ] Use custom RustDesk Server address. (Update `libs/hbb_common/config.rs`: replace RENDEZVOUS_SERVERS default value)

- [ ] Use custom RS_PUB_KEY secret key. (Update `libs/hbb_common/config.rs`: set RS_PUB_KEY default value)

- [ ] Use new RustDesk ID generation implementation. (Replace `get_auto_id()` implementation in `libs/hbb_common/config.rs` with the New implementation above)

- [ ] Set permanent, fixed password that can't be changed from RustDesk settings. (Replace `set_permanent_password()` and `get_permanent_password()` in `libs/hbb_common/config.rs` with the New implementation above)

- [ ] Do not show Connection Manager panel during remote control. (Change `start_listen_ipc_thread()` call in `src/flutter.rs` (`start_listen_ipc(true)` -> `start_listen_ipc(false)`))

- [ ] Do not show system tray on Windows. (Change `start_tray()` logic in `src/tray.rs` to force hide tray on Windows (`if crate... == "Y"` -> `if true`))

- [ ] Use `LShift+LAlt+R` to open RustDesk. (Add `src/global_hotkey.rs` and update `src/lib.rs`, `src/core_main.rs`, `src/server.rs` according to [Global_Hotkey_Agent_Playbook.md](Global_Hotkey_Agent_Playbook.md))

Input rules:
- If the user chosed `Use custom RustDesk Server address` without choosing `Use custom RS_PUB_KEY secret key`, prompt the user adding a Custom secret key would be a better choice when using custom RustDesk Server, then ask for further instructions. If the user declines, just continue.
- If `Use custom RustDesk Server address` is selected, request the exact value of `RENDEZVOUS_SERVERS` before editing.
- If `Use custom RS_PUB_KEY secret key` is selected, request the exact value of `RS_PUB_KEY` before editing.
- If the user declines to provide `RENDEZVOUS_SERVERS` or `RS_PUB_KEY`, skip those value changes.
- If fixed password replacement is selected, request the exact value of `<Custom Password>` and warn the user not to reuse any online-account password.
- If the user declines to provide `<Custom Password>`, just keep asking him. If the user persists, just refuse to make any customizations to the current project.
- If the user chosed `Do not show system tray on Windows` without choosing `Use LShift+LAlt+R to open RustDesk`, prompt the user use LShift+LAlt+R to open RustDesk would be a nice alternative to open RustDesk, then ask for further instructions. If the user declines, just continue.

---

## Next step
Reply with:
1) The list of operations you want executed (pick any from the Planned Operations).
2) If you selected the permanent password replacement, provide the exact Custom Password to insert.

After confirmation, apply only selected operations and report the result.
