# Native ice prototype

## Goal

Match the C reference's thick central ice, muted desktop and fine perimeter frost in a live desktop prototype. Measure rendering cost and resource release before integrating it into Blink reminders. Visual acceptance remains the user's decision.

## Design

Use a standalone macOS laboratory app with ScreenCaptureKit and Metal. One session owns capture, latest frame, renderer and lifecycle. Use an explicit idle/starting/running/stopping/failed state. Exclude the laboratory app from capture to avoid recursive feedback. Process screen pixels locally without recording or uploading them.

The alternative was transporting desktop frames through the existing WebView. The native session avoids repeated frame serialization and keeps GPU resources under one owner. Production reminder behavior stays unchanged while the material is evaluated.

An ice-thickness field drives scattering, background blur, small refractive offsets and local melting. Fine frost texture supplies surface detail. Synthetic backgrounds are clearly labelled and used for repeatable checks; they are not evidence of live capture performance.

## Work sequence

1. Build a native session and metadata-only monitoring before visual tuning.
2. Build the layered ice material and local thaw in the same renderer.
3. Compile and run an automated synthetic lifecycle test.
4. Inspect the native window, exercise live capture where permission permits, and review telemetry.
5. Record limits, visual acceptance status and integration readiness.

## Throughput checkpoint

- Blocking first steps: inspect the existing renderer, choose session ownership and define metrics.
- Independent workstreams: implementation owns `dev/native-ice`; parent owns this plan, review and verification evidence.
- Shared mutable state: one implementation owner controls capture and GPU lifetime. No concurrent production changes.
- Smallest safe decomposition: one native code owner keeps capture shutdown and renderer resource release consistent.

## Verification contract

- Rendering runs at a bounded target cadence with a bounded latest-frame buffer.
- Statistics identify source mode, capture frames, rendering frames, errors, GPU execution time, process CPU and resident memory.
- Input timing names its endpoint; CPU submission is not reported as display latency.
- Stop ends capture and draw scheduling, drains pending GPU work and releases session textures. Idle samples expose remaining activity.
- The first measurement is a descriptive baseline, not a speedup claim or a promise about multi-display performance.
- Synthetic and live runs are reported separately. Permission failures and unavailable metrics are explicit.
- Initial scope is one selected display in a preview window. Full desktop overlays and simultaneous multi-display reminders require a later integration check.

## Sources

- [Apple screen capture sample](https://developer.apple.com/documentation/screencapturekit/capturing-screen-content-in-macos)
- [ScreenCaptureKit content filter](https://developer.apple.com/documentation/screencapturekit/sccontentfilter)
- [Metal command buffer GPU end time](https://developer.apple.com/documentation/metal/mtlcommandbuffer/gpuendtime)

## Initial measurement environment

The development machine reports macOS 26.5.1, Swift 6.3.3 and 18 logical CPU cores. The initial load average was 1.59/2.05/2.16. WindowServer and Codex were active during development. Measurements from this session describe the local prototype under that background load; they do not establish performance on lower-power Macs.

A 30 fps target implies a 33.3 ms frame budget. The first run will report actual distributions rather than claim that a configured frame rate was achieved. Process CPU excludes WindowServer and capture service work; GPU command duration excludes display latency and other processes. Resident memory is not a direct GPU allocation or leak measurement.
