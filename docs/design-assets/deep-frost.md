# Deep frost texture

Original image generated for Blink on 2026-11-23. No third-party texture was supplied to the generation tool.

The user selected option C in `realistic-ice-options.png`. The target is dense peripheral frost with translucent central ice that makes desktop content appear below the ice surface. Erased areas must expose the live desktop.

`src/assets/ice/deep-frost.png` is an original transparent overlay with layered granular rime, intricate feather-like crystals, and an irregular clear center. It is blended with cached low-alpha grain and veils by the renderer. The image is a material input, not a baked desktop screenshot.

Generation direction: photorealistic winter-window ice; neutral white with subtle cool shadows; uneven peripheral thickness; a transparent center; no text, desktop, frame, backdrop, vector snowflakes, or opaque central tint.

## Work checkpoints

- Blocking first steps: inspect current renderer and confirm alpha-safe texture input before integration.
- Independent workstreams: parent supplies original material and validation; one worker owns renderer, Vue integration, and development preview.
- Shared mutable state: disjoint file ownership; round, input, and timing state remain in their existing owners.
- Smallest safe decomposition: one implementation owner keeps growth and erasure consistent between the app and preview.

## Shape

An immutable `IceSurface` holds the cached material canvas and its viewport dimensions. Pure rendering functions draw the material through the current growth mask and subtract erasure. The existing application owns round lifecycle and native blur updates. A separate development page reuses the same renderer for visual inspection.

A renderer class that owned both drawing and round/input state was rejected because it would duplicate the scheduler and Vue lifecycle.

## Verification

- `npm run build` passes with the texture bundled in production assets.
- Explicit TypeScript checking of `dev/ice-preview.ts` passes. The root Vue build does not include this development entry.
- Existing Rust suite passes: 51 tests, with the existing native callback test ignored by default.
- Browser inspection used the production renderer and growth mask at start, growth, hold, and completion. An actual pointer drag through the thick rim exposed the moving background; the sampled erased-core alpha was zero. Start and completion cleared the center. Dark and document backgrounds were inspected.
- A Vue watcher probe confirmed normal snapshot ticks do not restart desktop capture; a new round does. The integration uses separate watch sources for round ID and mode.
- Comment review removed three narration comments. Correctness review found no remaining actionable issues.
- Packaged macOS app built and launched at the user's stable authorized path. Fog mode was selected and preview invoked. Native desktop appearance remains subject to user visual acceptance; the browser check does not prove mixed-monitor native compositing.

## Limits

The user's macOS Reduce Transparency setting keeps native blur off. The center therefore adds translucent fixed grain and veils, not real optical refraction or background blur. The peripheral texture is more detailed; full visual parity with the generated C concept is not claimed. Windows and mixed-monitor visual acceptance remain pending.

Preview command: `npm run dev -- --host 127.0.0.1 --port 1422`, then open `/dev/ice-preview.html`. This page is outside the production app route. The local preview server is intentionally left available for this design review.
