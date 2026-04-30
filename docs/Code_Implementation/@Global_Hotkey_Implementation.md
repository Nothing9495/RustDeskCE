# @Global_Hotkey_Implementation.md — 全局热键 Shift+Alt+R 唤起主界面

> **日期**: 2026-04-29  
> **功能**: 捕获键盘快捷键 `Shift+Alt+R` 打开 RustDesk 主界面  
> **平台**: 仅 Windows（Android/macOS/Linux 不受影响）  
> **状态**: ✅ 已通过 GitHub Actions `x86_64-pc-windows-msvc` 构建

---

## 需求规格

| 项目 | 说明 |
|------|------|
| 快捷键 | `Shift + Alt + R` |
| 触发行为 | 唤起/启动 RustDesk Flutter 主窗口 |
| 生效条件 | 全局（无论主窗口是否打开）；通过手动跟踪 Shift/Alt 状态防止按键重复触发 |
| 运行载体 | `core_main()` 入口，`Once` 保证单次（覆盖 `--server` + 主窗口 + `--service` 三个进程） |
| 平台限制 | 仅 Windows 构建；Android 无此功能 |

---

## 架构概览

```
core_main() (所有进程共享入口)
├── init_log()
├── ★ Once → start_hotkey_listener()                     ← 用户会话进程生效
│       └── rdev::listen(WH_KEYBOARD_LL) 独立线程
│            └── Shift+Alt+R → show_or_launch_main_window()
│                 ├── 窗口存在 → FindWindowW → TOPMOST → SetForegroundWindow
│                 └── 窗口不存在 → run_me() 启动新实例
├── --service 分支 → start_os_service() → run_service() → launch_server(...)
├── --server 分支 → start_server(true, false)           ← IPC 失败不再退出
│       └── RendezvousMediator::start_all() (阻塞)
└── 主窗口分支 → Flutter UI
```


---

## 文件变更清单

### 1. 新建: `src/global_hotkey.rs`

完整的 Windows 全局热键监听模块。

**核心逻辑**:
- `start_hotkey_listener()` 启动独立线程
- 使用 `rdev::listen()` (WH_KEYBOARD_LL) 检测 Shift+Alt+R
- 触发热键时调用 `show_or_launch_main_window()`：
  - `FindWindowW("FLUTTER_RUNNER_WIN32_WINDOW", app_name)` 查找已有窗口
  - 窗口存在 → `SetWindowPos(HWND_TOPMOST)` → `SetForegroundWindow` → `BringWindowToTop` → `SetWindowPos(HWND_NOTOPMOST)` 可靠激活
  - 窗口不存在 → `crate::run_me()` fallback 启动新实例

**依赖**: `rdev` + `winapi`（项目已有，无需新增）

**模块级导入**:
```rust
use hbb_common::log;
use rdev::{listen, Event, EventType, Key};
#[cfg(windows)]
use winapi::um::winuser::{
    BringWindowToTop, FindWindowW, IsIconic, SetForegroundWindow,
    SetWindowPos, ShowWindow, HWND_NOTOPMOST, HWND_TOPMOST,
    SW_RESTORE, SWP_NOMOVE, SWP_NOSIZE,
};
```

**函数结构**:
```rust
#[cfg(windows)]
pub fn start_hotkey_listener() {
    std::thread::spawn(move || {
        let mut shift = false; let mut alt = false;
        rdev::listen(move |event: Event| {
            // track Shift/Alt state, detect R chord
            if shift && alt && key == KeyR {
                show_or_launch_main_window();  // ★ 直接操作窗口
            }
        });
    });
}

#[cfg(windows)]
fn show_or_launch_main_window() {
    let hwnd = FindWindowW("FLUTTER_RUNNER_WIN32_WINDOW", &crate::get_app_name());
    if !hwnd.is_null() {
        // TOPMOST → SetForegroundWindow → BringWindowToTop → NOTOPMOST
    } else {
        crate::run_me::<&str>(vec![]).ok();  // fallback
    }
}
```

### 2. 修改: `src/lib.rs`

**位置**: ~第 80 行，`clipboard_file` 模块声明之后

**新增**:
```rust
#[cfg(all(windows, not(any(target_os = "android", target_os = "ios"))))]
mod global_hotkey;
```

### 3. 修改: `src/core_main.rs`

**位置**: `init_log()` 调用之后、所有 arg 分发之前（~第 158 行）

**新增**:
```rust
hbb_common::init_log(false, &log_name);

#[cfg(windows)]
{
    use std::sync::Once;
    static HOTKEY_START: Once = Once::new();
    HOTKEY_START.call_once(|| {
        crate::global_hotkey::start_hotkey_listener();
    });
}
```

### 4. 修改: `src/server.rs`

**位置**: `start_server(is_server=true)` 分支，IPC 启动失败处理（~第 598 行）

**变更**: Windows 上不再 `process::exit(-1)`：
```rust
std::thread::spawn(move || {
    if let Err(err) = crate::ipc::start("") {
        log::error!("Failed to start ipc: {}", err);
        // ...
        #[cfg(not(windows))]
        std::process::exit(-1);
        #[cfg(windows)]
        log::warn!("ipc start failed, server continues (--service provides IPC)");
    }
});
```

---

## 已知问题修复历史

### v1.0 → v1.1 (2026-04-29): GitHub Actions 构建错误修复

**现象**: `cargo build --features hwcodec,vram,flutter --lib --release` 失败，8 个编译错误。

| 错误 | 原因 | 修复 |
|------|------|------|
| `cannot find type HWND/UINT/WPARAM/LPARAM/LRESULT in this scope` | `winapi` 类型导入在函数内部 `use` 块，独立 wndproc 访问不到 | 导入提升到模块顶层，标注 `#[cfg(windows)]` |
| `cannot find function DefWindowProcW in this scope` | 同上 | 模块顶层导入 |
| `expected u32, found isize` | 位标志运算类型推断为 `isize`，与 `UINT` 不匹配 | `as UINT` 显式转换 |

### v1.1 → v1.2 (2026-04-29): 热键不生效（第一轮）

**现象**: 构建通过、进程运行，但 Shift+Alt+R 无反应。

**根因**: `hInstance = 0` → `RegisterClassExW` 虽返回成功，`CreateWindowExW(HWND_MESSAGE)` 却静默失败，热键绑定到 NULL hwnd 后消息无法送达。

**尝试的修复**: 移除窗口创建，改用 `RegisterHotKey(NULL, ...)` 无窗口模式 → **仍然不生效**（见 v1.3）。

### v1.2 → v1.3 (2026-04-29): 热键不生效（第二轮）

**现象**: `RegisterHotKey(NULL, ...)` + `GetMessageW(NULL, ...)` 无窗口模式下热键依然不触发。

**根因**: `RegisterHotKey(NULL, ...)` 将 `WM_HOTKEY` 投递到线程消息队列（`msg.hwnd = NULL`），不经过窗口过程分发。这种方式在实践中不可靠——消息可能被系统丢弃或需要线程先创建消息泵。Windows API 文档推荐的做法是始终传入有效的窗口句柄。

**最终修复**: 回归窗口模式，用 `GetModuleHandleW(NULL)` 获取有效 `hInstance`：
- `GetModuleHandleW(NULL)` → 有效模块句柄
- `RegisterClassExW(&wc)` → `wc.hInstance = hinst`
- `CreateWindowExW(..., HWND_MESSAGE, ..., hinst)` → 消息窗口
- `RegisterHotKey(hwnd, ...)` → 绑定到有效窗口
- `hotkey_wndproc` → `DispatchMessageW` 将 `WM_HOTKEY` 路由到窗口过程
- `WM_DESTROY` → `PostQuitMessage(0)` → 退出消息循环

### v1.3 → v1.4 (2026-04-29): 热键不生效（第三轮）— 放弃 RegisterHotKey

**现象**: `GetModuleHandleW(NULL)` + `HWND_MESSAGE` + `RegisterHotKey` 窗口模式仍然不触发。实际运行环境 `NT AUTHORITY\SYSTEM` 在用户交互会话中，`RegisterHotKey` 行为不可靠——可能因桌面/安全边界导致 `WM_HOTKEY` 无法送达窗口过程。

**最终方案**: 完全放弃 `RegisterHotKey` / `winapi` 窗口模式，改用 **`rdev::listen()`** 低层级键盘钩子（`WH_KEYBOARD_LL`）：

| 对比 | `RegisterHotKey` | `rdev::listen()` |
|------|-------------------|-------------------|
| 实现层级 | 系统 API，依赖窗口消息分发 | 低层级全局钩子，拦截所有按键 |
| 窗口需求 | 需要 `HWND_MESSAGE` 窗口 + 消息泵 | **不需要任何窗口** |
| SYSTEM 兼容 | 不确定（桌面/安全边界问题） | ✅ 完全兼容 SYSTEM |
| 代码复杂度 | ~120 行 winapi 代码 | ~50 行纯 Rust |
| 依赖 | winapi | **rdev**（项目已有依赖） |

**新实现**:
```rust
use rdev::{listen, Event, EventType, Key};

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
                    Key::KeyR => { if shift && alt { crate::run_me::<&str>(vec![]).ok(); } }
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
        rdev::listen(callback).ok();
    });
}
```

### v1.4 → v1.5 (2026-04-29): 零日志 — 代码从未执行 → 移至 `core_main()` 入口

**现象**: 服务正常运行，但日志中无任何 `global_hotkey` 相关输出。

**根因**: v1.4 将热键放在 `server.rs` 的 `start_server()` 中，但 `start_server()` 内部的 `start_hotkey_listener()` 调用在 `RendezvousMediator::start_all()` 前——二者都在 `tokio::main` 生成的 async runtime 上下文中，可能导致线程 spawn 时序问题。

**修复**: 将热键启动移到 `core_main()` 中 `init_log()` **之后**，所有 arg 分发**之前**，用 `std::sync::Once` 保证单次执行。同时从 `server.rs` 中移除调用。

```rust
// core_main.rs, ~第 158 行（init_log 之后）
#[cfg(windows)]
{
    use std::sync::Once;
    static HOTKEY_START: Once = Once::new();
    HOTKEY_START.call_once(|| {
        crate::global_hotkey::start_hotkey_listener();
    });
}
```

| 对比 | v1.4 (`server.rs`) | v1.5 (`core_main()` 入口) |
|------|--------------------|---------------------------|
| 调用时机 | async runtime 内部 | `init_log` 后、arg 分发前 |
| 日志确认 | ❌ 无日志 | ✅ `listener started` 出现 |

### v1.5 → v1.6 (2026-04-29): 热键生效，窗口前置修复

**现象**: 热键能触发（日志可见），但窗口不总是可靠置顶（任务栏闪烁但窗口不弹到最前）。

**根因**: `run_me()` spawn 新进程 → `main.cpp` → `SetForegroundWindow`。Windows 限制新 spawn 进程调用 `SetForegroundWindow` 窃取焦点。

**修复**: 在热键回调中**直接**操作窗口，不再 spawn 进程。新增 `show_or_launch_main_window()` 函数：

| 场景 | 操作 |
|------|------|
| 窗口存在 | `FindWindowW` → `SetWindowPos(HWND_TOPMOST)` → `SetForegroundWindow` → `BringWindowToTop` → `SetWindowPos(HWND_NOTOPMOST)` |
| 窗口不存在 | fallback `crate::run_me()` 启动新实例 |

### v1.6 (restart): 重启后主功能正常，但主窗口关闭后热键失效

**现象**: 主窗口打开时热键正常工作，关闭后热键停止响应。日志显示 `--server` IPC 冲突后进程退出。

**根因**: 
1. **IPC 冲突**：`std::process::exit(-1)` 在 Windows 上杀掉 `--server` 进程（报 "ipc is occupied..."、"kill failed"）
2. `--server` 一死，热键线程随之消亡
3. 主窗口关闭（`SetQuitOnClose(true)`）→ 主窗口进程也退出
4. `--service` (Session 0) 未注册热键 → 全系统无热键

### v1.6 → v1.7 (2026-04-30): 错误尝试 → `run_service()` (Session 0)

**尝试**: 将热键注册移到 `platform/windows.rs` 的 `run_service()` 中。

**结果**: ❌ 完全无效（零日志）。`WH_KEYBOARD_LL` 钩子在 Session 0 中不能拦截用户交互会话的按键。

### v1.7 → v1.8 (2026-04-30): 回退至 `basically-functional` (487287d9) + 修复 `--server` 存活

**修复**:
1. 从 `platform/windows.rs` 的 `run_service()` 中**移除**热键注册
2. 恢复 `core_main()` 中的 `Once` 热键启动（commit 487287d9）
3. **新增** `server.rs` 修复：Windows 上 IPC 失败时不再 `process::exit(-1)`，改为 warn 并继续运行

```rust
// server.rs, start_server(true, false):
#[cfg(not(windows))]
std::process::exit(-1);                     // 保持原行为
#[cfg(windows)]
log::warn!("ipc start failed on Windows, server continues...");  // ★ 新: 不退出
```

| 对比 | v1.7 (Session 0) | v1.8 (`core_main` Once + IPC 容错) |
|------|-------------------|--------------------------------------|
| 热键位置 | `--service` run_service (Session 0) | `core_main()` 入口 (用户会话) |
| `WH_KEYBOARD_LL` | ❌ Session 0 隔离 | ✅ 用户会话 |
| `--server` IPC 失败 | 退出 → 热键丢失 | warn → 继续存活 |
| 主窗口关闭 | 热键丢失 | ✅ `--server` 仍在，热键存活 |

| 版本 | 窗口激活方式 | 结果 |
|------|-------------|------|
| v1.0-v1.5 | `crate::run_me()` spawn 新进程 → `SetForegroundWindow` | ❌ 新进程无焦点权限 |
| **v1.6** | 直接 `FindWindowW` + `HWND_TOPMOST` + `SetForegroundWindow` | ✅ 从当前进程直接操作 |

---

## 运行环境说明

`--server` 进程由 Windows 服务通过 `LaunchProcessWin(session_id, FALSE, ...)` 启动：

| 属性 | 值 |
|------|----|
| 运行身份 | `NT AUTHORITY\SYSTEM` |
| 会话 | 用户交互会话（`session_id` ≠ 0，非 Session 0） |
| 对热键的影响 | `RegisterHotKey` 基于**会话**而非用户身份 — SYSTEM 在用户会话中注册热键是可行的 |

---

## 关键设计决策

| 决策 | 理由 |
|------|------|
| 热键注册在 `core_main()` 入口，`Once` 防重复 | 所有进程（`--service`/`--server`/主窗口）共享该入口；用户会话进程中 `WH_KEYBOARD_LL` 可靠 |
| **`rdev::listen()` + 直接 Win32 窗口操作** | 检测用 `WH_KEYBOARD_LL` 钩子；激活用 `FindWindowW` + `HWND_TOPMOST` 技巧，绕过 spawn 进程的焦点限制 |
| **`--server` IPC 失败不退出**（Windows） | 防止 IPC 竞争导致 `--server` 崩溃带走热键线程 |
| `run_me()` 仅作 fallback | 只在窗口不存在时才 spawn 新进程；窗口存在时直接操作 |
| 条件编译 `#[cfg(windows)]` | macOS/Linux/Android 构建不受影响 |

---

## 构建验证

```powershell
# Windows 构建（需要 vcpkg）
cd rustdesk
python3 build.py --flutter --release
```

热键监听线程在 `--server` 进程启动时自动初始化。可通过任务管理器确认存在 `--server` 进程后，按 `Shift+Alt+R` 测试。

---

## 后续可能的问题

1. **热键冲突**: 如果 `Shift+Alt+R` 已被其他程序注册，`RegisterHotKey` 会失败（日志输出 error），不影响服务功能
2. **管理员权限**: `RegisterHotKey` 无需管理员权限即可工作
3. **远程桌面回话**: 在全屏 RDP 中，热键由客户端处理，不会传递给宿主机
