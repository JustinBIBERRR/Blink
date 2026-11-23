<template><button v-if="snapshot.round_id !== null" aria-label="结束所有屏幕上的提示" title="结束提示" @click="close">×</button></template>
<script setup lang="ts">
import { useReminder } from '../../composables/useReminder';
import { invoke } from '@tauri-apps/api/core';
const { snapshot } = useReminder();
async function close() { if (snapshot.value.round_id !== null) await invoke('finish_reminder', { roundId: snapshot.value.round_id }); }
</script>
<style>
html, body, #app { background: transparent !important; margin: 0; }
button { width: 44px; height: 44px; border-radius: 50%; border: 1px solid #e2eff2aa; background: #506572dd; color: white; font: 28px system-ui; cursor: pointer; }
button:hover { background: #324650; }
</style>
