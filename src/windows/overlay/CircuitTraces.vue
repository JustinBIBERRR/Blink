<template>
  <div class="circuit-root" :class="{ active: visible }">
    <svg class="circuit-svg" xmlns="http://www.w3.org/2000/svg">
      <!-- Top edge traces -->
      <path class="trace" d="M 0,4 L 420,4 L 420,14 L 860,14" />
      <path class="trace delay-1" d="M 1920,4 L 1300,4 L 1300,10 L 880,10" />
      <!-- Bottom edge traces -->
      <path class="trace delay-2" d="M 0,1076 L 580,1076 L 580,1066 L 1020,1066" />
      <path class="trace delay-3" d="M 1920,1076 L 1380,1076 L 1380,1068 L 940,1068" />
      <!-- Left edge traces -->
      <path class="trace delay-1" d="M 4,0 L 4,260 L 14,260 L 14,480" />
      <path class="trace delay-3" d="M 4,1080 L 4,680 L 10,680 L 10,500" />
      <!-- Right edge traces -->
      <path class="trace delay-2" d="M 1916,0 L 1916,300 L 1906,300 L 1906,520" />
      <path class="trace" d="M 1916,1080 L 1916,720 L 1910,720 L 1910,540" />
      <!-- Corner nodes -->
      <circle class="node" cx="4" cy="4" r="3" />
      <circle class="node delay-1" cx="1916" cy="4" r="3" />
      <circle class="node delay-2" cx="4" cy="1076" r="3" />
      <circle class="node delay-3" cx="1916" cy="1076" r="3" />
      <!-- Junction nodes on traces -->
      <circle class="node delay-1" cx="420" cy="4" r="2.5" />
      <circle class="node delay-2" cx="1300" cy="4" r="2.5" />
      <circle class="node" cx="580" cy="1076" r="2.5" />
      <circle class="node delay-3" cx="1380" cy="1076" r="2.5" />
    </svg>
  </div>
</template>

<script setup lang="ts">
defineProps<{ visible: boolean }>();
</script>

<style scoped>
.circuit-root {
  position: fixed;
  inset: 0;
  pointer-events: none;
  opacity: 0;
  transition: opacity 0.4s ease;
}
.circuit-root.active {
  opacity: 1;
}
.circuit-svg {
  width: 100vw;
  height: 100vh;
  position: absolute;
  inset: 0;
  overflow: visible;
}
.trace {
  fill: none;
  stroke: #89b4fa;
  stroke-width: 1.5;
  stroke-linecap: round;
  filter: drop-shadow(0 0 4px #89b4fa) drop-shadow(0 0 10px rgba(137, 180, 250, 0.5));
  stroke-dasharray: 300;
  animation: trace-flow 1.4s ease-in-out infinite alternate;
}
.trace.delay-1 {
  animation-delay: 0.35s;
  stroke: #cba6f7;
  filter: drop-shadow(0 0 4px #cba6f7) drop-shadow(0 0 10px rgba(203, 166, 247, 0.5));
}
.trace.delay-2 {
  animation-delay: 0.7s;
}
.trace.delay-3 {
  animation-delay: 1.05s;
  stroke: #cba6f7;
  filter: drop-shadow(0 0 4px #cba6f7);
}
.node {
  fill: #ffffff;
  filter: drop-shadow(0 0 5px #89b4fa) drop-shadow(0 0 12px rgba(137, 180, 250, 0.7));
  animation: node-pulse 1.4s ease-in-out infinite alternate;
}
.node.delay-1 {
  animation-delay: 0.35s;
  fill: #cba6f7;
  filter: drop-shadow(0 0 5px #cba6f7);
}
.node.delay-2 {
  animation-delay: 0.7s;
}
.node.delay-3 {
  animation-delay: 1.05s;
  fill: #cba6f7;
}
@keyframes trace-flow {
  0%   { stroke-dashoffset: 300; opacity: 0.3; }
  50%  { stroke-dashoffset: 0;   opacity: 1; }
  100% { stroke-dashoffset: -300; opacity: 0.3; }
}
@keyframes node-pulse {
  0%   { opacity: 0.2; r: 2; }
  50%  { opacity: 1;   r: 4; }
  100% { opacity: 0.2; r: 2; }
}
</style>
