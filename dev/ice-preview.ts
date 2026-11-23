import { createIceSurface, iceTextureUrl, paintFogGrowthMask, paintIceSurface, type IceSurface } from '../src/windows/overlay/iceSurface';

const output = document.querySelector<HTMLCanvasElement>('#ice')!;
const stage = document.querySelector<HTMLDivElement>('#stage')!;
const timeline = document.querySelector<HTMLInputElement>('#time')!;
const radius = document.querySelector<HTMLInputElement>('#radius')!;
const elapsed = document.querySelector<HTMLOutputElement>('#elapsed')!;
const play = document.querySelector<HTMLButtonElement>('#play')!;
const metrics = document.querySelector<HTMLDivElement>('#metrics')!;
const erased = document.createElement('canvas');
const mask = document.createElement('canvas');
mask.width = 160; mask.height = 90;
const erase = erased.getContext('2d')!;
const texture = new Image();
let surface: IceSurface;
let playing = false, last = 0, frame = 0;
let previous: { x: number; y: number } | undefined;
let probe: { x: number; y: number } | undefined;

function draw() {
  paintFogGrowthMask(mask, Number(timeline.value));
  paintIceSurface(output, { surface, growthMask: mask, erased });
  elapsed.value = `${Number(timeline.value).toFixed(2)} s`;
  const ctx = output.getContext('2d')!;
  const center = ctx.getImageData(Math.floor(output.width / 2), Math.floor(output.height / 2), 1, 1).data[3];
  const cleared = probe ? ctx.getImageData(Math.floor(probe.x), Math.floor(probe.y), 1, 1).data[3] : 'not sampled';
  metrics.textContent = `Texture ${texture.naturalWidth ? 'loaded' : 'loading'} · Center alpha ${center}/255 · Last erased core alpha ${cleared}${probe ? '/255' : ''} · ${output.width} × ${output.height}`;
}
document.querySelector<HTMLSelectElement>('#background')!.onchange = event => {
  const value = (event.target as HTMLSelectElement).value;
  document.querySelector<HTMLDivElement>('#desktop')!.style.display = value === 'desktop' ? 'grid' : 'none';
  stage.style.background = value === 'dark' ? '#10151c' : value === 'light' ? '#fff' : 'linear-gradient(130deg,#d37544,#354f77 50%,#133d38)';
};
function resize() {
  output.width = erased.width = stage.clientWidth;
  output.height = erased.height = stage.clientHeight;
  probe = undefined;
  surface = createIceSurface(output.width, output.height, texture);
  draw();
}
function tick(now: number) {
  timeline.value = String(Math.min(20, Number(timeline.value) + (now - last) / 1000));
  last = now;
  draw();
  if (Number(timeline.value) >= 20) { playing = false; play.textContent = 'Play'; }
  if (playing) frame = requestAnimationFrame(tick);
}
function pause() { playing = false; cancelAnimationFrame(frame); play.textContent = 'Play'; }
play.onclick = () => {
  if (playing) { pause(); return; }
  if (Number(timeline.value) >= 20) timeline.value = '0';
  playing = true; play.textContent = 'Pause'; last = performance.now(); frame = requestAnimationFrame(tick);
};
timeline.oninput = () => { pause(); draw(); };
document.querySelector<HTMLButtonElement>('#clear')!.onclick = () => {
  erase.clearRect(0, 0, erased.width, erased.height); probe = undefined; draw();
};
document.querySelector<HTMLButtonElement>('#restart')!.onclick = () => {
  pause(); timeline.value = '0'; erase.clearRect(0, 0, erased.width, erased.height); probe = undefined; draw(); play.click();
};
function eraseAt(event: PointerEvent) {
  const bounds = output.getBoundingClientRect();
  const point = { x: event.clientX - bounds.left, y: event.clientY - bounds.top };
  erase.lineCap = 'round'; erase.strokeStyle = 'white'; erase.fillStyle = 'white';
  erase.lineWidth = 2 * Number(radius.value);
  erase.beginPath(); erase.moveTo(previous?.x ?? point.x, previous?.y ?? point.y);
  erase.lineTo(point.x, point.y); erase.stroke();
  erase.beginPath(); erase.arc(point.x, point.y, Number(radius.value), 0, Math.PI * 2); erase.fill();
  previous = point; probe = point; draw();
}
output.onpointerdown = event => { output.setPointerCapture(event.pointerId); eraseAt(event); };
output.onpointermove = event => { if (previous) eraseAt(event); };
output.onpointerup = output.onpointercancel = () => { previous = undefined; };
texture.onload = () => { surface = createIceSurface(output.width, output.height, texture); draw(); };
texture.src = iceTextureUrl;
new ResizeObserver(resize).observe(stage);
resize();
