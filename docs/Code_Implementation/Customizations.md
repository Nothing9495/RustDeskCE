# Customizations
## Changes
 This customized edition has been made some changes, which have been listed below, to adapt repo owner's needs.
|Function/Variables/Module|Where is it?|What changed?|Details|Remark|
|:------|:-------------:|:------:|:------:|:------|
|RENDEZVOUS_SERVERS|libs/hbb_common/config.rs|It's value|none|Change default server|
|RS_PUB_KEY|libs/hbb_common/config.rs|It's value|none|Change default secret key|
|get_auto_id()|libs/hbb_common/config.rs|Implementation|See [Detailed Implementations](#detailed-implementaions)|Use custom id generating logic|
|set_permanent_password()<br>get_permanent_password()|libs/hbb_common/config.rs|Implementation|See [Detailed Implementations](#detailed-implementaions)|Set fixed permanent password|
|start_listen_ipc_thread()|src/flutter.rs|Implementation|`start_listen_ipc(true)`-><br>`start_listen_ipc(false)`|Disable Connection Manager Panel|
|start_tray()|src/tray.rs|Implementation|`if crate... == "Y"`-><br>`if true`|Disable system tray in Windows|
|global_hotkey|src/global_hotkey.rs<br>src/lib.rs<br>src/core_main.rs<br>src/server.rs|New Module|See [@Global_Hotkey_Implementation.md](@Global_Hotkey_Implementation.md)|Add hotkey to call RustDesk main window|

## Detailed Implementaions
### get_auto_id()
**New Implementation:**
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
**Original Implementation:**
```rust
```
### set_permanent_password() and get_permanent_password()
**New Implementation:**
```rust
pub fn set_permanent_password(password: &str) {
    return;
}

pub fn get_permanent_password() -> String {
    const FIXED_PWD: &str = "<Custom Password>";
    FIXED_PWD.to_string()
}
```
> [!CAUTION]
> DO NOT use password relates to your online accounts!
**Original Implementation:**
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