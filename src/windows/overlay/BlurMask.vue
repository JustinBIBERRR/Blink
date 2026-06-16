<template>
  <div
    class="blur-wrap"
    :class="{ active: isActive }"
    :style="maskStyle"
  >
    <!-- Holding phase: center hint -->
    <div v-if="phase === 'holding'" class="center-hint">
      <div class="eye-ring">👁</div>
      <span class="zh">望向 6 米外</span>
      <span class="en">LOOK 20 FT AWAY</span>
    </div>

    <!-- Bottom hints -->
    <div v-if="phase === 'spreading'" class="bottom-hint bright">
      <kbd>Ctrl+Shift+Space</kbd> 跳过
    </div>
    <div v-if="phase === 'holding'" class="bottom-hint dim">
      <kbd>Ctrl+Shift+Space</kbd> 提前退出
    </div>
    <div v-if="phase === 'dissolving'" class="bottom-hint green">
      ✓ 护眼完成，恢复中…
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, onUnmounted } from 'vue';
import type { Phase } from '../../types/blink';

const props = defineProps<{ phase: Phase }>();

// ellipseR: percentage of clear center. 100% = fully clear, 11% = max blur
const ellipseR = ref(100);
let animFrame = 0;

function animate(from: number, to: number, durationMs: number) {
  cancelAnimationFrame(animFrame);
  const start = performance.now();
  function step(now: number) {
    const t = Math.min((now - start) / durationMs, 1);
    // Ease in-out cubic
    const eased = t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
    ellipseR.value = from + (to - from) * eased;
    if (t < 1) {
      animFrame = requestAnimationFrame(step);
    }
  }
  animFrame = requestAnimationFrame(step);
}

watch(
  () => props.phase,
  (newPhase) => {
    cancelAnimationFrame(animFrame);
    if (newPhase === 'spreading') {
      ellipseR.value = 100;
      animate(100, 11, 20000);
    } else if (newPhase === 'holding') {
      ellipseR.value = 11;
    } else if (newPhase === 'dissolving') {
      animate(11, 100, 10000);
    } else {
      ellipseR.value = 100;
    }
  },
  { immediate: true }
);

onUnmounted(() => cancelAnimationFrame(animFrame));

const isActive = computed(() =>
  ['spreading', 'holding', 'dissolving'].includes(props.phase)
);

const maskStyle = computed(() => {
  if (!isActive.value) return {};
  const r = ellipseR.value;
  return {
    WebkitMaskImage: `radial-gradient(ellipse ${r}% ${r}% at 50% 52%, transparent 0%, rgba(0,0,0,0.05) 40%, black 70%)`,
    maskImage: `radial-gradient(ellipse ${r}% ${r}% at 50% 52%, transparent 0%, rgba(0,0,0,0.05) 40%, black 70%)`,
  };
});
</script>

<style scoped>
.blur-wrap {
  position: fixed;
  inset: 0;
  background: rgba(10, 10, 20, 0.4);
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.5s ease;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
}
.blur-wrap.active {
  opacity: 1;
}
.center-hint {
  text-align: center;
}
.eye-ring {
  font-size: 26px;
  width: 54px;
  height: 54px;
  border-radius: 50%;
  border: 1px solid rgba(205, 214, 244, 0.12);
  display: flex;
  align-items: center;
  justify-content: center;
  margin: 0 auto 12px;
  opacity: 0.75;
}
.zh {
  display: block;
  font-size: 13px;
  font-weight: 500;
  color: rgba(205, 214, 244, 0.9);
  letter-spacing: 3px;
  margin-bottom: 4px;
}
.en {
  display: block;
  font-size: 10px;
  color: rgba(205, 214, 244, 0.35);
  letter-spacing: 2px;
}
.bottom-hint {
  position: fixed;
  bottom: 20px;
  left: 50%;
  transform: translateX(-50%);
  font-size: 11px;
  font-family: Inter, 'Segoe UI', sans-serif;
  background: rgba(0, 0, 0, 0.25);
  padding: 4px 14px;
  border-radius: 12px;
  border: 1px solid rgba(255, 255, 255, 0.06);
  white-space: nowrap;
}
.bottom-hint.bright {
  color: rgba(205, 214, 244, 0.6);
}
.bottom-hint.dim {
  color: rgba(205, 214, 244, 0.2);
  border-color: rgba(255, 255, 255, 0.03);
}
.bottom-hint.green {
  color: rgba(166, 227, 161, 0.7);
}
kbd {
  background: rgba(255, 255, 255, 0.1);
  border: 1px solid rgba(255, 255, 255, 0.18);
  border-radius: 3px;
  padding: 1px 6px;
  font-family: 'JetBrains Mono', monospace;
  font-size: 10px;
  color: rgba(205, 214, 244, 0.6);
}
</style>
