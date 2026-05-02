# Customizations
## Changes
This customized edition includes the following changes to meet the repository owner's needs.
|Function/Variables/Module|Where is it?|What changed?|Details|Remark|
|:------|:-------------:|:------:|:------:|:------|
|RENDEZVOUS_SERVERS|libs/hbb_common/config.rs|Its value|N/A|Change the default server address|
|RS_PUB_KEY|libs/hbb_common/config.rs|Its value|N/A|Change the default secret key|
|get_auto_id()|libs/hbb_common/config.rs|Implementation|See [Detailed Implementations](#detailed-implementations)|Use custom ID generation logic|
|set_permanent_password()<br>get_permanent_password()|libs/hbb_common/config.rs|Implementation|See [Detailed Implementations](#detailed-implementations)|Set a fixed permanent password|
|start_listen_ipc_thread()|src/flutter.rs|Implementation|`start_listen_ipc(true)`-><br>`start_listen_ipc(false)`|Disable the Connection Manager panel|
|start_tray()|src/tray.rs|Implementation|`if crate... == "Y"`-><br>`if true`|Disable the system tray on Windows|
|global_hotkey|src/global_hotkey.rs<br>src/lib.rs<br>src/core_main.rs<br>src/server.rs|New module|See [@Global_Hotkey_Implementation.md](@Global_Hotkey_Implementation.md)|Add a hotkey to open the RustDesk main window|

## Detailed Implementations
### get_auto_id()
> [!NOTE]
> This new Implementation may not work on computer running offical RustDesk Server OSS. To resolve this, see [RustDeskServerCE](https://github.com/Nothing9495/RustDeskServerCE) 
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
### set_permanent_password() and get_permanent_password()
**New Implementation:**
> [!CAUTION]
> DO NOT use a password related to your online accounts!
```rust
pub fn set_permanent_password(password: &str) {
    return;
}

pub fn get_permanent_password() -> String {
    const FIXED_PWD: &str = "<Custom Password>";
    FIXED_PWD.to_string()
}
```
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