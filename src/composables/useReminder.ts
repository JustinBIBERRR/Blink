import { onMounted, onUnmounted, ref } from 'vue';
import { listen } from '@tauri-apps/api/event';
import { invoke } from '@tauri-apps/api/core';
export type Eraser = { enabled: boolean; held: boolean; radius: number; cursor: [number,number] | null; strokes: [number,number,number,number,number][] };
export type Snapshot = { generation: number; round_id: number | null; elapsed_ms: number; phase: string; mode: 'soft' | 'fog'; eraser: Eraser };
export function useReminder() {
  const snapshot = ref<Snapshot>({ generation: 0, round_id: null, elapsed_ms: 0, phase: 'monitoring', mode: 'soft', eraser: { enabled: false, held: false, radius: 38, cursor: null, strokes: [] } });
  const receivedAt = ref(performance.now());
  let stop: (() => void) | undefined;
  let disposed = false;
  function accept(next: Snapshot) {
    if (disposed || next.generation < snapshot.value.generation) return;
    // A slower initial query must not rewind an already received same-round event.
    if (next.generation === snapshot.value.generation && next.elapsed_ms < snapshot.value.elapsed_ms) return;
    snapshot.value = next; receivedAt.value = performance.now();
  }
  onMounted(async () => {
    const unlisten = await listen<Snapshot>('blink://reminder', event => accept(event.payload));
    if (disposed) { unlisten(); return; }
    stop = unlisten;
    await invoke('overlay_ready');
    accept(await invoke<Snapshot>('get_reminder'));
  });
  onUnmounted(() => { disposed = true; stop?.(); });
  return { snapshot, receivedAt };
}
