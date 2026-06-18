import { ref, watch, onUnmounted, type Ref } from 'vue';
import type { Phase } from '../types/blink';

/** Largest clear hole (per axis fraction). Must match Rust HOLE_MAX in state.rs. */
export const HOLE_MAX = 0.92;
const SPREADING_MS = 20000;
const DISSOLVING_MS = 10000;
/** Max blur-layer opacity. Matches the Stitch design's ~0.9 frost; the clear
    center (mask transparent) still shows the live desktop, so this reads as a
    frosted ring around a clear focus, not a dark cover. */
const MAX_BLUR = 0.9;

function ease(t: number): number {
  t = Math.min(Math.max(t, 0), 1);
  return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2;
}

/**
 * Drives the overlay animation from the daemon phase.
 *
 * - `holeFrac`: clear-center fraction of the viewport per axis (0..HOLE_MAX).
 *   Mirrors the SetWindowRgn hole geometry so the blurred snapshot's radial
 *   mask lines up exactly with the physical hole.
 * - `blurOpacity`: 0..1 strength of the blur/vignette layers.
 * - `progress`: 0..1 completion of the current timed phase (spreading/dissolving).
 */
export function useOverlayAnim(phase: Ref<Phase>) {
  const holeFrac = ref(HOLE_MAX);
  const blurOpacity = ref(0);
  const progress = ref(0);
  let raf = 0;

  function animate(durationMs: number, apply: (e: number) => void) {
    cancelAnimationFrame(raf);
    const start = performance.now();
    function step(now: number) {
      const t = Math.min((now - start) / durationMs, 1);
      progress.value = t;
      apply(ease(t));
      if (t < 1) raf = requestAnimationFrame(step);
    }
    raf = requestAnimationFrame(step);
  }

  watch(
    phase,
    (p) => {
      cancelAnimationFrame(raf);
      progress.value = 0;
      if (p === 'warning') {
        // Warning: big clear hole (only the edge frame is affected) and NO blur —
        // the circuit traces are the warning cue. Center stays the live screen.
        holeFrac.value = HOLE_MAX;
        blurOpacity.value = 0;
      } else if (p === 'spreading') {
        animate(SPREADING_MS, (e) => {
          holeFrac.value = HOLE_MAX * (1 - e);
          blurOpacity.value = MAX_BLUR * e;
        });
      } else if (p === 'holding') {
        holeFrac.value = 0;
        blurOpacity.value = MAX_BLUR;
      } else if (p === 'dissolving') {
        animate(DISSOLVING_MS, (e) => {
          holeFrac.value = HOLE_MAX * e;
          blurOpacity.value = MAX_BLUR * (1 - e);
        });
      } else {
        holeFrac.value = HOLE_MAX;
        blurOpacity.value = 0;
      }
    },
    { immediate: true }
  );

  onUnmounted(() => cancelAnimationFrame(raf));

  return { holeFrac, blurOpacity, progress };
}
