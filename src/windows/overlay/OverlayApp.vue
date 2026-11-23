<template>
  <FogLayer :snapshot="snapshot" :received-at="receivedAt" />
  <div v-if="snapshot.round_id !== null" :key="snapshot.round_id" class="soft-reminder"
    :style="{ animationDelay: `-${snapshot.elapsed_ms}ms` }" aria-hidden="true" />
</template>

<script setup lang="ts">
import { useReminder } from '../../composables/useReminder';
import FogLayer from './FogLayer.vue';
const { snapshot, receivedAt } = useReminder();
</script>

<style>
html, body, #app { background: transparent !important; }
</style>
<style scoped>
.soft-reminder {
  position: fixed;
  inset: 0;
  pointer-events: none;
  border-radius: 5px;
  /* Only the outermost 24 pixels glow; no capture, blur or central scrim. */
  box-shadow: inset 0 0 7px 2px rgb(151 219 255 / 75%),
              inset 0 0 24px 1px rgb(104 190 244 / 27%);
  opacity: 0;
  animation: soft-pulse 3000ms ease-in-out both;
}
@keyframes soft-pulse {
  0%, 100% { opacity: 0; }
  38% { opacity: 0.85; }
  58% { opacity: 0.7; }
}
</style>
