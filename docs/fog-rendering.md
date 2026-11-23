# Fog rendering and acceptance

The scheduler fixes the selected mode for each round: 3 seconds of edge cue,
then 20 seconds of fog (5 seconds growth, 12 holding, 3 fade). All monitors
consume the same round ID and elapsed time. Native close controls are separate
48-point nonfocusable windows; visual windows always ignore mouse input.

macOS uses `NSVisualEffectView` with behind-window blending, active state and a
160×90 alpha `maskImage`. The alpha is sent at at most 20 Hz, and checked against
the current round again on the AppKit thread. The WebView draws only frost,
droplets and tint. This uses the compositor's live desktop, without screen capture.
The operating system's material/accessibility settings may affect appearance.

Windows captures the bounds of each overlay's monitor through GDI, including
negative monitor origins, downsampled by two. It blurs the captured image only
inside the fog's alpha mask. This image is frozen during the round. Transparent
areas contain no screenshot and show the actual live desktop. Capture exclusion
is requested for the visual windows to prevent overlay feedback. This needs
Windows 10 2004 or later and actual-machine verification.

Eraser integration should retain strokes by round and monitor, subtract their
alpha from both the native mask and the **complete final decoration canvas**
(including frost and droplets). Do not replace erased pixels with captured
pixels. Controls can grow to include the holder without making the full-screen
visual surface interactive. Late mask messages cannot reopen an ended round.

Automated checks cover selected duration, early close, stale close, mode changes
during an active round, configuration migration and persistence. Native code
compiles on macOS. Remaining packaged-app acceptance: moving desktop content
under the native blur and clear frontier, multi-monitor alignment and mixed DPI,
close controls without focus changes, and Windows capture/compositing. These are
manual release checks, not proven by the lifecycle tests or frontend build.

## Shared eraser (#9)

The optional eraser is off by default. Its held state and persistent global stroke
segments belong to Scheduler and retain the original round deadline. Every monitor
projects the same state. Coordinates use CoreGraphics desktop points on macOS
(native NSWindow bounds account for negative origins) and physical pixels on Windows.
The complete decoration canvas and native blur mask both subtract the stroke mask;
transparent holes expose live desktop pixels, never a replacement screenshot.

Only a small holder/close window accepts ordinary clicks. Fullscreen visuals remain
pass-through. Active CGEventTap / WH_MOUSE_LL callbacks selectively consume owned
left presses and their paired releases, queue events to the scheduler, and pass
right/middle buttons through. Ending any round clears atomic capture eligibility
immediately; an already swallowed press still consumes its matching release.
Accessibility denial leaves pickup inactive and the independent close control usable.
macOS asks only when an enabled eraser is explicitly picked up.

Automated scheduler coverage verifies shared pickup, multiple strokes (including
negative desktop coordinates), return without losing marks, sleep cleanup and stale
round rejection. Native compilation and frontend build do not prove native input:
release acceptance still requires both platforms with mixed-DPI monitors, original
application drags, right/middle clicks, ending mid-drag, denied Accessibility, and
moving desktop content visible through erased frost/droplets. SVG felt/holder art is
an implementation of the approved oblique reference, pending visual acceptance.
