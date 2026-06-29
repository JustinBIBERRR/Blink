# Overlay Hole-Punch Design

**Date:** 2026-06-17
**Status:** Approved (pending spec review)
**Component:** blink-rs eye-protection overlay

## Problem

During the eye-protection cycle the fullscreen transparent overlay must let
the user keep working in the screen center while edge/blur effects play, then
fully blur the screen at the rest moment. Prior attempts toggled
`set_ignore_cursor_events` per-region via a cursor poller. This is unreliable:
the window pixel still exists at the OS level, so a click in the "clear" center
is not guaranteed to reach the desktop. Two builds failed to deliver an
operable center.

## Decision

Make the center pixels **genuinely not part of the window** using the Win32
region API (`SetWindowRgn`). Where the region has a hole, the window does not
exist — clicks reach the desktop automatically and nothing renders there. This
is OS-guaranteed, with no polling ambiguity and no flicker.

Hole shape: **rounded rectangle** (keeps screen corners — taskbar, etc. —
operable; reads as a blurred picture frame).

## Phase behavior

| Phase            | Hole (fraction of window)     | Visual / interaction                                   |
|------------------|-------------------------------|--------------------------------------------------------|
| Monitoring       | n/a (window hidden)           | nothing                                                |
| Warning (5s)     | static large (~0.92 each axis)| thin edge frame; circuit traces only, blur opacity 0; center fully operable |
| Spreading (20s)  | shrinks 0.92 → 0              | blur frame grows inward; clear center shrinks, stays operable |
| Holding (20s)    | none (full window)            | full-screen blur, blocks all input, 👁 rest text       |
| Dissolving (10s) | grows 0 → 0.92               | blur recedes; center reopens and becomes operable      |

"Fraction" = the rounded-rect hole's width/height as a fraction of the window's
width/height, centered. 0.92 leaves a ~4% frame on each edge. Easing matches
the existing cubic ease-in-out used by the visual animation.

## Components

### 1. Geometry source — `src-tauri/src/state.rs`

Replace `OverlayInteract` / `overlay_interact()` (the Passthrough/Ring cursor
model) with a hole-geometry method:

```
pub enum OverlayHole {
    Hidden,           // window hidden (monitoring)
    Frame(f64),       // rounded-rect hole, fraction in 0.0..=0.92
    Full,             // no hole, whole window is solid (holding)
}

impl DaemonState {
    pub fn overlay_hole(&self) -> OverlayHole { ... }
}
```

- Monitoring → `Hidden`
- Warning → `Frame(0.92)` (static)
- Spreading → `Frame(lerp 0.92 → 0.0 over spreading_secs, eased)`
- Holding → `Full`
- Dissolving → `Frame(lerp 0.0 → 0.92 over dissolving_secs, eased)`

Corner radius: a fixed fraction of the smaller window dimension (e.g. ~2.5%),
computed in the animator from physical pixels.

### 2. Region animator — `src-tauri/src/lib.rs`

Replace the `set_ignore_cursor_events` poller thread with a region thread.
Every ~20 ms:

1. Read `state.overlay_hole()`.
2. `Hidden` → skip (window hidden).
3. Get `overlay.hwnd()`.
4. `Full` → `SetWindowRgn(hwnd, full_rect_rgn, TRUE)` (or NULL region).
5. `Frame(f)` → compute hole rect in physical px from `overlay.inner_size()`,
   centered: `hole_w = f * w`, `hole_h = f * h`. Build:
   - `outer = CreateRectRgn(0, 0, w, h)`
   - `hole  = CreateRoundRectRgn(holeLeft, holeTop, holeRight, holeBottom, radius, radius)`
   - `CombineRgn(outer, outer, hole, RGN_DIFF)`
   - `DeleteObject(hole)`
   - `SetWindowRgn(hwnd, outer, TRUE)` — Windows takes ownership of `outer`;
     do not delete it.

Track the last applied fraction to skip redundant `SetWindowRgn` calls when
geometry is unchanged (e.g. during Holding).

Dependency: add the `windows` crate (matching Tauri's version) with features
`Win32_Foundation`, `Win32_Graphics_Gdi`, `Win32_UI_WindowsAndMessaging`.

The per-phase `show`/`hide`/resize logic in the daemon callback stays. On
`Monitoring`, reset the window region to full (NULL) before/after hide so the
next show is clean.

### 3. Frontend

`src/windows/overlay/BlurMask.vue`:
- Blur layer: full-window `backdrop-filter: blur(...)`, uniform. The OS hole
  does the clipping — remove the JS `mask-image` radial-gradient used for
  operability. Keep a soft visual only via the feather ring below.
- Feather ring: a div positioned on the hole boundary (same phase-driven
  fraction as Rust) with an inset blurred shadow / soft border so the hard
  region edge reads as soft.
- Blur opacity: 0 during Warning, fades in during Spreading, full during
  Holding, fades out during Dissolving.
- 👁 center text: render only during Holding (no hole) so it is never clipped.

`src/windows/overlay/CircuitTraces.vue`:
- Already renders traces hugging the screen edges using `window.innerWidth/Height`.
  These sit inside the frame (outside the hole) during Warning + Spreading.
  No change needed beyond confirming it shows for both phases.

The CSS feather fraction and the Rust hole fraction are both derived from the
same phase + elapsed time, so they align at any device pixel ratio (both are
fraction-based, DPI-independent).

## Data flow

```
daemon phase loop (Rust)
   → DaemonState.phase / phase_start   (shared, Arc<Mutex>)
        ├─ region animator thread (Rust): reads overlay_hole() → SetWindowRgn(hwnd)
        └─ emit "blink://phase" → overlay WebView (Vue): blur opacity, circuit, feather, 👁 text
```

Geometry is computed independently on both sides from the same phase + elapsed
time using the same easing — no IPC of geometry needed.

## Testing / verification

Runtime, on the running exe:
1. Set interval to 1 min, save, wait for the cycle.
2. Warning: click/type in the center → works; edges glow with circuit.
3. Spreading: center still clickable while the blur frame closes in; clicking
   into the blurred frame does not reach the desktop.
4. Holding: whole screen blurred, input blocked, 👁 text centered and readable.
5. Dissolving: center reopens and becomes clickable as blur recedes.
6. Hotkey Ctrl+Shift+Space still exits early from any active phase.

## Risks / notes

- Hard hole edge: mitigated by the feather ring + near-zero blur at the
  boundary during the thin-frame warning state.
- `SetWindowRgn` clips the host HWND, which clips the WebView2 child — verified
  behavior on Windows. If WebView2 hardware compositing ignores the region,
  fallback is four edge windows (rejected approach ②).
- `windows` crate version must match the one Tauri pulls in to avoid a
  duplicate-HWND-type mismatch with `overlay.hwnd()`.
- This is Windows-only code; the project targets Windows, so no cross-platform
  guard is required beyond what already exists.
