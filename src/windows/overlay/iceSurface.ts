export interface IceSurface {
  readonly canvas: HTMLCanvasElement;
  readonly width: number;
  readonly height: number;
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
  const strength = smooth(0, .65, fogElapsedSeconds) * (1 - smooth(17, 20, fogElapsedSeconds));
  const front = .012 + .57 * smooth(0, 5, fogElapsedSeconds);
  for (let i = 0; i < mask.width * mask.height; i++) {
    const threshold = field((i % mask.width) / mask.width, Math.floor(i / mask.width) / mask.height);
    pixels.data[i * 4] = pixels.data[i * 4 + 1] = pixels.data[i * 4 + 2] = 255;
    pixels.data[i * 4 + 3] = Math.round(255 * smooth(threshold - .014, threshold + .065, front) * strength);
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
