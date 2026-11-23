<template>
  <EraserArt v-if="snapshot.eraser.held && snapshot.eraser.cursor" class="held-eraser" :empty="false" :lifted="true" :style="{left: `${(snapshot.eraser.cursor[0]-geometry[0])/geometry[2]}px`,top: `${(snapshot.eraser.cursor[1]-geometry[1])/geometry[2]}px`}" />
  <FogLayer :snapshot="snapshot" :received-at="receivedAt" />
  <div v-if="snapshot.round_id !== null" :key="snapshot.round_id" class="soft-reminder"
    :style="{ animationDelay: `-${snapshot.elapsed_ms}ms` }" aria-hidden="true" />
</template>

<script setup lang="ts">
import { useReminder } from '../../composables/useReminder';
import FogLayer from './FogLayer.vue';
import EraserArt from './EraserArt.vue';
import { ref, onMounted, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
const geometry=ref([0,0,1]);
onMounted(async()=>{geometry.value=await invoke<number[]>('overlay_geometry');});
const { snapshot, receivedAt } = useReminder();
watch(()=>snapshot.value.round_id, async id=>{if(id!==null) geometry.value=await invoke<number[]>('overlay_geometry');});
</script>

<style>
html, body, #app { background: transparent !important; }
</style>
<style scoped>
.held-eraser { position:fixed;width:104px;height:48px;transform:translate(-50%,-50%) rotate(-18deg);z-index:5;pointer-events:none;filter:drop-shadow(0 5px 3px #17263177); }
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
