<template><canvas ref="canvas" aria-hidden="true" /></template>
<script setup lang="ts">
import { onMounted, onUnmounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import type { Snapshot } from '../../composables/useReminder';
const props = defineProps<{ snapshot: Snapshot; receivedAt: number }>();
const canvas = ref<HTMLCanvasElement>();
const mask = document.createElement('canvas'); mask.width = 160; mask.height = 90;
const maskContext = mask.getContext('2d')!;
const pixels = maskContext.createImageData(mask.width, mask.height);
const layer = document.createElement('canvas');
const ctx = layer.getContext('2d')!;
const desktop = new Image();
let capturedRound: number | null = null;
let geometry = [0,0,1];
const erased = document.createElement('canvas');
const eraseContext = erased.getContext('2d')!;
let frame = 0, lastMask = 0, sending = false;
const smooth = (a: number, b: number, t: number) => { const v = Math.max(0, Math.min(1, (t-a)/(b-a))); return v*v*(3-2*v); };
function field(x: number, y: number) {
  return Math.max(0, Math.min(
    x*1.13 + .027*Math.sin(19*y) + .017*Math.cos(43*y),
    (1-x)*.99 + .04*Math.sin(17*y+2) + .014*Math.cos(45*y),
    y*.88 + .036*Math.sin(16*x) + .016*Math.cos(39*x+1),
    (1-y)*1.10 + .032*Math.cos(21*x+1) + .018*Math.sin(42*x)));
}
// Stable across frames and displays: droplets never move while fog grows.
let seed = 91;
const random = () => { seed = (seed * 1664525 + 1013904223) >>> 0; return seed / 4294967296; };
const beads = Array.from({ length: 68 }, () => {
  const side = Math.floor(random()*4), edge = .009 + random()*.12, pos = random();
  const x = side === 0 ? edge : side === 1 ? 1-edge : pos;
  const y = side === 2 ? edge : side === 3 ? 1-edge : pos;
  return { x, y, r: .8 + random()**3*3.8, delay: field(x,y) };
});
const crystals = Array.from({ length: 30 }, (_, i) => ({
  x: (random()**1.5*.15+.004)*(i%2===0?-1:1)+(i%2===0?1:0),
  y: (random()**2*.087+.004)*(i%4<2?-1:1)+(i%4<2?1:0),
  direction: (i%4<2?-1:1)*(.47+random()*2.1), length: 5+random()**1.5*27,
}));
watch(() => props.snapshot.round_id, async id => {
  desktop.src = ''; capturedRound = null;
  if (id === null || props.snapshot.mode !== 'fog') return;
  try {
    const url = await invoke<string | null>('fog_capture');
    if (props.snapshot.round_id === id && url) { desktop.src = url; capturedRound = id; }
  } catch (error) { console.error('Desktop blur capture failed', error); }
}, { immediate: true });
function render(now: number) {
  frame = requestAnimationFrame(render);
  const output = canvas.value; if (!output) return;
  const w = window.innerWidth, h = window.innerHeight;
  if (output.width !== w || output.height !== h) { output.width=w; output.height=h; layer.width=w; layer.height=h; }
  const out = output.getContext('2d')!; out.clearRect(0,0,w,h);
  const id = props.snapshot.round_id;
  if (id === null || props.snapshot.mode !== 'fog') return;
  const since = (props.snapshot.elapsed_ms + now - props.receivedAt)/1000 - 3;
  const strength = smooth(0,.65,since)*(1-smooth(17,20,since));
  const front = .012 + .57*smooth(0,5,since);
  if (erased.width !== w || erased.height !== h) { erased.width=w; erased.height=h; }
  eraseContext.clearRect(0,0,w,h);
  eraseContext.lineCap='round'; eraseContext.strokeStyle='white'; eraseContext.fillStyle='white';
  const [ox,oy,scale]=geometry;
  for (const [ax,ay,bx,by,r] of props.snapshot.eraser.strokes) {
    eraseContext.lineWidth=2*r/scale;
    eraseContext.beginPath(); eraseContext.moveTo((ax-ox)/scale,(ay-oy)/scale); eraseContext.lineTo((bx-ox)/scale,(by-oy)/scale); eraseContext.stroke();
    eraseContext.beginPath(); eraseContext.arc((bx-ox)/scale,(by-oy)/scale,r/scale,0,Math.PI*2); eraseContext.fill();
  }
  const alpha: number[] = [];
  for (let i=0; i<mask.width*mask.height; i++) {
    const threshold=field((i%mask.width)/mask.width,Math.floor(i/mask.width)/mask.height);
    const coverage=Math.round(255*smooth(threshold-.014,threshold+.065,front)*strength);
    alpha.push(coverage);
    pixels.data[i*4]=pixels.data[i*4+1]=pixels.data[i*4+2]=255; pixels.data[i*4+3]=coverage;
  }
  maskContext.putImageData(pixels,0,0);
  maskContext.globalCompositeOperation='destination-out';maskContext.drawImage(erased,0,0,mask.width,mask.height);maskContext.globalCompositeOperation='source-over';
  const masked=maskContext.getImageData(0,0,mask.width,mask.height).data;
  for(let i=0;i<alpha.length;i++) alpha[i]=masked[i*4+3];
  // The same complete alpha mask is used by native blur and WebView decorations.
  // Eraser integration subtracts persistent strokes here before both consumers.
  if (!sending && now-lastMask>=50) {
    sending=true; lastMask=now;
    invoke('fog_mask',{roundId:id,alpha,width:mask.width,height:mask.height}).catch(console.error).finally(()=>{sending=false;});
  }
  ctx.clearRect(0,0,w,h);
  if (capturedRound===id && desktop.complete && desktop.naturalWidth) {
    ctx.filter='blur(13px)'; ctx.drawImage(desktop,-16,-16,w+32,h+32); ctx.filter='none';
  }
  ctx.fillStyle='rgba(166,192,208,.47)';ctx.fillRect(0,0,w,h);
  ctx.globalCompositeOperation='destination-in';ctx.drawImage(mask,0,0,w,h);ctx.globalCompositeOperation='source-over';
  out.drawImage(layer,0,0);
  for (const crystal of crystals) {
    out.save();out.translate(crystal.x*w,crystal.y*h);out.rotate(crystal.direction);
    out.globalAlpha=.45*strength*smooth(.006,.15,front);out.strokeStyle='#d7ebf5';out.lineWidth=.7;
    out.beginPath();out.moveTo(0,0);out.lineTo(crystal.length,0);
    for(const f of [.30,.54,.74]) { const x=f*crystal.length,twig=(1-f)*crystal.length*.47;out.moveTo(x+twig,-twig);out.lineTo(x,0);out.lineTo(x+twig,twig); }
    out.stroke();out.restore();
  }
  for(const bead of beads) {
    const x=bead.x*w,y=bead.y*h,r=bead.r;
    out.globalAlpha=strength*smooth(bead.delay+.018,bead.delay+.10,front)*.78;
    const lens=out.createRadialGradient(x-r*.28,y-r*.35,0,x,y,r);
    lens.addColorStop(0,'rgba(238,250,255,.94)');lens.addColorStop(.37,'rgba(184,219,234,.52)');lens.addColorStop(.73,'rgba(78,105,124,.22)');lens.addColorStop(1,'rgba(31,55,72,.73)');
    out.fillStyle=lens;out.beginPath();out.ellipse(x,y,r*.86,r,0,0,Math.PI*2);out.fill();
  }
  out.globalAlpha=1;
  out.globalCompositeOperation='destination-out';out.drawImage(erased,0,0);out.globalCompositeOperation='source-over';
  if (props.snapshot.eraser.held && props.snapshot.eraser.cursor) {
    const [x,y]=props.snapshot.eraser.cursor;
    out.strokeStyle='rgba(235,248,255,.8)';out.lineWidth=1;
    out.beginPath();out.arc((x-ox)/scale,(y-oy)/scale,props.snapshot.eraser.radius/scale,0,Math.PI*2);out.stroke();
  }
}
onMounted(async()=>{geometry=await invoke<number[]>('overlay_geometry');frame=requestAnimationFrame(render);});
onUnmounted(()=>cancelAnimationFrame(frame));
</script>
<style scoped>canvas { position:fixed; inset:0; width:100%; height:100%; pointer-events:none; }</style>
