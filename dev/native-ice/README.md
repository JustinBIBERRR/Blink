# Blink native ice lab

This standalone macOS prototype evaluates the C-style thick ice material and its runtime cost before replacing Blink's production overlay. It does not change Blink's settings, timer, permissions, or installed application.

## Build and run

Requires Apple Silicon and a macOS SDK with ScreenCaptureKit (deployment target macOS 13).

```sh
bash dev/native-ice/build.sh
'/Volumes/代码/projects/Blink-artifacts/2026-11-23/Blink Ice Lab.app/Contents/MacOS/BlinkIceLab'
```

The app has a separate bundle identity, `com.blink.ice-lab`. The build is locally ad-hoc signed. Live capture requires Screen Recording permission for **Blink Ice Lab**, independently of Blink's Accessibility permission. The live button invokes ScreenCaptureKit, which enforces permission and reports any access failure. The app does not use CoreGraphics preflight as a separate gate. No permission is needed for synthetic preview.

- **合成预览** renders a generated test document. The moving gold marker identifies the animated synthetic mode. This is not a live-capture performance test.
- **实时桌面** captures the selected display with ScreenCaptureKit, excluding this lab's own application. The preview samples the latest complete frame, with a bounded one-frame mailbox. Replaced unread frames are counted.
- Drag in the preview to thaw locally. Each new gesture starts independently. The transition lasts about 650 ms with a slightly slower outer edge.
- **中央消融** clears from center to perimeter over three seconds.
- **重置冰层** restores the material and removes contact history.
- **停止** stops the draw timer, closes the capture mailbox, awaits stream shutdown and GPU completion, then drops material and frame textures. The last displayed preview remains visible, but no new draws occur.
- **显示/隐藏监控** toggles only the displayed statistics. Metadata-only logging continues at one sample per second.

Live capture is capped at 1600 pixels wide and requests 30 fps. Rendering targets 30 fps; the output uses the window's Retina drawable size. The source is aspect-fitted with letterboxing instead of stretching the desktop. Background Gaussian blur runs only when a new source frame arrives. Fine frost texture remains sampled at output resolution. Completed GPU work is limited to one command buffer in flight. Contact history uses a bounded 480×300 timestamp texture, updated only while the previous GPU command has completed.

## Reproducible lifecycle test

```sh
'/Volumes/代码/projects/Blink-artifacts/2026-11-23/Blink Ice Lab.app/Contents/MacOS/BlinkIceLab' \
  --self-test --log /private/tmp/blink-ice-lab-self-test.jsonl
```

The test records three seconds of idle baseline, starts synthetic rendering, injects two contacts through the lab's own developer test method, dissolves the ice, stops, records five seconds after stop, and exits. It deliberately does not request Screen Recording permission. This validates the synthetic render/lifecycle path; it does not establish live desktop or multi-display performance.

To exercise cancellation while startup is suspended and verify restart is blocked until cleanup completes:

```sh
'/Volumes/代码/projects/Blink-artifacts/2026-11-23/Blink Ice Lab.app/Contents/MacOS/BlinkIceLab' \
  --startup-stop-test --log /private/tmp/blink-ice-startup-stop.jsonl
```

This uses delayed synthetic startup, so it requires no capture permission. Assertions verify that the cancelled session never renders and a subsequent session starts and drains successfully.

## Telemetry contract

Each JSONL row includes `event`, wall-clock `timestamp`, lifecycle `state`, `source`, drawable `width`/`height`, and these fields. Optional unavailable values are omitted, never substituted with zero.

| Field | Meaning |
|---|---|
| `renderedFrames` | Cumulative submitted draw commands, not successful presentation count |
| `completedFrames` | Cumulative completed command buffers, including failed buffers |
| `gpuErrors` | Cumulative texture conversion/allocation/encoding/command errors |
| `skippedDraws` | Draw opportunities skipped for in-flight work or unavailable resources/drawables |
| `captureFrames` | Complete capture frames accepted this session |
| `replacedFrames` | Unread captured frames replaced in the single-frame mailbox |
| `frameP50Ms`, `frameP95Ms` | Intervals between submitted draw commands since previous log row; not compositor presentation timings |
| `gpuP50Ms` | Median completed GPU command duration since previous row, from Metal timestamps; includes Gaussian prepass when needed |
| `frameAgeMs` | Time since the latest used capture frame reached this process; excludes upstream capture latency |
| `inputToSubmitMs` | Most recent contact-to-draw-submission delay; not input-to-visible-pixel latency; retained until next contact/reset |
| `cpuPercent` | Process user+system CPU delta / elapsed wall time ×100; 100% means one logical CPU core |
| `residentMB` | Process resident bytes / 1,048,576; includes framework/driver caches |
| `inFlight` | Command buffers submitted but whose completion handler has not retired |
| `texturesHeld` | Source, preblurred source, frost, contact textures held by session; excludes view drawables/framework allocations |
| `captureActive`, `renderActive` | Capture lifecycle and draw timer state |

Use regular one-second `sample` rows for CPU comparisons. Lifecycle events retain the latest complete interval CPU measurement instead of computing a short-window percentage. Initial shader compilation occurs before baseline sampling; first material upload and Gaussian setup occur during startup. A stopped session is verified by stable submitted/completed/capture counts, no active capture or rendering, zero in-flight commands, and zero session-held textures. RSS need not return exactly to initial values because Metal/AppKit retain caches.

No screen images, window titles, or pixel data are written to the log. Default logs are `/private/tmp/blink-ice-lab-<timestamp>.jsonl`; `--log` chooses an explicit file and replaces its previous contents.

## Limits still requiring acceptance

- Windowed preview only. Fully thawed pixels show the captured desktop in the preview, not an actual transparent hole in a desktop overlay.
- One selected display at a time. Multi-display concurrent performance and hot-plug handling are not certified.
- Runtime uses a shared thickness field, static crystal artwork, Gaussian scattering and subtle refraction. This is an artistic material, not a physical phase-change solver.
- ScreenCaptureKit frame rates can drop on an unchanged desktop. Capture age alone is not proof of lag.
- No automatic quality adjustment or persistent monitoring daemon. No claim of an accepted resource budget until real live runs are measured.
