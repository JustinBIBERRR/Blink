export interface IceSurface {
  readonly canvas: HTMLCanvasElement;
  readonly width: number;
  readonly height: number;
}

export interface TimedEraserStroke {
  readonly segment: readonly [number, number, number, number, number];
  readonly startedAtMs: number;
}

export const eraserMeltDurationMs = 650;

export interface EraserMeltCache {
  projection: string;
  strokes: readonly TimedEraserStroke[];
  settledCount: number;
  pixels: Uint8ClampedArray;
}

export function createEraserMeltCache(): EraserMeltCache {
  return { projection: '', strokes: [], settledCount: 0, pixels: new Uint8ClampedArray() };
}

export function paintEraserMeltMask(
  mask: HTMLCanvasElement,
  strokes: readonly TimedEraserStroke[],
  nowMs: number,
  viewport: { width: number; height: number; originX?: number; originY?: number; scale?: number },
  cache?: EraserMeltCache,
): void {
  const { width, height, originX = 0, originY = 0, scale = 1 } = viewport;
  const reduction = Math.min(1, 960 / Math.max(width, height));
  const w = Math.max(1, Math.round(width * reduction));
  const h = Math.max(1, Math.round(height * reduction));
  if (mask.width !== w || mask.height !== h) { mask.width = w; mask.height = h; }
  const ctx = mask.getContext('2d')!;
  const pixels = ctx.createImageData(w, h);
  const projection = `${width},${height},${originX},${originY},${scale}`;
  if (cache && (cache.projection !== projection || cache.strokes !== strokes || cache.settledCount > strokes.length || (cache.settledCount > 0 && nowMs - strokes[cache.settledCount - 1].startedAtMs < eraserMeltDurationMs))) {
    cache.projection = projection;
    cache.strokes = strokes;
    cache.settledCount = 0;
    cache.pixels = new Uint8ClampedArray(pixels.data.length);
  }
  let settledCount = cache?.settledCount ?? 0;
  while (settledCount < strokes.length && nowMs - strokes[settledCount].startedAtMs >= eraserMeltDurationMs) settledCount++;
  if (cache) pixels.data.set(cache.pixels);
  for (let strokeIndex = cache?.settledCount ?? 0; strokeIndex < strokes.length; strokeIndex++) {
    const { segment, startedAtMs } = strokes[strokeIndex];
    if (!segment.every(Number.isFinite) || !Number.isFinite(startedAtMs) || !(scale > 0)) continue;
    const [ax, ay, bx, by, radius] = segment;
    const r = radius / scale;
    if (!(r > 0)) continue;
    const x1 = (ax - originX) / scale, y1 = (ay - originY) / scale;
    const x2 = (bx - originX) / scale, y2 = (by - originY) / scale;
    const left = Math.max(0, Math.floor((Math.min(x1, x2) - r) * w / width));
    const right = Math.min(w - 1, Math.ceil((Math.max(x1, x2) + r) * w / width));
    const top = Math.max(0, Math.floor((Math.min(y1, y2) - r) * h / height));
    const bottom = Math.min(h - 1, Math.ceil((Math.max(y1, y2) + r) * h / height));
    const strength = .08 + .92 * smooth(0, eraserMeltDurationMs, nowMs - startedAtMs);
    const maxAlpha = Math.round(255 * strength);
    const dx = x2 - x1, dy = y2 - y1, lengthSquared = dx * dx + dy * dy;
    for (let y = top; y <= bottom; y++) {
      for (let x = left; x <= right; x++) {
        const i = (y * w + x) * 4;
        if (pixels.data[i + 3] >= maxAlpha) continue;
        const px = (x + .5) * width / w, py = (y + .5) * height / h;
        const t = lengthSquared ? Math.max(0, Math.min(1, ((px - x1) * dx + (py - y1) * dy) / lengthSquared)) : 0;
        const distance = Math.hypot(px - x1 - t * dx, py - y1 - t * dy) / r;
        const coverage = strength * (1 - smooth(.86, 1, distance));
        pixels.data[i] = pixels.data[i + 1] = pixels.data[i + 2] = 255;
        pixels.data[i + 3] = Math.max(pixels.data[i + 3], Math.round(255 * coverage));
      }
    }
    if (cache && strokeIndex === settledCount - 1) {
      cache.pixels.set(pixels.data);
      cache.settledCount = settledCount;
    }
  }
  ctx.putImageData(pixels, 0, 0);
}

export const iceTextureUrl = new URL('../../assets/ice/deep-frost.png', import.meta.url).href;

const smooth = (a: number, b: number, t: number) => {
  const v = Math.max(0, Math.min(1, (t - a) / (b - a)));
  return v * v * (3 - 2 * v);
};

function field(x: number, y: number) {
  return Math.max(0, Math.min(
    x * 1.13 + .027 * Math.sin(19 * y) + .017 * Math.cos(43 * y),
    (1 - x) * .99 + .04 * Math.sin(17 * y + 2) + .014 * Math.cos(45 * y),
    y * .88 + .036 * Math.sin(16 * x) + .016 * Math.cos(39 * x + 1),
    (1 - y) * 1.10 + .032 * Math.cos(21 * x + 1) + .018 * Math.sin(42 * x)));
}

export function createIceSurface(width: number, height: number, texture?: HTMLImageElement): IceSurface {
  const canvas = document.createElement('canvas');
  canvas.width = width;
  canvas.height = height;
  const ctx = canvas.getContext('2d')!;
  const grain = ctx.createImageData(width, height);
  let seed = 91;
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      seed = (seed * 1664525 + 1013904223) >>> 0;
      const noise = seed / 4294967296;
      const nx = x / width, ny = y / height;
      const veil = Math.sin(nx * 11 + Math.sin(ny * 8)) * Math.cos(ny * 13 - nx * 4);
      const edge = 1 - smooth(.015, .27, field(nx, ny));
      const i = (y * width + x) * 4;
      grain.data[i] = 204 + noise * 30;
      grain.data[i + 1] = 225 + noise * 22;
      grain.data[i + 2] = 237 + noise * 18;
      grain.data[i + 3] = 15 + 9 * veil + noise * 12 + edge * 35;
    }
  }
  ctx.putImageData(grain, 0, 0);
  if (texture?.complete && texture.naturalWidth) {
    ctx.globalAlpha = .68;
    ctx.drawImage(texture, 0, 0, width, height);
    ctx.globalAlpha = 1;
  }
  return { canvas, width, height };
}

export function paintFogGrowthMask(mask: HTMLCanvasElement, fogElapsedSeconds: number): void {
  const ctx = mask.getContext('2d')!;
  const pixels = ctx.createImageData(mask.width, mask.height);
  const strength = smooth(0, .65, fogElapsedSeconds);
  const front = .012 + .57 * smooth(0, 5, fogElapsedSeconds);
  for (let i = 0; i < mask.width * mask.height; i++) {
    const x = (i % mask.width) / mask.width, y = Math.floor(i / mask.width) / mask.height;
    const threshold = field(x, y);
    const radius = Math.hypot((x - .5) * 2, (y - .5) * 2) / Math.SQRT2;
    const meltThreshold = radius * .91 + .035 * Math.sin(x * 23 + y * 11) * Math.sin(y * 19 - x * 7);
    const meltFront = (fogElapsedSeconds - 17) / 3 * 1.2 - .1;
    const remaining = fogElapsedSeconds >= 20 ? 0 : 1 - smooth(meltThreshold - .08, meltThreshold + .08, meltFront);
    pixels.data[i * 4] = pixels.data[i * 4 + 1] = pixels.data[i * 4 + 2] = 255;
    pixels.data[i * 4 + 3] = Math.round(255 * smooth(threshold - .014, threshold + .065, front) * strength * remaining);
  }
  ctx.putImageData(pixels, 0, 0);
}

export function paintIceSurface(output: HTMLCanvasElement, options: {
  surface: IceSurface;
  growthMask: HTMLCanvasElement;
  erased: HTMLCanvasElement;
  desktop?: HTMLImageElement;
}): void {
  const { surface, growthMask, erased, desktop } = options;
  const { width, height } = surface;
  const ctx = output.getContext('2d')!;
  ctx.clearRect(0, 0, width, height);
  ctx.save();
  if (desktop?.complete && desktop.naturalWidth) {
    ctx.filter = 'blur(13px)';
    ctx.drawImage(desktop, -16, -16, width + 32, height + 32);
    ctx.filter = 'none';
  }
  ctx.drawImage(surface.canvas, 0, 0);
  ctx.globalCompositeOperation = 'destination-in';
  ctx.drawImage(growthMask, 0, 0, width, height);
  ctx.globalCompositeOperation = 'destination-out';
  ctx.drawImage(erased, 0, 0, width, height);
  ctx.restore();
}
