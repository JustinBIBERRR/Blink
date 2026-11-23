import { createIceSurface, createEraserMeltCache, eraserMeltDurationMs, iceTextureUrl, paintEraserMeltMask, paintFogGrowthMask, paintIceSurface, type IceSurface, type TimedEraserStroke } from '../src/windows/overlay/iceSurface';

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
const meltCache = createEraserMeltCache();
const texture = new Image();
let strokes: TimedEraserStroke[] = [];
let surface: IceSurface;
let playing = false, last = 0, frame = 0;
let previous: { x: number; y: number } | undefined;
let probe: { x: number; y: number } | undefined;

function thawPending(now: number) {
  return strokes.length > 0 && now - strokes[strokes.length - 1].startedAtMs < eraserMeltDurationMs;
}
function schedule() {
  if (!frame) frame = requestAnimationFrame(tick);
}
function draw(now = performance.now()) {
  paintFogGrowthMask(mask, Number(timeline.value));
  paintEraserMeltMask(erased, strokes, now, { width: output.width, height: output.height }, meltCache);
  paintIceSurface(output, { surface, growthMask: mask, erased });
  elapsed.value = `${Number(timeline.value).toFixed(2)} s`;
  const ctx = output.getContext('2d')!;
  const center = ctx.getImageData(Math.floor(output.width / 2), Math.floor(output.height / 2), 1, 1).data[3];
  const cleared = probe ? ctx.getImageData(Math.floor(probe.x), Math.floor(probe.y), 1, 1).data[3] : 'not sampled';
  metrics.textContent = `Texture ${texture.naturalWidth ? 'loaded' : 'loading'} · Center alpha ${center}/255 · Last erased core alpha ${cleared}${probe ? '/255' : ''} · ${output.width} × ${output.height}`;
  if (playing || thawPending(now)) schedule();
}
document.querySelector<HTMLSelectElement>('#background')!.onchange = event => {
  const value = (event.target as HTMLSelectElement).value;
  document.querySelector<HTMLDivElement>('#desktop')!.style.display = value === 'desktop' ? 'grid' : 'none';
  stage.style.background = value === 'dark' ? '#10151c' : value === 'light' ? '#fff' : 'linear-gradient(130deg,#d37544,#354f77 50%,#133d38)';
};
function resize() {
  output.width = stage.clientWidth;
  output.height = stage.clientHeight;
  surface = createIceSurface(output.width, output.height, texture);
  draw();
}
function tick(now: number) {
  frame = 0;
  if (playing) timeline.value = String(Math.min(20, Number(timeline.value) + (now - last) / 1000));
  last = now;
  if (Number(timeline.value) >= 20) { playing = false; play.textContent = 'Play'; }
  draw(now);
}
function pause() { playing = false; play.textContent = 'Play'; }
function resetStrokes() { strokes = []; previous = undefined; probe = undefined; }
play.onclick = () => {
  if (playing) { pause(); return; }
  if (Number(timeline.value) >= 20) { timeline.value = '0'; resetStrokes(); }
  playing = true; play.textContent = 'Pause'; last = performance.now(); schedule();
};
timeline.oninput = () => { pause(); draw(); };
document.querySelector<HTMLButtonElement>('#clear')!.onclick = () => { resetStrokes(); draw(); };
document.querySelector<HTMLButtonElement>('#restart')!.onclick = () => {
  pause(); timeline.value = '0'; resetStrokes(); draw(); play.click();
};
function eraseAt(event: PointerEvent) {
  const bounds = output.getBoundingClientRect();
  const point = { x: (event.clientX - bounds.left) * output.width / bounds.width, y: (event.clientY - bounds.top) * output.height / bounds.height };
  strokes.push({ segment: [previous?.x ?? point.x, previous?.y ?? point.y, point.x, point.y, Number(radius.value)], startedAtMs: performance.now() });
  previous = point; probe = point; draw();
}
output.onpointerdown = event => {
  if (event.button !== 0) return;
  output.setPointerCapture(event.pointerId); previous = undefined; eraseAt(event);
};
output.onpointermove = event => { if (previous) eraseAt(event); };
output.onpointerup = output.onpointercancel = output.onlostpointercapture = () => { previous = undefined; };
texture.onload = () => { surface = createIceSurface(output.width, output.height, texture); draw(); };
texture.src = iceTextureUrl;
new ResizeObserver(resize).observe(stage);
resize();

document.querySelector<HTMLButtonElement>('#check-melt')!.onclick = () => {
  const results: string[] = [];
  const check = (name: string, passed: boolean) => results.push(`${passed ? 'PASS' : 'FAIL'} ${name}`);
  const testOutput = document.createElement('canvas');
  testOutput.width = 160; testOutput.height = 90;
  const testSurface = createIceSurface(160, 90);
  const material = testSurface.canvas.getContext('2d')!;
  material.fillStyle = 'white'; material.fillRect(0, 0, 160, 90);
  const growth = document.createElement('canvas'); growth.width = 160; growth.height = 90;
  const eraseMask = document.createElement('canvas');
  const cache = createEraserMeltCache();
  const viewport = { width: 160, height: 90 };
  const sample = (x: number, y: number) => testOutput.getContext('2d')!.getImageData(x, y, 1, 1).data[3];
  const render = (time: number, marks: readonly TimedEraserStroke[], now: number) => {
    paintFogGrowthMask(growth, time);
    paintEraserMeltMask(eraseMask, marks, now, viewport, cache);
    paintIceSurface(testOutput, { surface: testSurface, growthMask: growth, erased: eraseMask });
  };
  render(18, [], 0);
  check('18s center clears before rim', sample(80, 45) < sample(2, 2) && sample(80, 45) === 0);
  let previousPixels: Uint8ClampedArray | undefined;
  let monotonic = true;
  for (let step = 0; step <= 60; step++) {
    render(17 + step / 20, [], 0);
    const pixels = testOutput.getContext('2d')!.getImageData(0, 0, 160, 90).data;
    if (previousPixels) for (let i = 3; i < pixels.length; i += 4) if (pixels[i] > previousPixels[i]) monotonic = false;
    previousPixels = pixels;
  }
  check('17–20s every pixel melts monotonically', monotonic);
  check('20s every pixel transparent', previousPixels!.every((value, index) => index % 4 !== 3 || value === 0));
  const mark: TimedEraserStroke = { segment: [60, 45, 100, 45, 15], startedAtMs: 1000 };
  const marks = [mark];
  render(8, marks, 1000); const fresh = sample(80, 45);
  check('contact starts partial thinning', fresh > 0 && fresh < 255);
  render(8, marks, 1300); const partial = sample(80, 45);
  check('contact keeps melting', partial > 0 && partial < fresh);
  render(8, Array.from({ length: 40 }, () => mark), 1300);
  check('overlapping new strokes stay partial', sample(80, 45) === partial);
  render(8, marks, 1650); check('650ms core fully transparent', sample(80, 45) === 0);
  render(8, marks, 8000); check('mature stroke persists', sample(80, 45) === 0);
  marks.push({ segment: [20, 20, 20, 20, 10], startedAtMs: 8000 });
  render(8, marks, 8000);
  check('cached mature mark and appended fresh mark coexist', sample(80, 45) === 0 && sample(20, 20) > 0 && sample(20, 20) < 255);
  render(8, marks, 8650);
  check('appended mark settles into cache', sample(20, 20) === 0 && sample(80, 45) === 0);
  render(8, [], 9000); check('reset removes erasure', sample(80, 45) === 255 && sample(20, 20) === 255);
  const projectedMarks: TimedEraserStroke[] = [{ segment: [-80, -55, -80, -55, 20], startedAtMs: 0 }];
  paintEraserMeltMask(eraseMask, projectedMarks, 1000,
    { width: 160, height: 90, originX: -240, originY: -145, scale: 2 }, cache);
  check('negative origin and scale project correctly', eraseMask.getContext('2d')!.getImageData(80, 45, 1, 1).data[3] === 255);
  paintEraserMeltMask(eraseMask, projectedMarks, 1000,
    { width: 160, height: 90, originX: -200, originY: -145, scale: 2 }, cache);
  check('geometry change rebuilds settled projection', eraseMask.getContext('2d')!.getImageData(60, 45, 1, 1).data[3] === 255 && eraseMask.getContext('2d')!.getImageData(80, 45, 1, 1).data[3] === 0);
  const report = document.querySelector<HTMLOutputElement>('#check-results')!;
  report.textContent = results.join(' · ');
  report.dataset.result = results.some(result => result.startsWith('FAIL')) ? 'fail' : 'pass';
};
