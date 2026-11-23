# Ice thaw behavior

The user requested natural center-out dissipation and gradual melting after eraser contact.

## Behavior

The final three seconds of the existing fog round clear the middle first. An irregular soft melt front expands toward the perimeter. Dense edge frost and corners clear last. The round still ends at the existing deadline, with no residual ice.

Eraser contact starts a short local thaw. Each new stroke retains its first-contact time. It thins immediately, becomes fully clear after the transition, and stays clear until the round ends. Repeated snapshots do not restart the transition. Releasing the mouse does not stop thaw already in progress.

The same evolving erase mask removes both native blur and visible frost. Overlapping fresh segments must not skip the transition through opacity accumulation.

## Shape and checkpoints

`TimedEraserStroke` combines a stroke segment with its first-contact time. The view owns the per-round append-only list and resets it with the round. Shared rendering functions consume this data in both the application and development preview.

- Blocking first steps: inspect current growth and erase masks, define shared timing and alpha ownership.
- Independent workstreams: one implementation owner changes renderer/integration/preview; parent owns specification and verification.
- Shared mutable state: no backend stroke format changes. The existing scheduler remains the owner of round lifetime and persistent stroke coordinates.
- Smallest safe decomposition: one code owner keeps native and decoration masks synchronized.

Per-stroke alpha fading through ordinary source-over compositing was rejected because overlapping fresh segments could become immediately clear. The mask combines coverage without that accumulation.

## Verification

- Production frontend build and standalone preview TypeScript check pass.
- Browser `Check melt` passes all 13 checks, including monotonic center-out clearing, fully transparent completion, progressive contact, overlapping strokes, mature-stroke persistence, cache reset and display-coordinate reprojection.
- Desktop-aspect preview at 19 seconds retains perimeter frost while the center is clear. At 20 seconds the visible frost is gone.
- Independent review found no correctness issues. Removed one redundant preview comment.
- Native blur keeps its existing 160×90 mask and 20 Hz update cadence. macOS Reduce Transparency still uses the existing no-native-blur fallback. Native visual acceptance remains manual.
