<template>
  <!-- Live blurred desktop, double-buffered. Each new captured frame is loaded
       into the hidden image and revealed with a short crossfade once decoded, so
       the refresh never shows a gap (no flicker). Each image is blurred
       individually (cached), so the crossfade is a cheap composite. The wrapper
       applies the spreading mask (--clear) and frost opacity (--frost). -->
  <div class="capture">
    <img
      v-if="bufA"
      class="capture-buf"
      :class="{ show: shown === 'a' }"
      :src="bufA"
      alt=""
      draggable="false"
      @load="onLoad('a')"
    />
    <img
      v-if="bufB"
      class="capture-buf"
      :class="{ show: shown === 'b' }"
      :src="bufB"
      alt=""
      draggable="false"
      @load="onLoad('b')"
    />
  </div>
</template>

<script setup lang="ts">
import { ref, watch } from 'vue';

const props = defineProps<{ src: string }>();

const bufA = ref('');
const bufB = ref('');
const shown = ref<'a' | 'b'>('a');
let pending: 'a' | 'b' | null = null;

// Load each new frame into the hidden buffer; reveal it once it has decoded.
watch(
  () => props.src,
  (url) => {
    if (!url) return;
    const target = shown.value === 'a' ? 'b' : 'a';
    pending = target;
    if (target === 'a') bufA.value = url;
    else bufB.value = url;
  }
);

function onLoad(which: 'a' | 'b') {
  if (which === pending) {
    shown.value = which;
    pending = null;
  }
}
</script>

<style scoped>
.capture {
  position: fixed;
  inset: 0;
  transform: scale(1.06); /* hide blur edge sampling */
  pointer-events: none;
  opacity: var(--frost, 0);
  /* Spreading iris: transparent (clear, live desktop) inside --clear, then only a
     NARROW arc (~14%) ramps to fully opaque — everything past it is near-opaque
     blur. So just the moving arc is see-through; the rest is solidly frosted.
     Overridden for holding (focal gradient) in OverlayApp. */
  -webkit-mask-image: radial-gradient(
    ellipse farthest-side at 50% 50%,
    transparent calc(var(--clear, 0.92) * 100%),
    #000 calc(var(--clear, 0.92) * 100% + 14%)
  );
  mask-image: radial-gradient(
    ellipse farthest-side at 50% 50%,
    transparent calc(var(--clear, 0.92) * 100%),
    #000 calc(var(--clear, 0.92) * 100% + 14%)
  );
  transition: opacity 0.4s ease;
  will-change: opacity, mask-image;
}
.capture-buf {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  object-fit: cover;
  /* Blur each frame (cached); --blur grows for holding, eased via filter. */
  filter: blur(var(--blur, 12px)) saturate(0.96) brightness(0.94);
  opacity: 0;
  transition: opacity 0.25s linear, filter 0.5s ease;
}
.capture-buf.show {
  opacity: 1;
}
</style>
