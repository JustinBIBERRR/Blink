# Blink Overlay — Implemented Architecture

The eye-rest intervention overlay: a global gaussian-blur mask that spreads from
the screen edges to the center, over a **live, interactive** desktop center, with a
soft borderless edge. This document records the architecture that works.

## Window

- A single transparent, always-on-top, borderless, `skipTaskbar`, `focus: false`
  WebView2 window (`overlay` in `tauri.conf.json`), sized to the primary monitor.
- Shown on `Warning`, hidden on `Monitoring`.

## The four hard-won decisions

1. **Transparent body is mandatory.** `tokens.css` sets an opaque
   `body { background: #1e1e2e }` for the app windows. The overlay route overrides
   it: `html, body, #app { background: transparent !important }` (non-scoped style
   in `OverlayApp.vue`). Without this, every "transparent" CSS pixel is painted
   `#1e1e2e` and the clear center shows dark instead of the desktop. (Spike 1.)

2. **No `SetWindowRgn` hole.** A GDI region cut has a hard, non-anti-aliased edge.
   Instead, the clear center is produced purely by a CSS radial **opacity gradient**
   over the transparent window — the live desktop shows through the transparent
   pixels, and the clear→blur transition is a soft anti-aliased gradient (no line).

3. **Whole-window click-through.** `overlay.set_ignore_cursor_events(true)` on show.
   Once the body is transparent (decision 1), this keeps transparency intact AND
   lets clicks fall through to the live desktop (Spike 2). No `WM_NCHITTEST`/region
   needed.

4. **The spread is animated in pure CSS via `@property`.** A JS `requestAnimationFrame`
   loop that regenerated the mask each frame caused visible stepping (the unfocused,
   always-on-top overlay throttles JS timers, and re-rasterizing a blurred
   full-screen mask per frame is heavy). Driving the animation with an animatable
   custom property lets the browser interpolate it continuously on its compositor
   timeline — smooth, no JS, no per-frame Vue re-render.

## Visual layers (`OverlayApp.vue` composes them)

Behind → front:
1. **Live desktop** — through the transparent window.
2. **`CaptureLayer.vue`** — the frozen desktop snapshot (captured at break start by
   `capture.rs`), `filter: blur(...)`, revealed by a 2-stop radial mask
   `radial-gradient(ellipse farthest-side, transparent calc(var(--clear)*100%), #000 100%)`:
   transparent (clear, live desktop) inside the `--clear` radius, ramping to opaque
   blur at the screen edge. `opacity: var(--frost)`.
3. **`OverlayChrome.vue`** — deep vignette (Stitch stops) using the same `--clear`/
   `--frost`, plus bottom hints.
4. **`CircuitTraces.vue`** — warning-phase edge traces (semi-transparent).
5. **`EtherealGlow.vue`** — holding-phase glow ring (exact Stitch `box-shadow`) +
   "Look 20 feet away · Ns" rest text.

## Animation variables & phases

Two animatable custom properties (registered with `@property`, `<number>`,
`inherits: true`) set on `.overlay-root`, varied by the `phase-<phase>` class:

- `--clear` — clear-center radius fraction (per axis). `0.92` = almost full screen
  clear; `0` = closed.
- `--frost` — blur-layer / vignette opacity.

| Phase | `--clear` | `--frost` | Notes |
|-------|-----------|-----------|-------|
| Warning | 0.92 | 0 | circuit traces; no blur |
| Spreading | `0.92 → 0` (20s `@keyframes blink-spread`, linear) | `0 → 0.9` | iris closes edges→center |
| Holding | 0 | 0.8 | **mask dropped → uniform** stronger blur (`blur(24px)`); glow ring is the focus; ~20% of desktop shows through |
| Dissolving | `0 → 0.92` (10s `@keyframes blink-dissolve`) | `0.9 → 0` | iris reopens |

- Holding overrides (`OverlayApp.vue` global): `.phase-holding .capture { filter:
  blur(24px); mask-image: none }` and a flat vignette, so the center is covered like
  the edges. `CaptureLayer`/`vignette` have `transition: filter/opacity` so the
  spreading→holding change eases instead of popping.
- Animation durations (20s / 10s) are hardcoded to the daemon defaults
  (`state.rs` `spreading_secs` / `dissolving_secs`). If those configs change, update
  the keyframe durations to match. (v1 limitation.)

## Interaction & control

- Phase machine: `daemon.rs` / `state.rs` (Monitoring → Warning → Spreading →
  Holding → Dissolving), emitted as `blink://phase`; the front-end sets the
  `phase-<phase>` class from it.
- Capture: emitted as `blink://capture` (base64 BMP data URL) at the
  Monitoring→Warning transition, before `overlay.show()`.
- Exit: the visible × or a docked-eraser double click ends the shared reminder. No close keyboard shortcut is registered.
- `BLINK_DEV_INTERVAL=<secs>` env var shortens the interval for testing.

## Verification

GDI screenshots (`CopyFromScreen`/BitBlt) **cannot** capture WebView2's
hardware-composited output, so the overlay never appears in such screenshots —
visual sign-off is by eye on the running app.

## Designs

Stitch project `1661188105909361640` ("Blink Eye Health Dashboard"); local
references in `.designref/` (`gaussian.png`, `ethereal.png`); prompts in
`stitch-prompts.md`. Spec: `docs/superpowers/specs/2026-06-18-overlay-gaussian-borderless-design.md`.
