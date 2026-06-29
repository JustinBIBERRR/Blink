<template>
  <div class="circuit-root" :class="{ active: isActive }">
    <svg class="circuit-svg" xmlns="http://www.w3.org/2000/svg">
      <path
        v-for="(t, i) in traces"
        :key="'t' + i"
        class="trace"
        :class="t.cls"
        :d="t.d"
        vector-effect="non-scaling-stroke"
      />
      <circle
        v-for="(n, i) in nodes"
        :key="'n' + i"
        class="node"
        :class="n.cls"
        :cx="n.x"
        :cy="n.y"
        :r="n.r"
      />
    </svg>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue';
import type { Phase } from '../../types/blink';

const props = defineProps<{ phase: Phase }>();

const W = ref(window.innerWidth);
const H = ref(window.innerHeight);

function onResize() {
  W.value = window.innerWidth;
  H.value = window.innerHeight;
}
onMounted(() => window.addEventListener('resize', onResize));
onUnmounted(() => window.removeEventListener('resize', onResize));

const isActive = computed(() => props.phase === 'warning' || props.phase === 'spreading');

const cls = (i: number) => ['', 'delay-1', 'delay-2', 'delay-3'][i % 4];

// Dense circuit: many short glowing traces hugging all four edges with zig-zag
// steps, plus glowing nodes at the corners and trace junctions. Generated from
// the viewport size so it scales to any monitor.
const traces = computed(() => {
  const w = W.value;
  const h = H.value;
  const out: { d: string; cls: string }[] = [];
  let k = 0;

  // Two inset rows per edge => roughly double the "current" elements.
  const insets = [4, 30];
  for (const e of insets) {
    // Top + bottom edges: horizontal runs that step inward.
    const xs = [0.0, 0.14, 0.28, 0.42, 0.56, 0.7, 0.84, 1.0];
    for (let i = 0; i < xs.length - 1; i++) {
      const x0 = w * xs[i];
      const x1 = w * xs[i + 1];
      const s = 9 + (i % 3) * 5; // step depth
      out.push({ d: `M ${x0},${e} L ${x1 - 24},${e} L ${x1 - 24},${e + s} L ${x1},${e + s}`, cls: cls(k++) });
      out.push({ d: `M ${x0},${h - e} L ${x1 - 24},${h - e} L ${x1 - 24},${h - e - s} L ${x1},${h - e - s}`, cls: cls(k++) });
    }

    // Left + right edges: vertical runs that step inward.
    const ys = [0.06, 0.24, 0.42, 0.6, 0.78, 0.95];
    for (let i = 0; i < ys.length - 1; i++) {
      const y0 = h * ys[i];
      const y1 = h * ys[i + 1];
      const s = 9 + (i % 3) * 5;
      out.push({ d: `M ${e},${y0} L ${e},${y1 - 24} L ${e + s},${y1 - 24} L ${e + s},${y1}`, cls: cls(k++) });
      out.push({ d: `M ${w - e},${y0} L ${w - e},${y1 - 24} L ${w - e - s},${y1 - 24} L ${w - e - s},${y1}`, cls: cls(k++) });
    }
  }

  // Short corner brackets for extra density at the corners.
  const c = 70;
  out.push({ d: `M 4,${c} L 4,4 L ${c},4`, cls: cls(k++) });
  out.push({ d: `M ${w - 4},${c} L ${w - 4},4 L ${w - c},4`, cls: cls(k++) });
  out.push({ d: `M 4,${h - c} L 4,${h - 4} L ${c},${h - 4}`, cls: cls(k++) });
  out.push({ d: `M ${w - 4},${h - c} L ${w - 4},${h - 4} L ${w - c},${h - 4}`, cls: cls(k++) });

  return out;
});

const nodes = computed(() => {
  const w = W.value;
  const h = H.value;
  const out: { x: number; y: number; r: number; cls: string }[] = [];
  let k = 0;
  // Corners.
  for (const [x, y] of [[4, 4], [w - 4, 4], [4, h - 4], [w - 4, h - 4]] as const) {
    out.push({ x, y, r: 3, cls: cls(k++) });
  }
  // Junctions along top/bottom.
  for (const fx of [0.13, 0.27, 0.41, 0.55, 0.69, 0.83]) {
    out.push({ x: w * fx, y: 4, r: 2.5, cls: cls(k++) });
    out.push({ x: w * fx, y: h - 4, r: 2.5, cls: cls(k++) });
  }
  // Junctions along left/right.
  for (const fy of [0.22, 0.39, 0.56, 0.73]) {
    out.push({ x: 4, y: h * fy, r: 2.5, cls: cls(k++) });
    out.push({ x: w - 4, y: h * fy, r: 2.5, cls: cls(k++) });
  }
  return out;
});
</script>

<style scoped>
.circuit-root {
  position: fixed;
  inset: 0;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.5s ease;
}
.circuit-root.active {
  /* Brighter than before so the warning is clearly perceived, still translucent
     enough that the screen behind the edge traces stays visible. */
  opacity: 0.85;
}
.circuit-svg {
  position: fixed;
  inset: 0;
  width: 100%;
  height: 100%;
  overflow: visible;
}

/* Each trace is a glowing segment travelling along the path. */
.trace {
  fill: none;
  stroke: #89b4fa;
  stroke-width: 1.5;
  stroke-linecap: round;
  filter: drop-shadow(0 0 5px #89b4fa);
  stroke-dasharray: 180 6000;
  stroke-dashoffset: 180;
  /* Flow slowed 2x (was 2.2s) for a calmer, denser current. */
  animation: trace-flow 4.4s ease-in-out infinite;
}
.trace.delay-1 {
  animation-delay: 1.1s;
  stroke: #cba6f7;
  filter: drop-shadow(0 0 5px #cba6f7);
}
.trace.delay-2 { animation-delay: 2.2s; }
.trace.delay-3 {
  animation-delay: 3.3s;
  stroke: #cba6f7;
  filter: drop-shadow(0 0 5px #cba6f7);
}

.node {
  fill: #ffffff;
  filter: drop-shadow(0 0 5px #89b4fa) drop-shadow(0 0 12px rgba(137, 180, 250, 0.7));
  animation: node-pulse 2.8s ease-in-out infinite alternate;
}
.node.delay-1 {
  animation-delay: 0.7s;
  fill: #cba6f7;
  filter: drop-shadow(0 0 5px #cba6f7);
}
.node.delay-2 { animation-delay: 1.4s; }
.node.delay-3 {
  animation-delay: 2.1s;
  fill: #cba6f7;
}

@keyframes trace-flow {
  0%   { stroke-dashoffset:  180; opacity: 0.15; }
  30%  { opacity: 1; }
  70%  { opacity: 1; }
  100% { stroke-dashoffset: -6180; opacity: 0.15; }
}
@keyframes node-pulse {
  from { opacity: 0.2; }
  to   { opacity: 1; }
}
</style>
