<template>
  <div v-if="snapshot.round_id !== null" :key="snapshot.round_id" class="soft-reminder"
    :style="{ animationDelay: `-${snapshot.elapsed_ms}ms` }" aria-hidden="true" />
</template>

<script setup lang="ts">
import { onMounted, onUnmounted, ref } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';

type Snapshot = { generation: number; round_id: number | null; elapsed_ms: number; phase: string };
const snapshot = ref<Snapshot>({ generation: 0, round_id: null, elapsed_ms: 0, phase: 'monitoring' });
let unlisten: (() => void) | undefined;
let disposed = false;
function accept(next: Snapshot) {
  // A late initial query or event cannot bring back an older round.
  if (next.generation < snapshot.value.generation) return;
  snapshot.value = next;
}
onMounted(async () => {
  const stop = await listen<Snapshot>('blink://reminder', event => accept(event.payload));
  if (disposed) { stop(); return; }
  unlisten = stop;
  await invoke('overlay_ready');
  const current = await invoke<Snapshot>('get_reminder');
  if (!disposed) accept(current);
});
onUnmounted(() => { disposed = true; unlisten?.(); });
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
