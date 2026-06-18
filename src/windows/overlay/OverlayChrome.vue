<template>
  <!-- Vignette + seam glow over the blurred snapshot. No backdrop-filter here:
       the real blur comes from CaptureLayer; this layer only darkens toward the
       edges and draws a faint blue seam at the moving hole boundary. -->
  <div class="vignette" :style="vignetteStyle" />

  <!-- Bottom hints (sit at the bottom edge = inside the frame). -->
  <template v-if="phase === 'spreading' || phase === 'holding' || phase === 'dissolving'">
    <div v-if="phase === 'spreading'" class="bottom-hint bright">
      <kbd>Ctrl+Shift+Space</kbd> 跳过
    </div>
    <div v-if="phase === 'holding'" class="bottom-hint dim">
      <kbd>Ctrl+Shift+Space</kbd> 提前退出
    </div>
    <div v-if="phase === 'dissolving'" class="bottom-hint green">
      ✓ 护眼完成，恢复中…
    </div>
  </template>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import type { Phase } from '../../types/blink';

const props = defineProps<{
  phase: Phase;
  holeFrac: number;
  blurOpacity: number;
}>();

// Deep vignette over the blur, using the Stitch stops (transparent core →
// rgba(24,24,37,…) deepening to the edge), eased to follow the blur mask band so
// the two darken together smoothly (no step). Edge alpha follows the design.
const vignetteStyle = computed(() => {
  const f = props.holeFrac;
  const inner = Math.max(f * 100, 0);
  const span = 100 - inner;
  const s1 = (inner + span * 0.3).toFixed(1);
  const s2 = (inner + span * 0.7).toFixed(1);
  const bg =
    f <= 0.02
      ? `radial-gradient(ellipse farthest-side at 50% 50%, rgba(24,24,37,0.10), rgba(24,24,37,0.50) 100%)`
      : `radial-gradient(ellipse farthest-side at 50% 50%,
           transparent ${inner.toFixed(1)}%,
           rgba(24,24,37,0.10) ${s1}%,
           rgba(24,24,37,0.30) ${s2}%,
           rgba(24,24,37,0.50) 100%)`;
  return { background: bg, opacity: props.blurOpacity };
});
</script>

<style scoped>
.vignette {
  position: fixed;
  inset: 0;
  pointer-events: none;
  transition: opacity 0.3s ease;
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
.bottom-hint.bright { color: rgba(205, 214, 244, 0.75); }
.bottom-hint.dim    { color: rgba(205, 214, 244, 0.3); border-color: rgba(255, 255, 255, 0.04); }
.bottom-hint.green  { color: rgba(166, 227, 161, 0.8); }
kbd {
  background: rgba(255, 255, 255, 0.12);
  border: 1px solid rgba(255, 255, 255, 0.2);
  border-radius: 3px;
  padding: 1px 6px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: rgba(205, 214, 244, 0.75);
}
</style>
