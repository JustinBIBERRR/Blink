<template>
  <div class="overlay-root">
    <CircuitTraces :visible="phase === 'warning'" />
    <BlurMask :phase="phase" />
  </div>
</template>

<script setup lang="ts">
import CircuitTraces from './CircuitTraces.vue';
import BlurMask from './BlurMask.vue';
import { usePhase } from '../../composables/useBlink';

const { phase } = usePhase();
</script>

<style scoped>
.overlay-root {
  position: fixed;
  inset: 0;
  background: transparent;
  pointer-events: none;
}
</style>
