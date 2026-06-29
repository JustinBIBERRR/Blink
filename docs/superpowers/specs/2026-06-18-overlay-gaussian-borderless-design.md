# Blink Overlay — Borderless Gaussian Intervention (Design Spec)

Date: 2026-06-18
Status: Approved design, pending spec review

## 1. Problem

The eye-rest intervention overlay must render, over the user's real screen:

- a **global gaussian-blur mask** that **spreads from the four edges toward the center**,
- a **clear center** that shows the **live, interactive** desktop,
- a **soft, borderless edge** between clear center and blurred surround — matching the
  Stitch designs (no hard line).

The previous implementation used `SetWindowRgn` to cut an elliptical hole for the clear
center. GDI regions have **hard, non-anti-aliased edges**, so the clear→blur boundary
always showed an obvious line. Attempts to remove the hole made the whole screen dark.

## 2. Root cause (validated by Spike 1)

`src/assets/styles/tokens.css` sets an **opaque** `body { background: #1e1e2e }`, applied
to every window including the transparent overlay. So every "transparent" CSS pixel was
painted `#1e1e2e` — the live desktop only ever showed through an actual `SetWindowRgn`
hole. **Spike 1 confirmed:** overriding `body` to `transparent` for the overlay makes the
transparent center show the live desktop. This unlocks a region-free design.

## 3. Approach (A1): webview-only visuals + native hit-test interaction

Separate the **visual effect** (100% in the webview) from the **interaction** (a native
hit-test). No window region cut, so the edge is a pure anti-aliased CSS gradient.

### 3.1 Transparency
- The overlay route forces `html, body, #app { background: transparent }`.
- Window stays `transparent: true`, `alwaysOnTop`, `skipTaskbar`, `decorations: false`.

### 3.2 Visual layers (z-order, behind → front)
1. **Live desktop** — shows through the transparent window where nothing is painted.
2. **Blur layer** (`CaptureLayer`): the frozen desktop snapshot (captured at break start,
   see capture.rs), Gaussian-blurred, revealed by a **radial opacity gradient** that is
   transparent at the clear radius and opaque at the edges. The clear radius (`holeFrac`)
   animates large→0 (spreading) and 0→large (dissolving).
3. **Vignette / glow** (`OverlayChrome`): deep-vignette darkening + (holding) the ethereal
   glow ring.
4. **Circuit traces** (`CircuitTraces`): warning-phase edge animation.
5. **HUD text**: rest text + countdown + skip hint.

### 3.3 Interaction — native `WM_NCHITTEST` (replaces `SetWindowRgn`)
- Subclass the overlay's top-level HWND (`SetWindowSubclass` via the `windows` crate).
- On `WM_NCHITTEST`: convert the cursor to client coords; if inside the current clear
  ellipse (radius from the shared `holeFrac` daemon state) return `HTTRANSPARENT` (the
  click falls through to the live desktop); otherwise `HTCLIENT` (blurred ring blocks).
- Uses **no** `WS_EX_LAYERED`/`WS_EX_TRANSPARENT`, so transparency and the soft edge are
  preserved.
- **Open risk → Spike 2:** verify the hit-test reaches the top-level window and isn't
  swallowed by the WebView2 child. **Fallback if it fails:** center is visible but not
  click-through (acceptable for a forced rest); revisit with a child-window hit-test hook.

## 4. Strict Stitch styling (authoritative values)

Styling must match the Stitch mockups. Exact values from the design HTML:

### 4.1 Spreading — "Full Screen Gaussian Blur Overlay" (`gaussian.html`)
- Blur: `blur(12px)` at mid-convergence (heavier toward holding); our blur applies to the
  captured snapshot via CSS `filter` (or WebGL for true variable radius).
- Iris mask shape: `radial-gradient(ellipse 45% 45% at 50% 50%, transparent 10%, black 70%)`
  — i.e. a **small clear core, wide soft ramp**. Our animated mask uses this curve, with
  the transparent radius driven by `holeFrac`.
- Deep vignette: `radial-gradient(circle at center, transparent 20%, rgba(24,24,37,0.4) 40%,
  rgba(24,24,37,0.95) 100%)`. DECISION: keep the **stops/shape** but **start at a low edge
  alpha** (~0.5) and the overall blur opacity low (`MAX_BLUR` ~0.5), then tune up by eye —
  the user wants to begin translucent. See §7.
- Palette: surface `#1e1e2e` / `#181825`, blue `#89b4fa`, green `#a6e3a1`, text `#cdd6f4`,
  dim `#45475a`.

### 4.2 Holding — "Full Screen Ethereal Glow Overlay" (`ethereal.html`)
- Blur layer: `blur(12px)`, full-screen.
- Glow ring (`.glow-seam`), centered, 500px circle:
  `box-shadow: inset 0 0 40px 10px rgba(137,180,250,0.15), 0 0 60px 5px rgba(137,180,250,0.1)`,
  `border: 1px solid rgba(137,180,250,0.05)`.
- HUD text: "Look 20 feet away · {N}s", with `subtle-pulse` 4s ease-in-out
  (opacity 0.9↔1, scale 1↔1.02).
- Bottom hint pill: "Ctrl + Shift + Space to skip", `#45475a` 10px monospace.

### 4.3 Warning — circuit traces on the edges
- Edge-hugging glowing traces + nodes in `#89b4fa` / `#cba6f7`, semi-transparent
  (~0.6 opacity) so the live screen shows through. Big clear center, no blur yet.

### 4.4 Card / rest-text placement (decided)
- **Spreading: NO card.** Only the gaussian blur spreading from the edges + the live
  interactive center. Nothing in the center.
- **Holding: the "Look 20 feet away" rest text** sits in the center, with the ethereal
  glow ring (§4.2). This is the only phase with center text.

## 5. Phase mapping & animation

| Phase | Duration | Clear radius (`holeFrac`) | Visual |
|------|----------|---------------------------|--------|
| Warning | 5s | `HOLE_MAX` (0.92) | circuit edges, no blur |
| Spreading | 20s | `HOLE_MAX → 0` (eased) | gradient blur converges; opacity 0→`MAX_BLUR` |
| Holding | 20s | 0 | full blur + glow ring + rest text |
| Dissolving | 10s | `0 → HOLE_MAX` | blur recedes, reverse |

- `MAX_BLUR` ≈ 0.6 (blur layer opacity cap so the desktop stays visible — tunable).
- Easing: existing cubic ease in `useOverlayAnim`/`state.rs` (kept in sync front/back).
- `holeFrac` is the single shared parameter: drives the CSS mask radius **and** the native
  hit-test ellipse.

## 6. Components / files

**Remove**
- `lib.rs` `SetWindowRgn` region-animator thread (and unused `OverlayHole` plumbing if no
  longer referenced; keep `state.rs::hole_for` math — reused for the hit-test radius).

**Add**
- Overlay transparent-body style (scoped to the overlay route).
- Native `WM_NCHITTEST` subclass module (Rust, `windows` crate) reading shared `holeFrac`.

**Keep / adapt**
- `capture.rs` (frozen snapshot — validated).
- `CaptureLayer.vue` (single blurred layer + animated radial gradient mask, Stitch values).
- `OverlayChrome.vue` (vignette + glow ring).
- `EtherealGlow.vue` (holding ring + text).
- `CircuitTraces.vue` (warning edges, low opacity).
- `useOverlayAnim.ts` (phase → holeFrac/blurOpacity/progress).
- Daemon phase machine (`daemon.rs`, `state.rs`) and capture-on-warning wiring.

## 7. Tunables (exposed for visual matching)
- Blur radius per phase (spreading vs holding).
- `MAX_BLUR` opacity cap.
- Vignette edge alpha.
- Mask ramp curve (match `transparent 10% → black 70%`).
- Glow-ring size/intensity.

## 8. Verification
1. **Spike 2** (first impl step): native `WM_NCHITTEST` click-through with WebView2 child.
2. Visual: trigger a cycle (`BLINK_DEV_INTERVAL`), compare each phase against
   `gaussian.png` / `ethereal.png`. (Note: GDI screenshots can't capture WebView2 output —
   visual sign-off is by eye.)
3. Center shows live desktop; edge has no hard line; clicks pass through the center
   (if Spike 2 passes).

## 9. Non-goals
- Live (per-frame) blur of the surround — frozen snapshot is intentional.
- DWM acrylic / DirectComposition native blur (rejected: uniform, can't gradient).
- Multi-monitor (v1 = primary monitor only).
