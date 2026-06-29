<template>
  <div class="overlay-root" :class="`phase-${phase}`">
    <!-- Circuit traces on the edges during the warning -->
    <CircuitTraces :phase="phase" />
    <!-- Frosted blur of the frozen snapshot; clear center shows live desktop -->
    <CaptureLayer :src="capture" />
    <!-- Vignette + bottom hints -->
    <OverlayChrome :phase="phase" :hotkey="hotkey" />
    <!-- Holding-phase ethereal glow ring + rest text -->
    <EtherealGlow v-if="phase === 'holding'" />
  </div>
</template>

<script setup lang="ts">
import CircuitTraces from './CircuitTraces.vue';
import CaptureLayer from './CaptureLayer.vue';
import OverlayChrome from './OverlayChrome.vue';
import EtherealGlow from './EtherealGlow.vue';
import { usePhase, useCapture, useHotkey } from '../../composables/useBlink';

const { phase } = usePhase();
const { capture } = useCapture();
const { hotkey } = useHotkey();
</script>

<style>
/* The overlay window must be truly transparent so the clear center shows the
   live desktop. tokens.css sets an opaque body background for the app windows;
   override it for the overlay route only (this component renders only there). */
html,
body,
#app {
  background: transparent !important;
}

/* Animatable custom properties drive the iris. Registering them as <number>
   lets the browser interpolate them continuously on its animation timeline —
   so the spread is smooth (no JS rAF, no per-frame re-render, no stepping).
   --clear: clear-center radius fraction (per axis). --frost: blur layer opacity. */
@property --clear {
  syntax: '<number>';
  inherits: true;
  initial-value: 0.92;
}
@property --frost {
  syntax: '<number>';
  inherits: true;
  initial-value: 0;
}

.overlay-root {
  --clear: 0.92;
  --frost: 0;
  --blur: 18px;
}
/* Warning: big clear center, no blur (circuit traces are the cue). */
.overlay-root.phase-warning {
  --clear: 0.92;
  --frost: 0;
}
/* Spreading: iris closes edges→center over 20s, frost fades in. CSS-driven. */
.overlay-root.phase-spreading {
  animation: blink-spread 20000ms linear forwards;
}
/* Holding: stronger blur with a focal gradient — ~60% opaque at the center
   (where the glow ring sits, so the desktop shows faintly through it), ramping to
   FULLY opaque by ~30% out and staying fully opaque to the screen edges. */
.overlay-root.phase-holding {
  --clear: 0;
  --frost: 1;
  --blur: 30px;
}
.overlay-root.phase-holding .capture {
  /* Center is at least 80% opaque (<=20% transparent), ramping to fully opaque. */
  -webkit-mask-image: radial-gradient(
    ellipse farthest-side at 50% 50%,
    rgba(0, 0, 0, 0.8) 0%,
    #000 30%
  );
  mask-image: radial-gradient(
    ellipse farthest-side at 50% 50%,
    rgba(0, 0, 0, 0.8) 0%,
    #000 30%
  );
}
.overlay-root.phase-holding .vignette {
  /* No extra darkening at the focal center; gentle edge depth past 30%. */
  background: radial-gradient(
    ellipse farthest-side at 50% 50%,
    transparent 30%,
    rgba(24, 24, 37, 0.3) 100%
  );
  -webkit-mask-image: none;
  mask-image: none;
}
/* Dissolving: iris reopens over 10s, frost fades out. */
.overlay-root.phase-dissolving {
  animation: blink-dissolve 10000ms linear forwards;
}
.overlay-root.phase-monitoring {
  --clear: 0.92;
  --frost: 0;
}

@keyframes blink-spread {
  from {
    --clear: 0.92;
    --frost: 0;
  }
  to {
    --clear: 0;
    --frost: 0.9;
  }
}
@keyframes blink-dissolve {
  from {
    --clear: 0;
    --frost: 0.9;
  }
  to {
    --clear: 0.92;
    --frost: 0;
  }
}
</style>

<style scoped>
.overlay-root {
  position: fixed;
  inset: 0;
  background: transparent;
  pointer-events: none;
}
</style>
