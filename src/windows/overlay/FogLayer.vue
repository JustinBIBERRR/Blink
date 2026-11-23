<template><canvas ref="canvas" aria-hidden="true" /></template>
<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Snapshot } from '../../composables/useReminder';
import { createIceSurface, iceTextureUrl, paintFogGrowthMask, paintIceSurface, type IceSurface } from './iceSurface';

const props = defineProps<{ snapshot: Snapshot; receivedAt: number }>();
const canvas = ref<HTMLCanvasElement>();
const mask = document.createElement('canvas');
mask.width = 160; mask.height = 90;
const nativeMask = document.createElement('canvas');
nativeMask.width = mask.width; nativeMask.height = mask.height;
const nativeContext = nativeMask.getContext('2d')!;
const erased = document.createElement('canvas');
const eraseContext = erased.getContext('2d')!;
const texture = new Image();
let surface: IceSurface | undefined;
let desktop: HTMLImageElement | undefined;
let geometry = [0, 0, 1];
let frame = 0, lastMask = 0, sending = false;
let mounted = false, requestVersion = 0;
const active = () => props.snapshot.round_id !== null && props.snapshot.mode === 'fog';

texture.onload = () => { surface = undefined; };
texture.src = iceTextureUrl;

watch([() => props.snapshot.round_id, () => props.snapshot.mode], async ([id, mode]) => {
  const version = ++requestVersion;
  desktop = undefined;
  if (mounted) start();
  if (id === null || mode !== 'fog') return;
  try {
    const nextGeometry = await invoke<number[]>('overlay_geometry');
    if (version !== requestVersion) return;
    geometry = nextGeometry;
    const url = await invoke<string | null>('fog_capture');
    if (version !== requestVersion || !url) return;
    const captured = new Image();
    captured.onload = () => {
      if (version === requestVersion && active()) desktop = captured;
    };
    captured.src = url;
  } catch (error) { console.error('Desktop blur capture failed', error); }
}, { immediate: true });

function start() {
  if (frame) cancelAnimationFrame(frame);
  frame = requestAnimationFrame(render);
}

function render(now: number) {
  frame = 0;
  const output = canvas.value;
  if (!output) return;
  const w = window.innerWidth, h = window.innerHeight;
  if (output.width !== w || output.height !== h) {
    output.width = w; output.height = h;
    erased.width = w; erased.height = h;
    surface = undefined;
  }
  const out = output.getContext('2d')!;
  const id = props.snapshot.round_id;
  if (id === null || !active()) { out.clearRect(0, 0, w, h); return; }
  frame = requestAnimationFrame(render);
  surface ??= createIceSurface(w, h, texture);
  const since = (props.snapshot.elapsed_ms + now - props.receivedAt) / 1000 - 3;
  paintFogGrowthMask(mask, since);
  eraseContext.clearRect(0, 0, w, h);
  eraseContext.lineCap = 'round';
  eraseContext.strokeStyle = 'white'; eraseContext.fillStyle = 'white';
  const [ox, oy, scale] = geometry;
  for (const [ax, ay, bx, by, r] of props.snapshot.eraser.strokes) {
    eraseContext.lineWidth = 2 * r / scale;
    eraseContext.beginPath();
    eraseContext.moveTo((ax - ox) / scale, (ay - oy) / scale);
    eraseContext.lineTo((bx - ox) / scale, (by - oy) / scale);
    eraseContext.stroke();
    eraseContext.beginPath();
    eraseContext.arc((bx - ox) / scale, (by - oy) / scale, r / scale, 0, Math.PI * 2);
    eraseContext.fill();
  }
  paintIceSurface(output, { surface, growthMask: mask, erased, desktop });
  if (!sending && now - lastMask >= 50) {
    nativeContext.clearRect(0, 0, mask.width, mask.height);
    nativeContext.drawImage(mask, 0, 0);
    nativeContext.globalCompositeOperation = 'destination-out';
    nativeContext.drawImage(erased, 0, 0, mask.width, mask.height);
    nativeContext.globalCompositeOperation = 'source-over';
    const pixels = nativeContext.getImageData(0, 0, mask.width, mask.height).data;
    const alpha = Array.from({ length: mask.width * mask.height }, (_, i) => pixels[i * 4 + 3]);
    sending = true; lastMask = now;
    invoke('fog_mask', { roundId: id, alpha, width: mask.width, height: mask.height })
      .catch(console.error).finally(() => { sending = false; });
  }
  if (props.snapshot.eraser.held && props.snapshot.eraser.cursor) {
    const [x, y] = props.snapshot.eraser.cursor;
    out.strokeStyle = 'rgba(235,248,255,.8)'; out.lineWidth = 1;
    out.beginPath();
    out.arc((x - ox) / scale, (y - oy) / scale, props.snapshot.eraser.radius / scale, 0, Math.PI * 2);
    out.stroke();
  }
}
onMounted(() => { mounted = true; start(); });
onUnmounted(() => {
  mounted = false;
  requestVersion++;
  texture.onload = null;
  cancelAnimationFrame(frame);
});
</script>
<style scoped>canvas { position:fixed; inset:0; width:100%; height:100%; pointer-events:none; }</style>
