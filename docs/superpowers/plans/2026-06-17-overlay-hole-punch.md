# Overlay Hole-Punch Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the eye-protection overlay's center genuinely operable by cutting a real hole in the window with `SetWindowRgn`, animated to grow the blur frame inward (spreading), fully cover at holding, then reopen (dissolving).

**Architecture:** A single fullscreen overlay window. Rust computes a per-phase rounded-rect hole fraction (`DaemonState::overlay_hole`) and a background thread applies it via `SetWindowRgn` ~50×/sec. Where the hole is, the window does not exist — clicks reach the desktop, raw desktop shows through. The Vue overlay renders a uniform blur (clipped by the OS hole), a feather ring at the boundary, edge circuit traces, and the center rest text — all driven by the same phase + easing so they stay aligned at any DPI.

**Tech Stack:** Rust, Tauri v2, `windows` crate 0.58 (Win32 GDI), Vue 3 + TypeScript.

> **Repo note:** This project is **not** a git repo. Either run `git init` first if you want the commit steps to work, or skip the `git commit` steps and treat each task's completion as the checkpoint. The plan keeps commit steps for completeness.

---

## File Structure

- `src-tauri/Cargo.toml` — add `Win32_Graphics_Gdi` to the existing `windows` crate features.
- `src-tauri/src/state.rs` — replace `OverlayInteract`/`overlay_interact()` with `OverlayHole` + a pure `hole_for()` helper and `overlay_hole()` wrapper. Unit-tested.
- `src-tauri/src/lib.rs` — replace the `set_ignore_cursor_events` poller thread with the `SetWindowRgn` region animator; swap the `OverlayInteract` import for `OverlayHole`.
- `src/windows/overlay/BlurMask.vue` — remove the `mask-image` operability hack; uniform blur with phase-driven opacity, a feather ring at the hole boundary, 👁 text only during holding.
- `src/windows/overlay/CircuitTraces.vue` — no change (already renders edge traces during warning + spreading); verified at runtime.

---

## Task 1: Add the GDI feature to the windows crate

**Files:**
- Modify: `src-tauri/Cargo.toml:28-37`

- [ ] **Step 1: Add the GDI feature**

In `src-tauri/Cargo.toml`, the `windows` dependency already lists Win32 features. Add `"Win32_Graphics_Gdi"`:

```toml
[target.'cfg(target_os = "windows")'.dependencies]
windows = { version = "0.58", features = [
  "Win32_UI_Shell",
  "Win32_System_Power",
  "Win32_System_Com",
  "Win32_Media_Audio",
  "Win32_Media_Audio_Endpoints",
  "Win32_Foundation",
  "Win32_System_Registry",
  "Win32_UI_WindowsAndMessaging",
  "Win32_Graphics_Gdi",
] }
```

- [ ] **Step 2: Verify it resolves**

Run: `cd src-tauri && cargo build --lib 2>&1 | tail -20`
Expected: builds (existing code still compiles; new feature available). Warnings are fine.

- [ ] **Step 3: Commit**

```bash
git add src-tauri/Cargo.toml
git commit -m "build: enable Win32_Graphics_Gdi feature for window regions"
```

---

## Task 2: Replace OverlayInteract with OverlayHole geometry (TDD)

**Files:**
- Modify: `src-tauri/src/state.rs` (remove `OverlayInteract` enum + `overlay_interact()` method added previously; add `OverlayHole`, `hole_for()`, `overlay_hole()`)
- Test: `src-tauri/src/state.rs` (`#[cfg(test)] mod tests`)

- [ ] **Step 1: Write the failing tests**

Add these tests inside the existing `mod tests` block in `src-tauri/src/state.rs`:

```rust
    fn frac(h: OverlayHole) -> f64 {
        match h {
            OverlayHole::Frame(f) => f,
            other => panic!("expected Frame, got {:?}", other),
        }
    }

    #[test]
    fn hole_hidden_when_monitoring() {
        let cfg = DaemonConfig::default();
        assert_eq!(hole_for(&Phase::Monitoring, 0.0, &cfg), OverlayHole::Hidden);
    }

    #[test]
    fn hole_full_when_holding() {
        let cfg = DaemonConfig::default();
        assert_eq!(hole_for(&Phase::Holding, 5.0, &cfg), OverlayHole::Full);
    }

    #[test]
    fn hole_large_static_during_warning() {
        let cfg = DaemonConfig::default();
        assert!((frac(hole_for(&Phase::Warning, 3.0, &cfg)) - 0.92).abs() < 1e-9);
    }

    #[test]
    fn hole_spreads_closed_over_spreading() {
        let cfg = DaemonConfig::default();
        // start of spreading: hole is large
        assert!((frac(hole_for(&Phase::Spreading, 0.0, &cfg)) - 0.92).abs() < 1e-9);
        // end of spreading: hole fully closed
        assert!(frac(hole_for(&Phase::Spreading, cfg.spreading_secs as f64, &cfg)).abs() < 1e-9);
    }

    #[test]
    fn hole_reopens_over_dissolving() {
        let cfg = DaemonConfig::default();
        // start of dissolving: closed
        assert!(frac(hole_for(&Phase::Dissolving, 0.0, &cfg)).abs() < 1e-9);
        // end of dissolving: large again
        assert!((frac(hole_for(&Phase::Dissolving, cfg.dissolving_secs as f64, &cfg)) - 0.92).abs() < 1e-9);
    }
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cd src-tauri && cargo test hole 2>&1 | tail -20`
Expected: FAIL to compile — `OverlayHole` and `hole_for` not defined.

- [ ] **Step 3: Remove the old OverlayInteract code**

In `src-tauri/src/state.rs`, delete the previously added `overlay_interact()` method (the one returning `OverlayInteract`) and the `OverlayInteract` enum definition. Leave the rest of `DaemonState` intact.

- [ ] **Step 4: Add OverlayHole, the pure helper, and the wrapper**

Add to `src-tauri/src/state.rs` (helper + enum at module scope; `overlay_hole` inside `impl DaemonState`):

```rust
/// Desired window region for the overlay this instant.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlayHole {
    /// Window hidden (monitoring) — region irrelevant.
    Hidden,
    /// Rounded-rect hole sized as a fraction (0.0..=0.92) of each window axis,
    /// centered. The hole is the clear, operable area.
    Frame(f64),
    /// No hole — the whole window is solid (full-screen blur, holding).
    Full,
}

/// Largest hole: leaves a ~4% blurred frame on each edge during warning.
const HOLE_MAX: f64 = 0.92;

/// Pure geometry: hole for a phase at `elapsed` seconds into it.
pub fn hole_for(phase: &Phase, elapsed: f64, cfg: &DaemonConfig) -> OverlayHole {
    // cubic ease-in-out, identical to the frontend animation
    let ease = |t: f64| -> f64 {
        let t = t.clamp(0.0, 1.0);
        if t < 0.5 {
            4.0 * t * t * t
        } else {
            1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
        }
    };
    match phase {
        Phase::Monitoring => OverlayHole::Hidden,
        Phase::Warning => OverlayHole::Frame(HOLE_MAX),
        Phase::Spreading => {
            let t = elapsed / cfg.spreading_secs.max(1) as f64;
            OverlayHole::Frame(HOLE_MAX * (1.0 - ease(t)))
        }
        Phase::Holding => OverlayHole::Full,
        Phase::Dissolving => {
            let t = elapsed / cfg.dissolving_secs.max(1) as f64;
            OverlayHole::Frame(HOLE_MAX * ease(t))
        }
    }
}

impl DaemonState {
    /// Hole for the current phase and elapsed time.
    pub fn overlay_hole(&self) -> OverlayHole {
        hole_for(&self.phase, self.phase_start.elapsed().as_secs_f64(), &self.config)
    }
}
```

> Note: `overlay_hole` is a second `impl DaemonState` block, which Rust allows. Alternatively fold it into the existing block — either is fine.

- [ ] **Step 5: Run tests to verify they pass**

Run: `cd src-tauri && cargo test hole 2>&1 | tail -20`
Expected: PASS — all 5 `hole_*` tests pass.

- [ ] **Step 6: Commit**

```bash
git add src-tauri/src/state.rs
git commit -m "feat: per-phase rounded-rect hole geometry for overlay"
```

---

## Task 3: Replace the cursor poller with the SetWindowRgn animator

**Files:**
- Modify: `src-tauri/src/lib.rs:13` (import), and the overlay-poller thread block added previously (replace it entirely).

- [ ] **Step 1: Swap the import**

In `src-tauri/src/lib.rs`, change:

```rust
use state::{DaemonConfig, DaemonState, OverlayInteract, Phase};
```

to:

```rust
use state::{DaemonConfig, DaemonState, OverlayHole, Phase};
```

- [ ] **Step 2: Replace the poller thread with the region animator**

In `src-tauri/src/lib.rs`, find the thread block that begins with the comment
`// Overlay cursor hit-test poller.` and ends just before
`// Hotkey listener thread`. Replace that entire `std::thread::spawn(move || { ... });`
block (and its preceding comment + the two `let app_for_poll` / `let state_for_poll` lines)
with:

```rust
            // Overlay region animator.
            // Cuts a rounded-rect hole in the overlay window via SetWindowRgn so the
            // clear center is genuinely not part of the window (clicks pass to the
            // desktop). The hole shrinks (spreading) / grows (dissolving); Holding
            // has no hole (full-screen blur blocks input).
            let app_for_region = app.handle().clone();
            let state_for_region = state_clone.clone();
            std::thread::spawn(move || {
                use windows::Win32::Foundation::BOOL;
                use windows::Win32::Graphics::Gdi::{
                    CombineRgn, CreateRectRgn, CreateRoundRectRgn, DeleteObject, HGDIOBJ, HRGN,
                    RGN_DIFF,
                };
                use windows::Win32::UI::WindowsAndMessaging::SetWindowRgn;

                // Dedup marker: 2.0 == Full, otherwise the Frame fraction.
                let mut last: Option<f64> = None;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(20));

                    let hole = {
                        let s = state_for_region.lock().unwrap();
                        s.overlay_hole()
                    };

                    let marker = match hole {
                        OverlayHole::Hidden => {
                            // Window hidden; forget last so the next show re-applies.
                            last = None;
                            continue;
                        }
                        OverlayHole::Full => 2.0,
                        OverlayHole::Frame(f) => f,
                    };
                    if last == Some(marker) {
                        continue;
                    }

                    let overlay = match app_for_region.get_webview_window("overlay") {
                        Some(w) => w,
                        None => continue,
                    };
                    let size = match overlay.inner_size() {
                        Ok(s) => s,
                        Err(_) => continue,
                    };
                    let hwnd = match overlay.hwnd() {
                        Ok(h) => h,
                        Err(_) => continue,
                    };
                    let w = size.width as i32;
                    let h = size.height as i32;
                    if w <= 0 || h <= 0 {
                        continue;
                    }

                    unsafe {
                        let region: HRGN = match hole {
                            OverlayHole::Full => CreateRectRgn(0, 0, w, h),
                            OverlayHole::Frame(f) => {
                                let outer = CreateRectRgn(0, 0, w, h);
                                let hole_w = (f * w as f64) as i32;
                                let hole_h = (f * h as f64) as i32;
                                let left = (w - hole_w) / 2;
                                let top = (h - hole_h) / 2;
                                // corner radius ~5% of the shorter side
                                let radius = (w.min(h) as f64 * 0.05) as i32;
                                let hole_rgn = CreateRoundRectRgn(
                                    left,
                                    top,
                                    left + hole_w,
                                    top + hole_h,
                                    radius,
                                    radius,
                                );
                                CombineRgn(outer, outer, hole_rgn, RGN_DIFF);
                                let _ = DeleteObject(HGDIOBJ(hole_rgn.0));
                                outer
                            }
                            OverlayHole::Hidden => unreachable!(),
                        };
                        // The system takes ownership of `region` and frees the previous one.
                        let _ = SetWindowRgn(hwnd, region, BOOL(1));
                    }

                    last = Some(marker);
                }
            });
```

> **windows 0.58 signature note:** if rustc complains that `CombineRgn`/`CreateRectRgn`
> handle args want `Option<HRGN>` or that `HGDIOBJ(hole_rgn.0)` is wrong, follow the
> compiler: wrap nullable handle args in `Some(...)`, and construct `HGDIOBJ` via
> `HGDIOBJ(hole_rgn.0)` (the tuple field is the raw `*mut c_void`). `BOOL(1)` is TRUE.

- [ ] **Step 3: Build**

Run: `cd src-tauri && cargo build --lib 2>&1 | tail -25`
Expected: compiles. If errors, they will be `windows`-crate signature mismatches — adjust per the note above and rebuild.

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "feat: animate overlay window region with SetWindowRgn"
```

---

## Task 4: Frontend — uniform blur + feather ring + holding-only rest text

**Files:**
- Modify: `src/windows/overlay/BlurMask.vue` (full rewrite)

- [ ] **Step 1: Rewrite BlurMask.vue**

Replace the entire contents of `src/windows/overlay/BlurMask.vue` with:

```vue
<template>
  <!-- Uniform blur layer; the OS window hole (SetWindowRgn) clips the center.
       Opacity ramps so warning shows no blur, holding is full blur. -->
  <div class="blur-layer" :style="{ opacity: blurOpacity }" />

  <!-- Feather ring sits on the hole boundary so the hard region edge reads soft.
       Sized to the same phase-driven fraction as the Rust hole. -->
  <div
    v-if="phase === 'warning' || phase === 'spreading' || phase === 'dissolving'"
    class="feather"
    :style="featherStyle"
  />

  <!-- Center rest text only during holding (no hole, so never clipped). -->
  <div v-if="phase === 'holding'" class="center-hint">
    <div class="eye-ring">👁</div>
    <span class="zh">望向 6 米外</span>
    <span class="en">LOOK 20 FT AWAY</span>
  </div>

  <!-- Bottom hints (sit at the bottom edge = inside the frame). -->
  <template v-if="phase === 'spreading' || phase === 'holding' || phase === 'dissolving'">
    <div v-if="phase === 'spreading'" class="bottom-hint bright">
      <kbd>Ctrl+Shift+Space</kbd> 跳过
    </div>
    <div v-if="phase === 'holding'" class="bottom-hint dim">
      <kbd>Ctrl+Shift+Space</kbd> 提前退出
    </div>
    <div v-if="phase === 'dissolving'" class="bottom-hint green">
      ✓ 护眼完成，恢复中…
    </div>
  </template>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue';
import type { Phase } from '../../types/blink';

const props = defineProps<{ phase: Phase }>();

// Must match Rust HOLE_MAX in state.rs.
const HOLE_MAX = 0.92;
const SPREADING_MS = 20000;
const DISSOLVING_MS = 10000;

// holeFrac: clear-center fraction of viewport (per axis). blurOpacity: 0..1.
const holeFrac = ref(HOLE_MAX);
const blurOpacity = ref(0);
let raf = 0;

function ease(t: number): number {
  t = Math.min(Math.max(t, 0), 1);
  return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

function animate(durationMs: number, apply: (e: number) => void) {
  cancelAnimationFrame(raf);
  const start = performance.now();
  function step(now: number) {
    const t = Math.min((now - start) / durationMs, 1);
    apply(ease(t));
    if (t < 1) raf = requestAnimationFrame(step);
  }
  raf = requestAnimationFrame(step);
}

watch(
  () => props.phase,
  (p) => {
    cancelAnimationFrame(raf);
    if (p === 'warning') {
      holeFrac.value = HOLE_MAX;
      blurOpacity.value = 0;
    } else if (p === 'spreading') {
      animate(SPREADING_MS, (e) => {
        holeFrac.value = HOLE_MAX * (1 - e);
        blurOpacity.value = e;
      });
    } else if (p === 'holding') {
      holeFrac.value = 0;
      blurOpacity.value = 1;
    } else if (p === 'dissolving') {
      animate(DISSOLVING_MS, (e) => {
        holeFrac.value = HOLE_MAX * e;
        blurOpacity.value = 1 - e;
      });
    } else {
      holeFrac.value = HOLE_MAX;
      blurOpacity.value = 0;
    }
  },
  { immediate: true }
);

onUnmounted(() => cancelAnimationFrame(raf));

const featherStyle = computed(() => {
  const pct = (holeFrac.value * 100).toFixed(2) + '%';
  return {
    width: pct,
    height: pct,
  };
});
</script>

<style scoped>
.blur-layer {
  position: fixed;
  inset: 0;
  backdrop-filter: blur(22px) saturate(0.6) brightness(0.85);
  -webkit-backdrop-filter: blur(22px) saturate(0.6) brightness(0.85);
  background: rgba(10, 8, 20, 0.25);
  pointer-events: none;
  transition: opacity 0.3s ease;
}

/* Soft inset ring on the hole boundary to hide the hard OS region edge. */
.feather {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  border-radius: 18px;
  pointer-events: none;
  box-shadow: inset 0 0 60px 24px rgba(10, 8, 20, 0.55);
}

.center-hint {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  text-align: center;
  pointer-events: none;
}
.eye-ring {
  font-size: 28px;
  width: 58px;
  height: 58px;
  border-radius: 50%;
  background: rgba(10, 8, 20, 0.5);
  border: 1px solid rgba(205, 214, 244, 0.25);
  box-shadow: 0 0 20px rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 14px;
}
.zh {
  display: block;
  font-size: 14px;
  font-weight: 500;
  color: rgba(205, 214, 244, 0.95);
  letter-spacing: 3px;
  margin-bottom: 5px;
  text-shadow: 0 2px 16px rgba(0, 0, 0, 0.9), 0 0 6px rgba(0, 0, 0, 0.7);
}
.en {
  display: block;
  font-size: 10px;
  color: rgba(205, 214, 244, 0.6);
  letter-spacing: 2px;
  text-shadow: 0 1px 10px rgba(0, 0, 0, 0.8);
}

.bottom-hint {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 11px;
  font-family: Inter, 'Segoe UI', sans-serif;
  background: rgba(0, 0, 0, 0.4);
  padding: 5px 16px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  white-space: nowrap;
  pointer-events: none;
  text-shadow: 0 1px 6px rgba(0, 0, 0, 0.8);
}
.bottom-hint.bright { color: rgba(205, 214, 244, 0.75); }
.bottom-hint.dim    { color: rgba(205, 214, 244, 0.3); border-color: rgba(255,255,255,0.04); }
.bottom-hint.green  { color: rgba(166, 227, 161, 0.8); }
kbd {
  background: rgba(255, 255, 255, 0.12);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 3px;
  padding: 1px 6px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: rgba(205, 214, 244, 0.75);
}
</style>
```

- [ ] **Step 2: Type-check the frontend**

Run: `npm run build 2>&1 | tail -20`
Expected: Vue/TS build succeeds (no type errors in BlurMask.vue).

- [ ] **Step 3: Commit**

```bash
git add src/windows/overlay/BlurMask.vue
git commit -m "feat: uniform blur + feather ring synced to overlay hole"
```

---

## Task 5: Full build and runtime verification

**Files:** none (verification only)

- [ ] **Step 1: Kill any running instance and build the bundle**

Run (PowerShell): `Stop-Process -Name "blink-rs" -Force -ErrorAction SilentlyContinue; npm run tauri build`
Expected: `Finished` + `bundle at: ...Blink_0.1.0_x64_en-US.msi`. Two dead-code warnings (`is_auto_start_enabled*`) are pre-existing and fine.

- [ ] **Step 2: Launch the exe**

Run (PowerShell): `Start-Process "src-tauri\target\release\blink-rs.exe"`
Expected: process starts, tray icon present.

- [ ] **Step 3: Drive the cycle and observe (manual, requires a human at the screen)**

1. Tray → Settings → set interval to **1 min** → Save → wait ~1 min.
2. **Warning (5s):** circuit traces glow at the screen edges; click/type in the center → it reaches the desktop normally; no blur yet.
3. **Spreading (20s):** the blurred frame closes inward; the still-clear center stays clickable; clicking into the blurred frame does **not** reach the desktop.
4. **Holding (20s):** whole screen blurred, input blocked, 👁 望向 6 米外 centered and readable.
5. **Dissolving (10s):** blur recedes; center reopens and becomes clickable again.
6. Press **Ctrl+Shift+Space** during any active phase → overlay dismisses immediately.

- [ ] **Step 4: Commit (if any verification fixes were needed)**

```bash
git add -A
git commit -m "chore: overlay hole-punch verified"
```

---

## Notes for the implementer

- The Rust `HOLE_MAX` (state.rs) and the JS `HOLE_MAX` (BlurMask.vue) must stay equal (0.92). If you tune one, tune both.
- The region animator runs every 20 ms but dedups on the hole fraction, so it only rebuilds the region while the fraction is actually changing (spreading/dissolving). Warning and holding apply once.
- If `SetWindowRgn` visibly clips but the WebView2 content ignores the region (raw desktop not showing in the hole), that is the documented fallback risk — escalate before switching approaches; do not silently revert to the cursor poller.
- `CircuitTraces.vue` already uses `window.innerWidth/Height` for its coordinates and renders for `warning` + `spreading`; confirm at runtime, no code change expected.
