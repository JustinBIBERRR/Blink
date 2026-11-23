<template>
  <!-- Deep vignette over the blur. No backdrop-filter; the real blur is in
       CaptureLayer. Driven by the same --clear/--frost CSS vars so it darkens the
       edges in lock-step with the blur, smoothly (CSS-animated, no stepping). -->
  <div class="vignette" />

  <!-- Bottom hints (sit at the bottom edge = inside the frame). -->
  <template v-if="phase === 'spreading' || phase === 'holding' || phase === 'dissolving'">
    <div v-if="phase === 'dissolving'" class="bottom-hint green">
      ✓ 护眼完成，恢复中…
    </div>
  </template>
</template>

<script setup lang="ts">
import type { Phase } from '../../types/blink';

defineProps<{ phase: Phase }>();
</script>

<style scoped>
.vignette {
  position: fixed;
  inset: 0;
  pointer-events: none;
  opacity: var(--frost, 0);
  transition: opacity 0.4s ease, background 0.5s ease;
  /* Stitch deep-vignette stops: transparent clear core → rgba(24,24,37,…)
     deepening to the edge. Inner radius tracks --clear so it darkens in
     lock-step with the blur iris. */
  background: radial-gradient(
    ellipse farthest-side at 50% 50%,
    transparent calc(var(--clear, 0.92) * 100%),
    rgba(24, 24, 37, 0.3) 70%,
    rgba(24, 24, 37, 0.5) 100%
  );
}


.bottom-hint {
  position: fixed;
  bottom: 24px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 11px;
  font-family: Inter, 'Segoe UI', sans-serif;
  background: rgba(0, 0, 0, 0.4);
  padding: 5px 16px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  white-space: nowrap;
  pointer-events: none;
  text-shadow: 0 1px 6px rgba(0, 0, 0, 0.8);
}
.bottom-hint.green  { color: rgba(166, 227, 161, 0.8); }
</style>
