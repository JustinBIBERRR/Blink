# Blink v1 release acceptance (#11)

Status: **release blocked; #11 remains open**. Builds and deterministic tests do
not establish native visual or input acceptance. Windows manual acceptance belongs
to the project author, per [the specification](../blink-v1-spec.md).

## Build identity and evidence

- Source baseline: integration commit `b47f1df` (tickets #2–#10), plus the #11
  delivery commit containing this record. Version `0.1.0`, bundle ID
  `dev.blink-rs.app`.
- Local build host: macOS **26.5.1 (25F80), arm64**; Rust **1.100.0**,
  Node **26.11.1**, npm **11.20.0**. Build command:
  `npm run tauri build -- --bundles app dmg`.
- Local build output/log: `/private/tmp/blink-target11/release/bundle/`,
  `/private/tmp/blink-ticket11-build.log`. This is a local unsigned/unnotarized
  build (Mach-O ad-hoc linker signature; no Developer ID), not a signed distribution release.
  App compilation and frontend typecheck/build passed. Tauri’s DMG script was
  blocked by sandbox disk-device access; the same app plus an Applications link
  was packaged with approved `hdiutil create -volname Blink -srcfolder
  /private/tmp/blink-ticket11-dmg -ov -format UDZO <output.dmg>`.
  `hdiutil verify` passed.
  DMG: `/private/tmp/blink-target11/release/bundle/dmg/Blink_0.1.0_aarch64.dmg`.
  SHA-256: `bf194749b4413762d662eab9d966fdb3eaba5ebd3fe1fa8ce4eee2f7c4c45942`.
  App executable SHA-256: `db01c3ed7e1ea570efc84b025fa033fa1d39109e0afeb960d046c896953955da`.
- Regression log: `/private/tmp/blink-ticket11-tests.log`; native callbacks:
  `/private/tmp/blink-ticket11-native-tests.log`. **52 passed**, two singleton
  tests intentionally ignored in the parallel suite; **both passed separately**
  with `cargo test native_ --lib --offline -- --ignored --test-threads=1`.
  `cargo fmt --check` and `git diff --check` passed.
- Previous Windows build: [Actions run 45128537712](https://github.com/JustinBIBERRR/Blink/actions/runs/45128537712),
  commit `46bdc20`. That build predates ticket #10 and is **not** evidence for the
  final release candidate. Record the final integration run and its commit here
  before accepting Windows packaging.
- [Windows workflow](../../.github/workflows/build-windows.yml) produces NSIS/MSI;
  [macOS workflow](../../.github/workflows/build-macos.yml) produces a DMG.
  Both run Rust behavior tests, isolated native callbacks and frontend typecheck
  plus production build before packaging. CI uses Node 20 and stable Rust;
  its runner image/version must be taken from the actual run log.

## Acceptance matrix

| Area | Automated evidence | macOS packaged app | Windows installed app |
| --- | --- | --- | --- |
| Default soft reminder, preview, selected mode | Scheduler lifecycle and config tests | Pending visual/input QA | Author pending |
| Interval, low-input reading, restart | Controlled-time scheduler tests | Real timer pending | Author pending |
| Five-minute idle / 30-second return buffer, one pending round | Scheduler public-operation tests | OS idle signal pending | Author pending |
| Pause, resume, deadline and restart persistence | Scheduler and storage roundtrip tests | Tray/settings/relaunch pending | Author pending |
| Lock/sleep interruption and remaining use time | System-event seam tests | Physical lock/sleep pending | Author pending |
| Fullscreen/game/presentation silence | Scheduler silence tests | Not applicable | Real Windows signals pending |
| Fog prelude, growth, fade, per-screen close | Lifecycle and stale-round tests | Native visual QA pending | Author pending |
| Silent start and natural-end notification; disabled/denied/DND | Submission count/timing tests | Permission/banner/audio pending | Identity/banner/audio pending |
| Multi-monitor alignment, live desktop and mixed DPI | Cannot establish with unit tests | Multi-screen QA pending | Author pending |
| Shared pickup, strokes, return, radius and deadline | Scheduler and native callback tests | Accessibility and native input pending | Native input pending |
| Modifier, double-click, optional shortcut | Scheduler/config/callback tests | Both modifier sides and timing pending | Author pending |
| Mid-drag close, pause, lock, sleep; restored ordinary input | Scheduler cleanup and callback filtering | Native interruption QA pending | Author pending |
| Installation and first-launch/reopen settings | Native compilation | App/DMG build; launch QA pending | NSIS/MSI final build/install pending |

## Packaged-app procedure

Open the app normally: an absent or unreadable configuration shows Settings.
The tray/menu-bar Settings item always opens it; reopening a hidden macOS app
opens Settings. For repeatable QA with an existing configuration, launch the
bundle executable with `--settings` (or `open Blink.app --args --settings` when
it is not already running). No QA controls are exposed in the product UI.

Use at least two monitors, including a negative desktop origin and different
scales. Keep moving content and a scrollable document underneath the overlay.
First check soft mode and timing, then fog and notifications, then eraser input.
For each matrix row record OS build, source commit, artifact, monitor layout,
permissions, observed result and screenshot/video/log link. A failed or unavailable
case remains a blocker; record permission denial as a distinct case from granted
input access. Notification tests must distinguish submission from a visible banner
and verify silence with sound enabled.

## Known release blockers and limits

- Final Windows artifact build/install and all author-owned Windows manual checks.
- Actual macOS and Windows multi-monitor visuals, input delivery, interruption,
  notification permission/DND/audio and installation acceptance.
- macOS unsigned local build is not notarized for public distribution.
- Windows fog currently freezes the captured image under the remaining fog;
  clear/erased pixels expose the live desktop. If acceptance requires live motion
  under the blur as well, this is a functional blocker, not a test limitation.
- macOS lock notifications use undocumented compatibility signals: verify on the
  tested OS version. Accessibility and system material settings affect native
  input and appearance.

Related evidence and constraints: [use-time validation](use-time.md),
[notification validation](ticket-8-notifications.md), and
[fog/eraser rendering and native acceptance](../fog-rendering.md).

## Review updates and quick local acceptance

The review fixes remove every close-shortcut setting, recorder and dispatcher.
Existing config files remain readable; their old `hotkey` value is discarded on
save. Exit uses × (including with the eraser disabled) or a docked-eraser double
click. Temporary modifier pickup remains available.

Launch the rebuilt macOS app directly for short local cycles:

```sh
/path/to/Blink.app/Contents/MacOS/blink-rs --settings --test-interval-secs 5
```

On Windows use `blink-rs.exe --settings --test-interval-secs 5`. This process-only
interval appears as `5 sec · 本次测试` in settings. The ordinary saved interval
and production default of 1200 seconds are unchanged by save or preview. The
interval slider is disabled during this local override. Soft reminders still
last 3 seconds; fog still lasts 3 + 20 seconds. In this acceptance mode only, the
next interval begins after natural or manual ending, leaving five clear seconds
to observe dismissal. Launch without the flag to resume the saved schedule.
`BLINK_DEV_INTERVAL=5` is retained as an equivalent local override.

Automatic reminders and previews now share one window-readiness barrier before
the scheduler starts the round clock, resets the ordinary interval or queues a
notification. Pause, lock, silence and replacement invalidate stale preparations.
The soft CSS cue catches up once per round instead of advancing twice as fast.

The earlier build identities and test totals above are historical evidence for
#11, not acceptance of this revised candidate. Native UI and Windows acceptance
must be repeated on the rebuilt candidate.
