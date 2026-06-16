import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { emit } from '@tauri-apps/api/event';
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
  return emit('blink://pause', secs);
}
