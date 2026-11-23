# AE ice reference observations

The user supplied `光厂_27459229_结冰冰冻工程文件.mp4` as a visual reference during the native prototype work. The local video is about 20 seconds. Exact-time keyframes were inspected; the supplied file is a preview video, not an inspected AE project.

## Visible behavior

Early frames retain the original scene. Small frost patches appear at the corners and bottom before large crystal groups spread inward. By the later sampled frames, directional striations and irregular icy regions distort the background while much of its original color remains visible. The right-hand preview displays the isolated-looking white effect over a checkerboard, but that presentation alone does not establish an alpha channel in this MP4.

## Implications for Blink

Keep C's muted central thickness as the target. Borrow the reference's directional crystal groups and refractive surface detail. The current native lab establishes live-frame sampling, a shared thickness field, local thaw and monitoring; it still uses static perimeter artwork and a low-frequency refractive field. It does not yet reproduce the reference's detailed crystal-growth sequence.

For a later material iteration, use coordinated crystal coverage, surface-normal and growth-arrival maps. Growth time can reveal crystal groups without running a physical crystallization solver. Use the same maps when thawing, with fine crystals disappearing before thicker structures. Actual quality and GPU cost need a new measured run after that change.

The provided preview is kept outside the repository. No third-party video frames or AE assets were bundled into the app. Inspect an editable project or exported material maps before deciding whether to reuse its assets directly.
