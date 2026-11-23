> **已取代（2026-11-23）**：用户已明确撤销关闭快捷键功能。以下仅保留历史记录，不再作为实施要求。当前退出方式为双击归位黑板擦或点击独立 ×；修饰键仅用于临时持擦。现行规范见 [Blink v1](../../blink-v1-spec.md)。

# 自定义取消热键（Custom Escape Hotkey）设计

**日期:** 2026-06-29
**分支:** `feat/custom-escape-hotkey`
**状态:** 设计待评审

## 背景与问题

PRD §5.3 承诺"支持用户自定义全局快捷键"一键取消遮罩，但当前实现**端到端都没接上线**：

- 前端 `SettingsApp.vue` 的 "ESCAPE HOTKEY" 区块是写死的静态 `<kbd>` 标签，没有任何可编辑控件。
- 后端 `hotkey.rs` 把 `Ctrl+Shift+Space` 硬编码进 `HotKey::new(...)`，从不读取 `DaemonConfig.hotkey`。
- `save_config` 只更新内存配置，没有任何重新注册热键的通路。

结果：用户无法更改取消按键。本设计补全该功能。

## 关键约束与取舍

Windows 系统级全局热键（`global-hotkey` 库 / `RegisterHotKey`）只接受 **0~N 个修饰键（Ctrl/Shift/Alt/Win）+ 恰好一个普通键**。因此 `F1 + Space`（两个普通键）这类组合无法注册成系统全局热键。

用户明确要求支持**任意键放在任意框**（含 `F1+Space`），因此选定方案：**改用低级键盘钩子 `WH_KEYBOARD_LL` 自行检测组合**。

**已知取舍（用户已确认接受）:**
- 全局键盘钩子可能被部分杀毒软件误判为键盘记录器（PRD §3.3 提示）。
- 钩子**只观察、不吞键**：组合命中时触发逃逸，同时按键照常传递给前台程序，不影响正常打字。

## 数据模型

- 热键 = **有序的按键槽列表**，每槽是一个 token，空槽不参与。
- `DaemonConfig.hotkey: String` 字段类型不变，用 `+` 连接非空槽序列化，例如 `"ctrl+shift+space"`、`"f1+space"`、`"esc"`；空组合序列化为 `""`（= 不设取消热键）。
- token 词表：`ctrl` `shift` `alt` `win` `space` `esc` `enter` `tab` `up` `down` `left` `right` `a`–`z` `0`–`9` `f1`–`f12`。
- 触发语义：组合内**所有**键同时按住即触发一次；松开任一键后才能再次触发（防连发）。
- 去重：重复 token 视为一个键。
- 空组合：不注册任何取消热键（手动逃逸被禁用，定时退散不受影响）。

## 前端（`SettingsApp.vue`）

- "ESCAPE HOTKEY" 区块改为**固定 3 个位置框**，绑定到本地的 `slots: [string, string, string]`（由 `config.hotkey` 拆分初始化）。
- 空槽显示为虚线占位框，提示可双击添加。
- **双击某框** → 该框进入"捕获"态（清空、显示"按下按键…"），监听一次性 `keydown`。
  - 用 `KeyboardEvent.code` 做布局无关识别，`preventDefault` 防止键被前台消费；映射成 token 填入该槽，退出捕获态。
  - 捕获态下**失焦/点击别处/再双击其他框**而未按键 → 该槽保持为空（= 删除此键）。
- 保存：把 3 个槽里非空 token 按序用 `+` 拼成字符串，写入 `config.hotkey`，走现有 `saveConfig`。
- 最多 3 键组合（贴合现有 UI 与用户预期）。

## 后端

### `hotkey.rs`（重写）
- 弃用 `global-hotkey` 依赖。
- 新增 `Win32_UI_Input_KeyboardAndMouse` feature 以使用虚拟键码（VK）。
- 纯函数（便于单测）：
  - `token_to_vk(token: &str) -> Option<u32>`：token → VK 码。
  - `normalize_vk(vk: u32) -> u32`：左右 Ctrl/Shift/Alt/Win 归一化到通用 VK。
  - `parse_combo(hotkey: &str) -> Vec<u32>`：配置字符串 → 归一化 VK 集合（去重、忽略未知 token）。
  - `combo_matches(pressed: &HashSet<u32>, combo: &[u32]) -> bool`：组合是否全部按下（combo 非空且为 pressed 子集）。
- 钩子线程：专用线程内 `SetWindowsHookExW(WH_KEYBOARD_LL, proc, ..)` + `GetMessageW` 消息循环。
- 共享状态（`static`，钩子回调可达）：
  - `ACTIVE_COMBO: Mutex<Vec<u32>>`——当前组合 VK 列表，`save_config` 更新，钩子下次事件即生效，**无需重装**。
  - `PRESSED: Mutex<HashSet<u32>>`——当前按下的归一化 VK。
  - `ARMED: AtomicBool`——边沿触发标志，命中后置 false，松开任一组合键后复位 true。
  - `ESCAPE_TX: OnceLock<Sender<()>>`——回调命中后发信号。
- 回调逻辑：按键按下/松开更新 `PRESSED`；若 `ARMED && combo_matches(...)` → 触发（发 `()` 到 channel）并 `ARMED=false`；任一组合键松开 → `ARMED=true`。始终 `CallNextHookEx`。

### `lib.rs`
- 逃逸处理线程改为从 `ESCAPE_TX` 对应的 receiver 收信号，沿用现有逻辑（仅在 `Warning/Spreading/Holding` 阶段切回 `Monitoring` + 隐藏 overlay + `record_session`）。
- 启动时把当前 `config.hotkey` 解析进 `ACTIVE_COMBO`。

### `commands.rs`
- `save_config`：更新内存 config + 把新 `hotkey` 解析进 `ACTIVE_COMBO` + **写盘持久化** + 现有 auto_start 逻辑。

## 配置持久化（新增）

- 路径：`%LOCALAPPDATA%/blink-rs/config.json`（与 `sessions.db` 同目录）。
- 启动（`lib.rs::run`）：若文件存在则反序列化为 `DaemonConfig` 覆盖默认值；解析失败则回落默认并记日志。`BLINK_DEV_INTERVAL` 覆盖仍优先于文件。
- `save_config`：序列化整个 `DaemonConfig` 到该文件（`serde_json`，已在依赖中）。
- 不引入新依赖。

## 测试

- 纯函数单测：`token_to_vk`、`normalize_vk`（左右键归一化）、`parse_combo`（含未知 token、去重、空串）、`combo_matches`（子集/超集/空组合）。
- 配置往返：`DaemonConfig` serde JSON 序列化/反序列化 round-trip。
- 钩子本体需桌面会话，沿用现有"能编译即过"的占位测试。

## 影响面

- 删除依赖：`global-hotkey`。
- 改动文件：`Cargo.toml`、`hotkey.rs`（重写）、`lib.rs`、`commands.rs`、`SettingsApp.vue`；新增配置持久化代码（可置于 `state.rs` 或新建 `config_store.rs`）。
- 跨平台：钩子为 Windows 专属，`#[cfg(target_os = "windows")]` 隔离，非 Windows 提供空实现以保证编译（与 `capture.rs` 一致）。

## 非目标（Out of Scope）

- 按键框数量动态增减（固定 3 个）。
- 修饰键左右区分（L/R 归一化为同一键）。
- PRD §5.3 的"信用透支/负债机制"（与本功能无关）。
