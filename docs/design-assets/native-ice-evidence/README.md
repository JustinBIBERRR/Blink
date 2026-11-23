# Native ice verification

The synthetic test ran once after the lifecycle and material revisions. This is a descriptive local baseline, not a comparative benchmark or a live-capture performance claim.

`synthetic.jsonl` contains the complete metadata-only run. `checks.txt` contains the independent verifier output. `startup-stop.jsonl` contains a delayed synthetic startup, cancellation, rejected premature restart, successful restart and drained stop. It exercises session sequencing without claiming to emulate ScreenCaptureKit's internal behavior.

The window rendered at 2216×1136 pixels. Across ten active one-second samples, the median per-interval frame p95 was 33.829 ms. The median per-interval GPU p50 was 1.819 ms. These are distributions of interval summaries, not percentiles computed over all individual frames. Median process CPU was 2.592% of one logical core and median RSS was 106.438 MiB. Stop left zero session-owned textures and no in-flight commands. Median post-stop RSS remained 94.203 MiB; framework and driver caches remain allocated.

The machine was used interactively during testing. Baseline CPU varied and startup effects are visible. CPU excludes WindowServer and capture service work. The source was synthetic, so Gaussian blur was cached after its first source frame. Live capture will add capture, texture conversion and repeated blur costs.

Native visual inspection found the initial center excessively blurred. The revised material lowers blur, blends clarity by thickness and preserves source aspect ratio. User acceptance of the C match is pending.

Live capture reached the permission gate. After the user enabled permission, actual ScreenCaptureKit access still returned error -3801. Read-only TCC diagnostics confirmed that the stored code requirement referenced an older ad-hoc binary hash. The early CoreGraphics preflight gate was removed; actual ScreenCaptureKit startup now decides access and its error stays visible after resource cleanup. The old ScreenCapture authorization was reset only for `com.blink.ice-lab`, and the user was asked to authorize the fixed binary again. Multi-display capture and full desktop transparency are not verified by this windowed prototype.

Rerun the independent check from the repository root:

```sh
python3 scripts/check-ice-lab.py docs/design-assets/native-ice-evidence/synthetic.jsonl
```

The verifier also rejected three intentionally corrupted copies of a real log: unfinished GPU work, retained textures after stop and a cleanup error. These temporary negative controls were not committed.

## Live desktop follow-up

The current binary was subsequently authorized successfully. `live.jsonl` and `live-checks.txt` supersede the permission-blocked status for this binary. Native inspection showed the actual desktop through the ice, local pointer erasure and a fully clear preview after center-out melt. The app excluded its own window from capture.

One live run produced 69 active one-second samples. Median process CPU was 3.330% of one logical core, median RSS 105.016 MiB, median per-interval frame p95 33.951 ms and median per-interval GPU p50 1.866 ms. The maximum interval p95 was 52.708 ms, so the run was not perfectly paced. UI automation ran concurrently. The input measurement ends at CPU submission, not screen presentation.

The run submitted and completed 2051 GPU frames and accepted 1967 captured frames. Three unread captured frames were replaced; there were no reported GPU errors. After stop, all frame counters stayed fixed, capture and rendering were inactive, session textures and in-flight work were zero. Post-stop median process CPU was 0.316% and median RSS was 99.141 MiB.

This is a single-display windowed baseline on this machine. The desktop contained ordinary application content and UI updates, not a controlled high-motion video workload. Concurrent multi-display cost, lower-power Macs and full-overlay behavior remain unverified. The low-resolution nearest-sampled contact map produces visible jagged edges after thaw; improving that material edge is an open prototype item. No new binary was installed after successful authorization.
