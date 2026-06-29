# 自定义取消热键 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 让用户在 Settings 里自定义"取消遮罩"的全局热键（含 `F1+Space` 这类任意组合），并持久化到磁盘。

**Architecture:** 弃用 `global-hotkey`（只支持"修饰键+单键"），改用 Windows 低级键盘钩子 `WH_KEYBOARD_LL` 自行检测组合按下；token↔虚拟键码用纯函数实现并单测；配置整体持久化到 `config.json`。

**Tech Stack:** Rust + Tauri 2 + `windows` 0.58 crate；前端 Vue 3 `<script setup>` + TypeScript。

---

## 文件结构

- **Create** `src-tauri/src/config_store.rs` — config 的磁盘读写（`config.json`）。
- **Modify** `src-tauri/src/hotkey.rs` — 重写：纯函数（token↔VK、组合匹配）+ 低级键盘钩子。
- **Modify** `src-tauri/src/lib.rs` — 启动时加载 config、装钩子、逃逸线程改走 mpsc channel。
- **Modify** `src-tauri/src/commands.rs` — `save_config` 增加持久化 + 更新活动组合。
- **Modify** `src-tauri/Cargo.toml` — 移除 `global-hotkey`，新增 `Win32_System_LibraryLoader` feature。
- **Modify** `src/windows/settings/SettingsApp.vue` — 3 个可编辑按键框 + 捕获逻辑。

每个任务结束都能编译并通过测试，独立提交。

---

## Task 1: 配置持久化

**Files:**
- Create: `src-tauri/src/config_store.rs`
- Modify: `src-tauri/src/lib.rs`（声明模块、启动加载、save 在 Task 3 接入）

- [ ] **Step 1: 写失败测试（serde round-trip + 解析回落）**

新建 `src-tauri/src/config_store.rs`，先只放测试：

```rust
use crate::state::DaemonConfig;
use std::path::PathBuf;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_then_load_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        let mut cfg = DaemonConfig::default();
        cfg.hotkey = "f1+space".to_string();
        cfg.interval_secs = 999;
        save_to(&path, &cfg).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded.hotkey, "f1+space");
        assert_eq!(loaded.interval_secs, 999);
    }

    #[test]
    fn load_from_missing_file_is_none() {
        let path = PathBuf::from("definitely/does/not/exist/config.json");
        assert!(load_from(&path).is_none());
    }

    #[test]
    fn load_from_garbage_is_none() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("config.json");
        std::fs::write(&path, "not json {{{").unwrap();
        assert!(load_from(&path).is_none());
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml config_store`
Expected: 编译失败（`save_to`/`load_from` 未定义）。

- [ ] **Step 3: 实现 config_store**

在 `src-tauri/src/config_store.rs` 顶部（测试模块之上）加入：

```rust
fn config_path() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("blink-rs")
        .join("config.json")
}

/// Load persisted config from the default path, or None if absent/corrupt.
pub fn load() -> Option<DaemonConfig> {
    load_from(&config_path())
}

/// Persist config to the default path.
pub fn save(config: &DaemonConfig) -> Result<(), String> {
    save_to(&config_path(), config)
}

fn load_from(path: &PathBuf) -> Option<DaemonConfig> {
    let data = std::fs::read_to_string(path).ok()?;
    match serde_json::from_str::<DaemonConfig>(&data) {
        Ok(cfg) => Some(cfg),
        Err(e) => {
            log::warn!("config.json parse failed, using defaults: {e}");
            None
        }
    }
}

fn save_to(path: &PathBuf, config: &DaemonConfig) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(path, json).map_err(|e| e.to_string())
}
```

- [ ] **Step 4: 声明模块**

在 `src-tauri/src/lib.rs` 顶部模块声明区（第 1–9 行附近，与 `mod commands;` 并列）加入：

```rust
mod config_store;
```

- [ ] **Step 5: 启动时加载 config**

在 `src-tauri/src/lib.rs::run()` 中，把：

```rust
    let mut config = DaemonConfig::default();
```

改为：

```rust
    let mut config = config_store::load().unwrap_or_default();
```

（其后的 `BLINK_DEV_INTERVAL` 覆盖逻辑保持不变，文件值会被 dev 环境变量覆盖。）

- [ ] **Step 6: 跑测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml config_store`
Expected: 3 个测试 PASS。

- [ ] **Step 7: 确认整体编译**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功（仍保留 global-hotkey，不受影响）。

- [ ] **Step 8: 提交**

```bash
git add src-tauri/src/config_store.rs src-tauri/src/lib.rs
git commit -m "feat(config): persist DaemonConfig to config.json"
```

---

## Task 2: 热键纯函数（token ↔ VK、组合匹配）

**Files:**
- Modify: `src-tauri/src/hotkey.rs`（新增纯函数与单测，暂保留旧 `HotkeyManager`）

- [ ] **Step 1: 写失败测试**

把 `src-tauri/src/hotkey.rs` 末尾的 `#[cfg(test)] mod tests { ... }` 整块替换为：

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn token_to_vk_known_keys() {
        assert_eq!(token_to_vk("ctrl"), Some(0x11));
        assert_eq!(token_to_vk("shift"), Some(0x10));
        assert_eq!(token_to_vk("alt"), Some(0x12));
        assert_eq!(token_to_vk("win"), Some(0x5B));
        assert_eq!(token_to_vk("space"), Some(0x20));
        assert_eq!(token_to_vk("esc"), Some(0x1B));
        assert_eq!(token_to_vk("a"), Some(0x41));
        assert_eq!(token_to_vk("z"), Some(0x5A));
        assert_eq!(token_to_vk("0"), Some(0x30));
        assert_eq!(token_to_vk("9"), Some(0x39));
        assert_eq!(token_to_vk("f1"), Some(0x70));
        assert_eq!(token_to_vk("f12"), Some(0x7B));
    }

    #[test]
    fn token_to_vk_unknown_is_none() {
        assert_eq!(token_to_vk(""), None);
        assert_eq!(token_to_vk("f13"), None);
        assert_eq!(token_to_vk("ab"), None);
        assert_eq!(token_to_vk("ctrlx"), None);
    }

    #[test]
    fn normalize_vk_folds_left_right() {
        assert_eq!(normalize_vk(0xA2), 0x11); // LCtrl -> Ctrl
        assert_eq!(normalize_vk(0xA3), 0x11); // RCtrl -> Ctrl
        assert_eq!(normalize_vk(0xA0), 0x10); // LShift -> Shift
        assert_eq!(normalize_vk(0xA5), 0x12); // RAlt -> Alt
        assert_eq!(normalize_vk(0x5C), 0x5B); // RWin -> Win
        assert_eq!(normalize_vk(0x70), 0x70); // F1 unchanged
    }

    #[test]
    fn parse_combo_dedupes_and_normalizes() {
        assert_eq!(parse_combo("ctrl+shift+space"), vec![0x11, 0x10, 0x20]);
        assert_eq!(parse_combo("f1+space"), vec![0x70, 0x20]);
        assert_eq!(parse_combo("esc"), vec![0x1B]);
        // empty / whitespace / unknown tokens are ignored
        assert_eq!(parse_combo(""), Vec::<u32>::new());
        assert_eq!(parse_combo("ctrl + ctrl"), vec![0x11]);
        assert_eq!(parse_combo("ctrl+bogus+space"), vec![0x11, 0x20]);
    }

    #[test]
    fn combo_matches_requires_all_keys_and_nonempty() {
        let mut pressed = HashSet::new();
        pressed.insert(0x70u32); // F1
        pressed.insert(0x20u32); // Space
        assert!(combo_matches(&pressed, &[0x70, 0x20]));
        assert!(combo_matches(&pressed, &[0x70]));
        assert!(!combo_matches(&pressed, &[0x70, 0x11])); // Ctrl not pressed
        assert!(!combo_matches(&pressed, &[])); // empty combo never matches
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --manifest-path src-tauri/Cargo.toml hotkey`
Expected: 编译失败（`token_to_vk` 等未定义）。

- [ ] **Step 3: 实现纯函数**

把 `src-tauri/src/hotkey.rs` **文件最顶部**（在现有 `use global_hotkey::...` 之前）加入：

```rust
use std::collections::HashSet;

/// Map a hotkey token (e.g. "ctrl", "space", "f1", "a") to a Windows virtual-key
/// code. Returns None for unknown tokens. Uses numeric VK constants so this stays
/// cross-platform and unit-testable without the `windows` crate.
pub fn token_to_vk(token: &str) -> Option<u32> {
    Some(match token {
        "ctrl" => 0x11,
        "shift" => 0x10,
        "alt" => 0x12,
        "win" => 0x5B,
        "space" => 0x20,
        "esc" => 0x1B,
        "enter" => 0x0D,
        "tab" => 0x09,
        "left" => 0x25,
        "up" => 0x26,
        "right" => 0x27,
        "down" => 0x28,
        t if t.len() == 1 && t.as_bytes()[0].is_ascii_lowercase() => {
            (t.as_bytes()[0] - b'a') as u32 + 0x41
        }
        t if t.len() == 1 && t.as_bytes()[0].is_ascii_digit() => {
            (t.as_bytes()[0] - b'0') as u32 + 0x30
        }
        t if t.starts_with('f') && t.len() >= 2 => {
            let n: u32 = t[1..].parse().ok()?;
            if (1..=12).contains(&n) {
                0x70 + (n - 1)
            } else {
                return None;
            }
        }
        _ => return None,
    })
}

/// Fold left/right modifier VK codes onto their generic equivalent so the hook
/// can compare against a layout-independent combo.
pub fn normalize_vk(vk: u32) -> u32 {
    match vk {
        0xA2 | 0xA3 => 0x11, // L/R Ctrl  -> Ctrl
        0xA0 | 0xA1 => 0x10, // L/R Shift -> Shift
        0xA4 | 0xA5 => 0x12, // L/R Alt   -> Alt
        0x5C => 0x5B,        // R Win     -> Win
        other => other,
    }
}

/// Parse a `+`-joined hotkey string into a deduped, normalized list of VK codes.
/// Unknown/empty tokens are skipped.
pub fn parse_combo(hotkey: &str) -> Vec<u32> {
    let mut out = Vec::new();
    for tok in hotkey.split('+') {
        let tok = tok.trim();
        if tok.is_empty() {
            continue;
        }
        if let Some(vk) = token_to_vk(tok) {
            let n = normalize_vk(vk);
            if !out.contains(&n) {
                out.push(n);
            }
        }
    }
    out
}

/// True when the combo is non-empty and every key in it is currently pressed.
pub fn combo_matches(pressed: &HashSet<u32>, combo: &[u32]) -> bool {
    !combo.is_empty() && combo.iter().all(|k| pressed.contains(k))
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --manifest-path src-tauri/Cargo.toml hotkey`
Expected: 5 个测试 PASS（旧 `HotkeyManager` 仍在，编译通过）。

- [ ] **Step 5: 提交**

```bash
git add src-tauri/src/hotkey.rs
git commit -m "feat(hotkey): pure token<->VK + combo matching functions"
```

---

## Task 3: 低级键盘钩子 + 后端接线（移除 global-hotkey）

**Files:**
- Modify: `src-tauri/Cargo.toml`
- Modify: `src-tauri/src/hotkey.rs`（删除旧 `HotkeyManager`/`is_escape_event`，加钩子）
- Modify: `src-tauri/src/lib.rs`（装钩子、逃逸线程改 channel、初始化组合）
- Modify: `src-tauri/src/commands.rs`（save_config 更新组合 + 持久化）

- [ ] **Step 1: 改 Cargo.toml**

在 `src-tauri/Cargo.toml` 中删除这一行：

```toml
global-hotkey = "0.5"
```

并在 `[target.'cfg(target_os = "windows")'.dependencies]` 的 `windows` features 列表中加入一行（放在 `"Win32_Foundation",` 后面即可）：

```toml
  "Win32_System_LibraryLoader",
```

- [ ] **Step 2: 重写 hotkey.rs 的钩子部分**

删除 `src-tauri/src/hotkey.rs` 中旧的 `use global_hotkey::...`、`struct HotkeyManager`、`impl HotkeyManager`、`pub fn is_escape_event(...)`（即 Task 2 新增纯函数之外、测试模块之前的所有旧代码）。在纯函数之后、测试模块之前，加入：

```rust
#[cfg(target_os = "windows")]
mod hook {
    use super::{combo_matches, normalize_vk};
    use std::collections::HashSet;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::mpsc::Sender;
    use std::sync::{Mutex, OnceLock};

    use windows::Win32::Foundation::{HINSTANCE, LPARAM, LRESULT, WPARAM};
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallNextHookEx, GetMessageW, SetWindowsHookExW, HHOOK, KBDLLHOOKSTRUCT, MSG,
        WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
    };

    static ACTIVE_COMBO: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static ARMED: AtomicBool = AtomicBool::new(true);
    static ESCAPE_TX: OnceLock<Sender<()>> = OnceLock::new();
    static PRESSED: OnceLock<Mutex<HashSet<u32>>> = OnceLock::new();

    fn pressed() -> &'static Mutex<HashSet<u32>> {
        PRESSED.get_or_init(|| Mutex::new(HashSet::new()))
    }

    pub fn set_combo(combo: Vec<u32>) {
        *ACTIVE_COMBO.lock().unwrap() = combo;
    }

    /// Install the global low-level keyboard hook on a dedicated message-pump
    /// thread. `tx` receives one `()` each time the active combo is pressed.
    pub fn install(tx: Sender<()>) {
        let _ = ESCAPE_TX.set(tx);
        std::thread::spawn(|| unsafe {
            let hmod = GetModuleHandleW(None).unwrap_or_default();
            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_hook_proc),
                HINSTANCE(hmod.0),
                0,
            );
            if hook.is_err() {
                log::error!("SetWindowsHookExW failed: {:?}", hook.err());
                return;
            }
            // Low-level hooks require a message pump on the installing thread.
            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
        });
    }

    unsafe extern "system" fn keyboard_hook_proc(
        code: i32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if code >= 0 {
            let kb = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
            let vk = normalize_vk(kb.vkCode);
            match wparam.0 as u32 {
                WM_KEYDOWN | WM_SYSKEYDOWN => {
                    let fire = {
                        let mut p = pressed().lock().unwrap();
                        p.insert(vk);
                        let combo = ACTIVE_COMBO.lock().unwrap();
                        ARMED.load(Ordering::Relaxed) && combo_matches(&p, &combo)
                    };
                    if fire {
                        ARMED.store(false, Ordering::Relaxed);
                        if let Some(tx) = ESCAPE_TX.get() {
                            let _ = tx.send(());
                        }
                    }
                }
                WM_KEYUP | WM_SYSKEYUP => {
                    let mut p = pressed().lock().unwrap();
                    p.remove(&vk);
                    // Re-arm once any combo key is released (edge-triggered).
                    if ACTIVE_COMBO.lock().unwrap().contains(&vk) {
                        ARMED.store(true, Ordering::Relaxed);
                    }
                }
                _ => {}
            }
        }
        CallNextHookEx(HHOOK(std::ptr::null_mut()), code, wparam, lparam)
    }
}

#[cfg(target_os = "windows")]
pub use hook::{install, set_combo};

/// Non-Windows stubs so the crate still compiles elsewhere.
#[cfg(not(target_os = "windows"))]
pub fn install(_tx: std::sync::mpsc::Sender<()>) {}

#[cfg(not(target_os = "windows"))]
pub fn set_combo(_combo: Vec<u32>) {}
```

- [ ] **Step 3: 重写 lib.rs 的热键线程**

lib.rs 对 global-hotkey 的唯一引用在热键线程内部（`global_hotkey::GlobalHotKeyEvent::receiver()` 等），顶部没有需要删除的 `use`。把 `setup` 闭包里现有的整段「Hotkey listener thread」（从注释 `// Hotkey listener thread` 到该 `std::thread::spawn(move || { ... });` 结束，约 155–190 行）整体替换为：

```rust
            // Global escape combo via low-level keyboard hook (hotkey.rs).
            let (esc_tx, esc_rx) = std::sync::mpsc::channel::<()>();
            hotkey::install(esc_tx);
            hotkey::set_combo(hotkey::parse_combo(
                &cfg_for_hotkey.lock().unwrap().hotkey,
            ));

            let app_handle2 = app.handle().clone();
            let state_for_hotkey = state_clone.clone();
            let db_for_hotkey = db_clone.clone();
            std::thread::spawn(move || {
                while esc_rx.recv().is_ok() {
                    let outcome = {
                        let mut s = state_for_hotkey.lock().unwrap();
                        if matches!(s.phase, Phase::Monitoring | Phase::Dissolving) {
                            continue; // nothing to escape in these phases
                        }
                        let outcome = daemon::escape_outcome(&s.phase);
                        s.transition_to(Phase::Monitoring);
                        outcome
                    };
                    if let Ok(db) = db_for_hotkey.lock() {
                        db.record_session(&outcome).ok();
                    }
                    let _ = app_handle2.emit("blink://phase", Phase::Monitoring);
                    if let Some(overlay) = app_handle2.get_webview_window("overlay") {
                        let _ = overlay.hide();
                    }
                }
            });
```

- [ ] **Step 4: 在 lib.rs 提供 cfg_for_hotkey 句柄**

在 `src-tauri/src/lib.rs::run()` 中，`tauri::Builder::default()` 调用**之前**（紧跟 `let state_clone = shared_state.clone();` 那几行附近）加入：

```rust
    let cfg_for_hotkey = shared_config.clone();
```

并把该 `cfg_for_hotkey` 通过 `setup(move |app| { ... })` 闭包捕获（闭包已是 `move`，直接在内部使用即可）。

- [ ] **Step 5: 更新 commands.rs 的 save_config**

把 `src-tauri/src/commands.rs` 的 `save_config` 整个函数替换为：

```rust
#[tauri::command]
pub fn save_config(
    new_config: DaemonConfig,
    config: State<AppConfig>,
    shared: State<AppSharedState>,
) -> Result<(), String> {
    let auto_start = new_config.auto_start;
    *config.0.lock().unwrap() = new_config.clone();
    shared.0.lock().unwrap().config = new_config.clone();
    // Push the new combo to the live keyboard hook and persist to disk.
    crate::hotkey::set_combo(crate::hotkey::parse_combo(&new_config.hotkey));
    crate::config_store::save(&new_config)?;
    crate::startup::set_auto_start(auto_start).map_err(|e| e.to_string())
}
```

- [ ] **Step 6: 编译**

Run: `cargo build --manifest-path src-tauri/Cargo.toml`
Expected: 编译成功，且不再有 `global_hotkey` 相关引用。若报 `HINSTANCE`/`GetModuleHandleW` 未找到，确认 Step 1 的 `Win32_System_LibraryLoader` feature 已加。

- [ ] **Step 7: 跑全部 Rust 测试**

Run: `cargo test --manifest-path src-tauri/Cargo.toml`
Expected: 全部 PASS（hotkey 纯函数 + config_store + 既有测试）。

- [ ] **Step 8: 提交**

```bash
git add src-tauri/Cargo.toml src-tauri/Cargo.lock src-tauri/src/hotkey.rs src-tauri/src/lib.rs src-tauri/src/commands.rs
git commit -m "feat(hotkey): detect escape combo via WH_KEYBOARD_LL, drop global-hotkey"
```

---

## Task 4: 前端可编辑按键框

**Files:**
- Modify: `src/windows/settings/SettingsApp.vue`

- [ ] **Step 1: 替换模板中的热键区块**

把 `src/windows/settings/SettingsApp.vue` 中 "Hotkey" 区块（`<!-- Hotkey -->` 到对应 `</section>`，约 54–65 行）替换为：

```html
      <!-- Hotkey -->
      <section class="section">
        <div class="label-caps">ESCAPE HOTKEY</div>
        <div class="card">
          <p class="field-label" style="margin-bottom:8px">
            Skip intervention · double-click a box to edit, leave empty to remove
          </p>
          <div class="keys-row">
            <template v-for="(slot, i) in slots" :key="i">
              <span v-if="i > 0" class="plus">+</span>
              <kbd
                class="key-box"
                :class="{ editing: editingIndex === i, empty: !slot && editingIndex !== i }"
                tabindex="0"
                @dblclick="startEdit(i, $event)"
                @keydown="onKeyCapture($event, i)"
                @blur="stopEdit(i)"
              >{{ editingIndex === i ? '…' : (slot ? keyLabel(slot) : '—') }}</kbd>
            </template>
          </div>
        </div>
      </section>
```

- [ ] **Step 2: 增加脚本逻辑**

在 `<script setup>` 中，`const intervalMinutes = ...` 之后加入：

```ts
// --- Hotkey editing: 3 positional slots, each a token or '' (removed) ---
const slots = reactive<string[]>(['', '', '']);
const editingIndex = ref<number | null>(null);

const KEY_LABELS: Record<string, string> = {
  ctrl: 'Ctrl', shift: 'Shift', alt: 'Alt', win: 'Win', space: 'Space',
  esc: 'Esc', enter: 'Enter', tab: 'Tab', left: '←', right: '→', up: '↑', down: '↓',
};
function keyLabel(tok: string): string {
  return KEY_LABELS[tok] ?? tok.toUpperCase();
}

// Physical key (event.code) -> our token. Layout-independent.
function codeToToken(code: string): string | null {
  if (code === 'ControlLeft' || code === 'ControlRight') return 'ctrl';
  if (code === 'ShiftLeft' || code === 'ShiftRight') return 'shift';
  if (code === 'AltLeft' || code === 'AltRight') return 'alt';
  if (code === 'MetaLeft' || code === 'MetaRight') return 'win';
  if (code === 'Space') return 'space';
  if (code === 'Escape') return 'esc';
  if (code === 'Enter') return 'enter';
  if (code === 'Tab') return 'tab';
  if (code === 'ArrowLeft') return 'left';
  if (code === 'ArrowRight') return 'right';
  if (code === 'ArrowUp') return 'up';
  if (code === 'ArrowDown') return 'down';
  if (/^Key[A-Z]$/.test(code)) return code.slice(3).toLowerCase();
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-2])$/.test(code)) return code.toLowerCase();
  return null;
}

function startEdit(i: number, e: MouseEvent) {
  editingIndex.value = i;
  slots[i] = ''; // clear while waiting for input
  (e.currentTarget as HTMLElement).focus();
}
function stopEdit(i: number) {
  // Blur with nothing captured leaves the slot empty (= key removed).
  if (editingIndex.value === i) editingIndex.value = null;
}
function onKeyCapture(e: KeyboardEvent, i: number) {
  if (editingIndex.value !== i) return;
  e.preventDefault();
  const tok = codeToToken(e.code);
  if (tok) slots[i] = tok;
  editingIndex.value = null;
  (e.target as HTMLElement).blur();
}

function loadSlotsFromConfig() {
  const parts = (config.hotkey || '').split('+').map((s) => s.trim()).filter(Boolean);
  slots[0] = parts[0] ?? '';
  slots[1] = parts[1] ?? '';
  slots[2] = parts[2] ?? '';
}
```

- [ ] **Step 3: 初始化 slots 并在保存时序列化**

在 `onMounted` 里 `Object.assign(config, remote);` 之后加入：

```ts
  loadSlotsFromConfig();
```

把 `save()` 函数体首行改为先序列化热键：

```ts
async function save() {
  config.hotkey = slots.filter(Boolean).join('+');
  await saveConfig({ ...config });
  saveFlash.value = true;
  setTimeout(() => { saveFlash.value = false; }, 2000);
  sendNotification({ title: 'Blink', body: 'Settings saved.' });
}
```

- [ ] **Step 4: 增加按键框样式**

在 `<style scoped>` 中 `kbd { ... }` 规则之后加入：

```css
.key-box {
  cursor: pointer;
  min-width: 34px;
  text-align: center;
  outline: none;
  user-select: none;
}
.key-box:hover { border-color: rgba(137, 180, 250, 0.5); }
.key-box.editing {
  border-color: #89b4fa;
  color: #89b4fa;
  box-shadow: 0 0 0 2px rgba(137, 180, 250, 0.25);
}
.key-box.empty { color: #45475a; border-style: dashed; }
```

- [ ] **Step 5: 类型检查 + 构建前端**

Run: `npm run build`
Expected: `vue-tsc` 无类型错误，`vite build` 成功。

- [ ] **Step 6: 提交**

```bash
git add src/windows/settings/SettingsApp.vue
git commit -m "feat(settings): editable 3-slot escape hotkey with key capture"
```

---

## Task 5: 端到端手动验证

**Files:** 无（运行验证）

- [ ] **Step 1: 以短计时启动**

Run（git bash）: `BLINK_DEV_INTERVAL=8 npm run tauri dev`
Expected: 应用启动，托盘出现 Blink 图标；约 8 秒后遮罩开始扩散。

- [ ] **Step 2: 验证默认热键仍生效**

遮罩出现时按 `Ctrl+Shift+Space`。
Expected: 遮罩立即消失，回到监听期。

- [ ] **Step 3: 改热键并验证**

右键托盘 → Settings → 双击第一个框、按 `F1` → 双击第二个框不输入（清空 Shift）→ 第三个框保持 Space → Save。等下一次遮罩出现，按 `F1+Space`。
Expected: 遮罩消失；按旧的 `Ctrl+Shift+Space` 不再起作用。

- [ ] **Step 4: 验证持久化**

关闭应用（托盘 → Quit），重新 `npm run tauri dev`，打开 Settings。
Expected: 热键框仍显示 `F1 + — + Space`（即上次保存的 `f1+space`）。

- [ ] **Step 5: 收尾**

确认无异常日志；如需把 dev 期间的临时改动还原则还原。功能完成。
```

---

## 自查

- **Spec 覆盖**：数据模型→Task 2/4；前端 3 框/双击/留空删除→Task 4；后端钩子/只观察不吞键/边沿触发/ACTIVE_COMBO 热更新→Task 3；token↔VK 归一化纯函数→Task 2；持久化 config.json→Task 1 + Task 3 Step 5；测试→Task 1/2 + Task 5；移除 global-hotkey→Task 3。全部覆盖。
- **类型一致**：`parse_combo`/`set_combo`/`combo_matches`/`token_to_vk`/`normalize_vk`/`install` 在 Task 2/3/save_config 中签名一致；前端 `slots`/`editingIndex`/`codeToToken`/`keyLabel` 命名一致。
- **无占位符**：每个改动步骤都给出完整代码与确切命令。
