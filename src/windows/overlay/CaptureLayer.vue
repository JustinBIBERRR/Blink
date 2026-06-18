<template>
  <!-- Frozen desktop snapshot, Gaussian-blurred, revealed by a radial OPACITY
       gradient: transparent at the clear radius (the transparent window shows the
       LIVE desktop through it) easing to opaque blur at the edges. No window
       region, so the clear->blur transition is a soft anti-aliased gradient with
       no edge line. As holeFrac -> 0 the blur covers the whole screen. -->
  <img v-if="src" class="capture" :src="src" :style="blurStyle" alt="" draggable="false" />
</template>

<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  src: string;
  holeFrac: number;
  blurOpacity: number;
}>();

const blurStyle = computed(() => {
  const f = Math.max(props.holeFrac, 0);
  const inner = f * 100;
  let mask: string;
  if (f <= 0.02) {
    mask = 'none';
  } else {
    // Stitch iris feel: small clear core, wide soft ramp to full blur at the
    // edge. Transparent at the clear radius (live desktop shows), easing opaque.
    const span = 100 - inner;
    const s1 = (inner + span * 0.18).toFixed(1);
    const s2 = (inner + span * 0.45).toFixed(1);
    const s3 = (inner + span * 0.72).toFixed(1);
    mask =
      `radial-gradient(ellipse farthest-side at 50% 50%,` +
      ` transparent ${inner.toFixed(1)}%,` +
      ` rgba(0,0,0,0.12) ${s1}%,` +
      ` rgba(0,0,0,0.45) ${s2}%,` +
      ` rgba(0,0,0,0.82) ${s3}%,` +
      ` #000 100%)`;
  }
  return {
    opacity: props.blurOpacity,
    WebkitMaskImage: mask,
    maskImage: mask,
  };
});
</script>

<style scoped>
.capture {
  position: fixed;
  inset: 0;
  width: 100vw;
  height: 100vh;
  object-fit: cover;
  /* Stitch-matched frosted blur (design uses blur(12px)); bright + lightly
     desaturated so the desktop reads through it as translucent frost. */
  filter: blur(12px) saturate(0.96) brightness(0.95);
  /* Scale up slightly so the blur doesn't sample past the image edge. */
  transform: scale(1.06);
  pointer-events: none;
  transition: opacity 0.3s ease;
  will-change: opacity, mask-image;
}
</style>
