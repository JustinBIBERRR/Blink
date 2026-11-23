<template>
  <div v-if="snapshot.round_id !== null" class="controls">
    <button v-if="snapshot.eraser.enabled" class="holder" :aria-label="snapshot.eraser.held ? '放回黑板擦' : '拿起黑板擦'" :title="error || (snapshot.eraser.held ? '点击空槽放回' : '拿起后按住左键擦拭')" @click="holderClick"><EraserArt :empty="snapshot.eraser.held" /></button>
    <button class="close" aria-label="结束所有屏幕上的提示" title="结束提示" @click="close">×</button>
    <span v-if="error" role="alert" class="error" :title="error">需辅助功能权限</span>
  </div>
</template>
<script setup lang="ts">
import { ref } from 'vue';
import { useReminder } from '../../composables/useReminder';
import { invoke } from '@tauri-apps/api/core';
import EraserArt from './EraserArt.vue';
const { snapshot } = useReminder();
const error=ref('');
let firstClickDocked = false;
let pending = Promise.resolve();
function holderClick(event: MouseEvent) {
  const roundId = snapshot.value.round_id;
  if (event.detail === 1) firstClickDocked = !snapshot.value.eraser.held;
  const double = event.detail === 2 && firstClickDocked;
  // Serialize native permission/pickup completion with the second click.
  pending = pending.then(async () => {
    try { await invoke('holder_click', { roundId, double }); error.value = ''; }
    catch (e) { error.value = String(e); }
  });
}
async function close() { if (snapshot.value.round_id !== null) await invoke('finish_reminder', { roundId: snapshot.value.round_id }); }
</script>
<style>
html, body, #app { background: transparent !important; margin: 0; }
.controls { display:flex;justify-content:flex-end;align-items:center;gap:8px;height:48px; }
button { cursor:pointer; }
.holder {width:148px;height:48px;padding:0;background:none;border:0;}
.holder svg {width:100%;height:100%;filter:drop-shadow(0 2px 2px #18252b66);}
.close { width:44px;height:44px;border-radius:50%;border:1px solid #e2eff2aa;background:#506572dd;color:white;font:28px system-ui;flex-shrink:0; }
.close:hover { background:#324650; }
.error {position:absolute;left:8px;bottom:0;font:10px system-ui;color:#fff;background:#384957;}
</style>
