import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { ref, onMounted, onUnmounted } from 'vue';
import type { Phase, StatsToday, DayStats, DaemonConfig } from '../types/blink';

/** Reactive phase — updates whenever blink://phase is emitted */
export function usePhase() {
  const phase = ref<Phase>('monitoring');
  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<Phase>('blink://phase', (event) => {
      phase.value = event.payload;
    });
  });

  onUnmounted(() => {
    unlisten?.();
  });

  return { phase };
}

/**
 * Frozen desktop snapshot captured by the Rust backend at the start of a break.
 * Emitted once per cycle as a `data:image/bmp;base64,...` URL (empty on failure).
 * The overlay blurs this behind a radial mask so the clear hole shows the live
 * desktop while the surrounding frame shows the blurred snapshot.
 */
export function useCapture() {
  const capture = ref<string>('');
  let unlisten: (() => void) | null = null;

  onMounted(async () => {
    unlisten = await listen<string>('blink://capture', (event) => {
      capture.value = event.payload ?? '';
    });
  });

  onUnmounted(() => {
    unlisten?.();
  });

  return { capture };
}

const HOTKEY_LABELS: Record<string, string> = {
  ctrl: 'Ctrl', shift: 'Shift', alt: 'Alt', win: 'Win', space: 'Space',
  esc: 'Esc', enter: 'Enter', tab: 'Tab', left: '←', right: '→', up: '↑', down: '↓',
};

/** Format a `+`-joined token hotkey (e.g. "ctrl+shift+esc") for display ("Ctrl+Shift+Esc"). */
export function formatHotkey(hotkey: string): string {
  return (hotkey || '')
    .split('+')
    .map((s) => s.trim())
    .filter(Boolean)
    .map((t) => HOTKEY_LABELS[t] ?? t.toUpperCase())
    .join('+');
}

/**
 * Reactive, display-formatted escape hotkey. Fetched on mount and refreshed at
 * the start of each break (phase → warning) so the overlay hint always reflects
 * the user's current setting, even after they change it mid-session. Empty when
 * no escape hotkey is configured.
 */
export function useHotkey() {
  const hotkey = ref<string>('');
  let unlisten: (() => void) | null = null;

  const refresh = async () => {
    try {
      const cfg = await getConfig();
      hotkey.value = formatHotkey(cfg.hotkey);
    } catch {
      /* leave previous value on transient error */
    }
  };

  onMounted(async () => {
    await refresh();
    unlisten = await listen<Phase>('blink://phase', (event) => {
      if (event.payload === 'warning') refresh();
    });
  });

  onUnmounted(() => {
    unlisten?.();
  });

  return { hotkey };
}

export async function getStatsToday(): Promise<StatsToday> {
  return invoke<StatsToday>('get_stats_today');
}

export async function getStatsWeek(): Promise<DayStats[]> {
  return invoke<DayStats[]>('get_stats_week');
}

export async function getStatsMonth(): Promise<DayStats[]> {
  return invoke<DayStats[]>('get_stats_month');
}

export async function getConfig(): Promise<DaemonConfig> {
  return invoke<DaemonConfig>('get_config');
}

export async function saveConfig(config: DaemonConfig): Promise<void> {
  return invoke<void>('save_config', { newConfig: config });
}

export async function pauseBlink(secs: number): Promise<void> {
  return invoke('set_pause', { seconds: secs });
}
