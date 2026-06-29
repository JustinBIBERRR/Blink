# Borderless Gaussian Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the hard-edged `SetWindowRgn` overlay hole with a region-free, soft gaussian-gradient blur that spreads edges→center, over a live interactive desktop center, styled strictly to the Stitch mockups.

**Architecture:** All visuals live in the transparent WebView2 overlay (CSS gradient mask over a frozen desktop snapshot); the clear center shows the live desktop because the overlay route forces `body` transparent. Interaction is handled separately by making the window click-through (verified in Spike 2) rather than by cutting a window region.

**Tech Stack:** Tauri 2 (Rust), Vue 3 + TypeScript, Windows `windows` crate 0.58, WebView2.

**Context for the engineer:** Much of the front-end already exists from prior iterations (`CaptureLayer.vue`, `OverlayChrome.vue`, `EtherealGlow.vue`, `CircuitTraces.vue`, `useOverlayAnim.ts`, `capture.rs`). This plan adapts them. The daemon phase machine (`daemon.rs`, `state.rs`) and capture-on-warning wiring already work. Spec: `docs/superpowers/specs/2026-06-18-overlay-gaussian-borderless-design.md`.

**Build/run commands:**
- Front-end typecheck+build: `npm run build` (from `blink-rs/`)
- Rust check: `cd src-tauri && cargo check`
- Rust tests: `cd src-tauri && cargo test`
- Run app with a fast break for visual checks: `$env:BLINK_DEV_INTERVAL='6'; $env:RUST_LOG='info'; npm run tauri dev` (PowerShell)

---

## File Structure

- `src/windows/overlay/OverlayApp.vue` — composes layers; **adds the transparent-body style** for the overlay route.
- `src/windows/overlay/CaptureLayer.vue` — frozen snapshot + animated radial gradient mask (region-free; transparent center shows live desktop).
- `src/windows/overlay/OverlayChrome.vue` — deep vignette (Stitch stops, low alpha).
- `src/windows/overlay/EtherealGlow.vue` — holding glow ring + rest text (Stitch values).
- `src/windows/overlay/CircuitTraces.vue` — warning edges (low opacity).
- `src/composables/useOverlayAnim.ts` — phase → `holeFrac`/`blurOpacity`; `MAX_BLUR` low.
- `src-tauri/src/state.rs` — add `point_in_clear_ellipse` pure geometry (+ tests); keep `hole_for`.
- `src-tauri/src/interact.rs` — **new**: cursor-position → window click-through toggle (or native hit-test fallback).
- `src-tauri/src/lib.rs` — remove `SetWindowRgn` region thread; spawn the interaction thread.

---

## Task 1: Transparent body for the overlay route

**Why first:** This is the validated unlock (Spike 1). Without it every transparent pixel is painted `#1e1e2e`.

**Files:**
- Modify: `src/windows/overlay/OverlayApp.vue`

- [ ] **Step 1: Add a non-scoped global style to OverlayApp.vue**

Add this second `<style>` block (non-scoped) to `OverlayApp.vue`, after the existing scoped one:

```vue
<style>
/* The overlay window must be truly transparent so the clear center shows the
   live desktop. tokens.css sets an opaque body background for the app windows;
   override it for the overlay route only (this component renders only there). */
html,
body,
#app {
  background: transparent !important;
}
</style>
```

- [ ] **Step 2: Build to verify it compiles**

Run: `npm run build`
Expected: `✓ built in …` with no errors.

- [ ] **Step 3: Commit**

```bash
git add src/windows/overlay/OverlayApp.vue
git commit -m "fix(overlay): force transparent body so clear center shows live desktop"
```

---

## Task 2: Spike 2 — verify click-through keeps transparency

**Why:** The interactive center needs clicks to pass through. The simplest mechanism is Tauri's `set_ignore_cursor_events(true)` (whole window). Earlier it appeared to darken the overlay — but that was before the Task 1 body fix. Verify whether, with a transparent body, it keeps the desktop visible AND passes clicks.

**Files:**
- Modify (temporarily): `src-tauri/src/lib.rs`

- [ ] **Step 1: Temporarily enable whole-window click-through on overlay show**

In `lib.rs`, in the daemon `on_phase_change` closure, in the `Phase::Warning` arm, immediately after `let _ = overlay.show();` add:

```rust
// SPIKE 2: whole-window click-through. Verify transparency stays intact now
// that the body is transparent (Task 1).
let _ = overlay.set_ignore_cursor_events(true);
```

- [ ] **Step 2: Run the app and observe**

Run (PowerShell): `$env:BLINK_DEV_INTERVAL='6'; npm run tauri dev`
During a break, manually check two things:
1. Does the overlay still show the blurred desktop (NOT all-black)?
2. Can you click desktop icons/windows through the overlay center?

Record the result. **Decision point:**
- **PASS** (transparent + click-through both work) → keep this line; the interaction model is whole-window click-through. In Task 9 take the "PASS" branch ("click-through = whole-window").
- **FAIL** (overlay goes dark) → remove the spike line; in Task 9 take the "FAIL" branch (region-aware cursor-toggle) or, if that also darkens, the documented fallback (non-interactive center).

- [ ] **Step 3: Stop the app; leave the code in the state dictated by the decision**

If PASS: keep the line. If FAIL: remove the spike line so `Warning` just calls `overlay.show()`.

- [ ] **Step 4: Commit the decision**

```bash
git add src-tauri/src/lib.rs
git commit -m "spike: determine overlay click-through mechanism (transparency-safe)"
```

---

## Task 3: Point-in-ellipse geometry (pure, tested)

**Why:** Both the CSS mask and any region-aware interaction need the same "is this point inside the clear ellipse?" math. Implement once, tested.

**Files:**
- Modify: `src-tauri/src/state.rs`
- Test: `src-tauri/src/state.rs` (`#[cfg(test)]`)

- [ ] **Step 1: Write the failing test**

Add to the `tests` module in `state.rs`:

```rust
#[test]
fn point_in_clear_ellipse_center_is_inside() {
    // 1000x800 window, hole_frac 0.5 => half-axes 250 x 200, centered at (500,400)
    assert!(point_in_clear_ellipse(1000, 800, 0.5, 500, 400));
}

#[test]
fn point_in_clear_ellipse_corner_is_outside() {
    assert!(!point_in_clear_ellipse(1000, 800, 0.5, 10, 10));
}

#[test]
fn point_in_clear_ellipse_zero_frac_is_outside() {
    assert!(!point_in_clear_ellipse(1000, 800, 0.0, 500, 400));
}

#[test]
fn point_in_clear_ellipse_on_axis_edge() {
    // hole_frac 0.5 => x half-axis = 250; point at x=500+249 just inside
    assert!(point_in_clear_ellipse(1000, 800, 0.5, 749, 400));
    assert!(!point_in_clear_ellipse(1000, 800, 0.5, 751, 400));
}
```

- [ ] **Step 2: Run to verify it fails**

Run: `cd src-tauri && cargo test point_in_clear_ellipse`
Expected: FAIL — `cannot find function point_in_clear_ellipse`.

- [ ] **Step 3: Implement the function**

Add to `state.rs` (module level, near `hole_for`):

```rust
/// True if window pixel (px, py) lies inside the centered clear ellipse whose
/// per-axis half-extent is `hole_frac` of the window half-size. Matches the CSS
/// `radial-gradient(ellipse farthest-side …)` transparent radius used by the
/// overlay mask, so interaction and visuals agree.
pub fn point_in_clear_ellipse(w: i32, h: i32, hole_frac: f64, px: i32, py: i32) -> bool {
    if hole_frac <= 0.0 || w <= 0 || h <= 0 {
        return false;
    }
    let cx = w as f64 / 2.0;
    let cy = h as f64 / 2.0;
    let ax = hole_frac * (w as f64 / 2.0); // x half-axis
    let ay = hole_frac * (h as f64 / 2.0); // y half-axis
    if ax <= 0.0 || ay <= 0.0 {
        return false;
    }
    let dx = (px as f64 - cx) / ax;
    let dy = (py as f64 - cy) / ay;
    dx * dx + dy * dy <= 1.0
}
```

- [ ] **Step 4: Run to verify it passes**

Run: `cd src-tauri && cargo test point_in_clear_ellipse`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/state.rs
git commit -m "feat(overlay): add tested point_in_clear_ellipse geometry"
```

---

## Task 4: Remove the SetWindowRgn region thread

**Why:** The hard edge comes from the GDI region cut. The clear center now comes from CSS transparency instead.

**Files:**
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: Delete the region-animator thread**

In `lib.rs`, remove the entire `std::thread::spawn(move || { … SetWindowRgn … })` block (the one introduced by the comment "Overlay region animator."). Replace it with:

```rust
            // No window region: the clear center is produced by the CSS gradient
            // mask over a transparent window (Task 1). Interaction is handled by
            // the interaction thread (Task 9), not by cutting the window.
```

- [ ] **Step 2: Remove the now-unused import**

In `lib.rs`, change:

```rust
use state::{DaemonConfig, DaemonState, OverlayHole, Phase};
```
to:
```rust
use state::{DaemonConfig, DaemonState, Phase};
```

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: `Finished`. (Note: `OverlayHole`/`hole_for`/`overlay_hole` remain in `state.rs` used by their own tests; pub items don't warn.)

- [ ] **Step 4: Commit**

```bash
git add src-tauri/src/lib.rs
git commit -m "refactor(overlay): remove SetWindowRgn hole (region-free soft edge)"
```

---

## Task 5: CaptureLayer — region-free gradient mask, Stitch values, low opacity

**Why:** The blurred ring must reveal the live desktop in the center via a soft radial *opacity* gradient (no region). Mask shape follows the Stitch iris (`transparent 10% → black 70%` feel), animated by `holeFrac`.

**Files:**
- Modify: `src/windows/overlay/CaptureLayer.vue`

- [ ] **Step 1: Replace the blurStyle computed + img filter**

Set `CaptureLayer.vue`'s `<script setup>` `blurStyle` to:

```ts
const blurStyle = computed(() => {
  const f = Math.max(props.holeFrac, 0);
  const inner = f * 100;
  let mask: string;
  if (f <= 0.02) {
    mask = 'none';
  } else {
    // Stitch iris feel: small clear core, wide soft ramp to full blur at the
    // edge. Transparent at the clear radius (live desktop shows), easing opaque.
    const span = 100 - inner;
    const s1 = (inner + span * 0.18).toFixed(1);
    const s2 = (inner + span * 0.45).toFixed(1);
    const s3 = (inner + span * 0.72).toFixed(1);
    mask =
      `radial-gradient(ellipse farthest-side at 50% 50%,` +
      ` transparent ${inner.toFixed(1)}%,` +
      ` rgba(0,0,0,0.12) ${s1}%,` +
      ` rgba(0,0,0,0.45) ${s2}%,` +
      ` rgba(0,0,0,0.82) ${s3}%,` +
      ` #000 100%)`;
  }
  return { opacity: props.blurOpacity, WebkitMaskImage: mask, maskImage: mask };
});
```

Set the `.capture` CSS `filter`/`transform` to the Stitch-matched frost (bright, translucent):

```css
.capture {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  object-fit: cover;
  filter: blur(12px) saturate(0.96) brightness(0.95);
  transform: scale(1.06);
  pointer-events: none;
  transition: opacity 0.3s ease;
  will-change: opacity, mask-image;
}
```

(Keep the existing `<template>` `<img class="capture" :src="src" :style="blurStyle" …>` and the `defineProps<{ src; holeFrac; blurOpacity }>()`.)

- [ ] **Step 2: Build**

Run: `npm run build`
Expected: `✓ built`.

- [ ] **Step 3: Commit**

```bash
git add src/windows/overlay/CaptureLayer.vue
git commit -m "feat(overlay): region-free gradient blur mask (Stitch iris, translucent)"
```

---

## Task 6: useOverlayAnim — low opacity cap

**Files:**
- Modify: `src/composables/useOverlayAnim.ts`

- [ ] **Step 1: Set MAX_BLUR low**

Ensure the constant is:

```ts
/** Cap the blur layer opacity low so the desktop stays visible through the frost. */
const MAX_BLUR = 0.5;
```

And the watch uses it for spreading/holding/dissolving (`blurOpacity = MAX_BLUR * e`, `MAX_BLUR`, `MAX_BLUR * (1 - e)`), with `warning` setting `blurOpacity = 0` and `holeFrac = HOLE_MAX`.

- [ ] **Step 2: Build**

Run: `npm run build`
Expected: `✓ built`.

- [ ] **Step 3: Commit**

```bash
git add src/composables/useOverlayAnim.ts
git commit -m "feat(overlay): start blur opacity low (translucent)"
```

---

## Task 7: OverlayChrome — Stitch vignette stops, low edge alpha

**Files:**
- Modify: `src/windows/overlay/OverlayChrome.vue`

- [ ] **Step 1: Set the vignette to the Stitch shape, low alpha**

Set `vignetteStyle` to (stops mirror the design's `transparent 20% → 0.4 @ 40% → edge`, but edge alpha starts low):

```ts
const vignetteStyle = computed(() => {
  const f = props.holeFrac;
  const inner = Math.max(f * 100, 0);
  const span = 100 - inner;
  const s1 = (inner + span * 0.30).toFixed(1);
  const s2 = (inner + span * 0.70).toFixed(1);
  const bg =
    f <= 0.02
      ? `radial-gradient(ellipse farthest-side at 50% 50%, rgba(24,24,37,0.10), rgba(24,24,37,0.50) 100%)`
      : `radial-gradient(ellipse farthest-side at 50% 50%,
           transparent ${inner.toFixed(1)}%,
           rgba(24,24,37,0.10) ${s1}%,
           rgba(24,24,37,0.30) ${s2}%,
           rgba(24,24,37,0.50) 100%)`;
  return { background: bg, opacity: props.blurOpacity };
});
```

- [ ] **Step 2: Build**

Run: `npm run build`
Expected: `✓ built`.

- [ ] **Step 3: Commit**

```bash
git add src/windows/overlay/OverlayChrome.vue
git commit -m "feat(overlay): vignette uses Stitch stops at low edge alpha"
```

---

## Task 8: EtherealGlow — exact Stitch glow ring (holding)

**Why:** Strict fidelity to `ethereal.html`'s glow seam.

**Files:**
- Modify: `src/windows/overlay/EtherealGlow.vue`

- [ ] **Step 1: Set the ring to the exact Stitch shadow**

Set the `.glow-ring` CSS to the design values (centered 500px circle):

```css
.glow-ring {
  width: 500px;
  height: 500px;
  border-radius: 50%;
  border: 1px solid rgba(137, 180, 250, 0.05);
  box-shadow:
    inset 0 0 40px 10px rgba(137, 180, 250, 0.15),
    0 0 60px 5px rgba(137, 180, 250, 0.1);
}
```

Keep the `subtle-pulse` (4s ease-in-out, opacity 0.9↔1, scale 1↔1.02) and the
"Look 20 feet away · {N}s" text with countdown.

- [ ] **Step 2: Build**

Run: `npm run build`
Expected: `✓ built`.

- [ ] **Step 3: Commit**

```bash
git add src/windows/overlay/EtherealGlow.vue
git commit -m "feat(overlay): holding glow ring matches Stitch exactly"
```

---

## Task 9: Interaction wiring (based on Spike 2 result)

**Why:** Make the clear center interactive without breaking transparency. Implement the mechanism chosen in Task 2.

**Files:**
- Create: `src-tauri/src/interact.rs`
- Modify: `src-tauri/src/lib.rs`

### If Spike 2 PASSED (whole-window click-through is transparency-safe)

- [ ] **Step 1: Keep the Task 2 line; no new module needed.** The overlay is click-through for the whole break. Skip to Step 5 (commit already done in Task 2). Mark this task complete.

### If Spike 2 FAILED (need region-aware toggle)

- [ ] **Step 1: Create `src-tauri/src/interact.rs`**

```rust
//! Region-aware click-through: toggles the overlay's ignore-cursor-events based
//! on whether the cursor is inside the clear ellipse, so the center is
//! interactive while the blurred ring blocks. Runs only while the overlay is up.

use crate::daemon::SharedState;
use crate::state::{point_in_clear_ellipse, Phase};
use tauri::{AppHandle, Manager};

pub fn spawn(app: AppHandle, state: SharedState) {
    std::thread::spawn(move || {
        use windows::Win32::Foundation::POINT;
        use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

        let mut last_ignore: Option<bool> = None;
        loop {
            std::thread::sleep(std::time::Duration::from_millis(30));

            let (hole_frac, active) = {
                let s = state.lock().unwrap();
                let active = !matches!(s.phase, Phase::Monitoring);
                let hole_frac = match s.overlay_hole() {
                    crate::state::OverlayHole::Frame(f) => f,
                    crate::state::OverlayHole::Full => 0.0,
                    crate::state::OverlayHole::Hidden => 0.0,
                };
                (hole_frac, active)
            };
            if !active {
                last_ignore = None;
                continue;
            }

            let overlay = match app.get_webview_window("overlay") {
                Some(w) => w,
                None => continue,
            };
            let size = match overlay.inner_size() {
                Ok(s) => s,
                Err(_) => continue,
            };
            let pos = match overlay.outer_position() {
                Ok(p) => p,
                Err(_) => continue,
            };

            let mut pt = POINT { x: 0, y: 0 };
            unsafe {
                if GetCursorPos(&mut pt).is_err() {
                    continue;
                }
            }
            // Cursor in window-local pixels.
            let lx = pt.x - pos.x;
            let ly = pt.y - pos.y;
            let inside = point_in_clear_ellipse(
                size.width as i32,
                size.height as i32,
                hole_frac,
                lx,
                ly,
            );
            // inside clear center => click-through (ignore = true).
            if last_ignore != Some(inside) {
                let _ = overlay.set_ignore_cursor_events(inside);
                last_ignore = Some(inside);
            }
        }
    });
}
```

- [ ] **Step 2: Register the module and spawn it**

In `lib.rs`, add `mod interact;` with the other `mod` lines, and in `setup` (after the daemon spawn, where `state_clone` and `app.handle()` are available) add:

```rust
interact::spawn(app.handle().clone(), state_clone.clone());
```

Ensure `Win32_UI_WindowsAndMessaging` (already enabled) covers `GetCursorPos`/`POINT`.

- [ ] **Step 3: Verify it compiles**

Run: `cd src-tauri && cargo check`
Expected: `Finished`.

- [ ] **Step 4: Run and verify**

Run: `$env:BLINK_DEV_INTERVAL='6'; npm run tauri dev`
During spreading/holding, move the cursor to the center and click — it should reach the desktop; move to the blurred ring and click — it should be blocked. Confirm the overlay does NOT flicker dark when toggling (if it does, the toggle approach is unviable → fall back to non-interactive center, documented in spec §3.3).

- [ ] **Step 5: Commit**

```bash
git add src-tauri/src/interact.rs src-tauri/src/lib.rs
git commit -m "feat(overlay): region-aware click-through center"
```

---

## Task 10: Full visual verification against the Stitch mockups

**Files:** none (verification only)

- [ ] **Step 1: Run a full cycle**

Run: `$env:BLINK_DEV_INTERVAL='6'; $env:RUST_LOG='info'; npm run tauri dev`

- [ ] **Step 2: Verify each phase by eye (GDI screenshots can't capture WebView2)**

Check against `.designref/gaussian.png` and `.designref/ethereal.png`:
- Warning: circuit traces on edges, large clear live center, no blur.
- Spreading: soft gaussian blur converging edges→center, **no hard edge line**, center shows live desktop, no card, translucent.
- Holding: full (translucent) blur + ethereal glow ring + "Look 20 feet away · Ns" text.
- Dissolving: blur recedes smoothly.
- Center is clickable (if Task 8 interactive), `Ctrl+Shift+Space` escapes.

- [ ] **Step 3: Tune the visual constants if needed**

Adjust only these, by eye, then rebuild: `MAX_BLUR` (useOverlayAnim), blur radius/brightness (CaptureLayer `.capture`), vignette alpha (OverlayChrome), glow-ring intensity (EtherealGlow). Commit any changes:

```bash
git add -A && git commit -m "polish(overlay): tune blur/opacity to match Stitch designs"
```

---

## Notes

- **No `set_ignore_cursor_events` before Task 1.** It must be tested only after the body is transparent, or transparency is confounded.
- **Multi-monitor:** primary only (v1).
- **Frozen snapshot:** the blurred ring is the snapshot captured at break start (`capture.rs`); the center is live. This is intentional.
