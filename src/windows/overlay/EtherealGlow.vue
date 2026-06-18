<template>
  <!-- Holding-phase "Ethereal Glow": a large soft luminous ring (pure box-shadow)
       over the fully-blurred snapshot, with calm rest text and a countdown.
       Ported from the Stitch "Full Screen Ethereal Glow Overlay" design. -->
  <div class="glow-wrap">
    <div class="glow-ring" />
    <div class="glow-text">
      <span class="zh">望向 6 米外</span>
      <span class="en">LOOK 20 FT AWAY<span v-if="secsLeft > 0" class="sec"> · {{ secsLeft }}s</span></span>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue';

const props = withDefaults(defineProps<{ durationSecs?: number }>(), {
  durationSecs: 20,
});

const secsLeft = ref(props.durationSecs);
let timer: number | undefined;

onMounted(() => {
  secsLeft.value = props.durationSecs;
  timer = window.setInterval(() => {
    if (secsLeft.value > 0) secsLeft.value -= 1;
    else if (timer) window.clearInterval(timer);
  }, 1000);
});

onUnmounted(() => {
  if (timer) window.clearInterval(timer);
});
</script>

<style scoped>
.glow-wrap {
  position: fixed;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  pointer-events: none;
  display: flex;
  align-items: center;
  justify-content: center;
  animation: ethereal-pulse 4s ease-in-out infinite;
}

/* Exact Stitch glow seam (ethereal.html): 500px ring built from shadows. */
.glow-ring {
  width: 500px;
  height: 500px;
  border-radius: 50%;
  border: 1px solid rgba(137, 180, 250, 0.05);
  box-shadow:
    inset 0 0 40px 10px rgba(137, 180, 250, 0.15),
    0 0 60px 5px rgba(137, 180, 250, 0.1);
}

.glow-text {
  position: absolute;
  text-align: center;
}
.zh {
  display: block;
  font-size: 18px;
  font-weight: 500;
  color: rgba(227, 224, 247, 0.95);
  letter-spacing: 4px;
  margin-bottom: 8px;
  text-shadow: 0 2px 20px rgba(0, 0, 0, 0.85), 0 0 12px rgba(137, 180, 250, 0.25);
}
.en {
  display: block;
  font-size: 11px;
  color: rgba(195, 198, 210, 0.6);
  letter-spacing: 3px;
  font-family: 'JetBrains Mono', monospace;
  text-shadow: 0 1px 12px rgba(0, 0, 0, 0.8);
}
.sec {
  color: rgba(137, 180, 250, 0.8);
}

@keyframes ethereal-pulse {
  0%,
  100% {
    opacity: 0.92;
    transform: translate(-50%, -50%) scale(1);
  }
  50% {
    opacity: 1;
    transform: translate(-50%, -50%) scale(1.02);
  }
}
</style>
