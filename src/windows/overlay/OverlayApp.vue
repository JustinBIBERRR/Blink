<template>
  <div class="overlay-root">
    <!-- Circuit traces on the edges during the warning -->
    <CircuitTraces :phase="phase" />
    <!-- Real blur of the frozen snapshot in the ring outside the live hole -->
    <CaptureLayer :src="capture" :hole-frac="holeFrac" :blur-opacity="blurOpacity" />
    <!-- Vignette, seam glow, bottom hints -->
    <OverlayChrome :phase="phase" :hole-frac="holeFrac" :blur-opacity="blurOpacity" />
    <!-- Holding-phase ethereal glow ring + rest text -->
    <EtherealGlow v-if="phase === 'holding'" />
  </div>
</template>

<script setup lang="ts">
import CircuitTraces from './CircuitTraces.vue';
import CaptureLayer from './CaptureLayer.vue';
import OverlayChrome from './OverlayChrome.vue';
import EtherealGlow from './EtherealGlow.vue';
import { usePhase, useCapture } from '../../composables/useBlink';
import { useOverlayAnim } from '../../composables/useOverlayAnim';

const { phase } = usePhase();
const { capture } = useCapture();
const { holeFrac, blurOpacity } = useOverlayAnim(phase);
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
</style>

<style scoped>
.overlay-root {
  position: fixed;
  inset: 0;
  background: transparent;
  pointer-events: none;
}
</style>
