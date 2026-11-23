export type Phase =
  | 'monitoring'
  | 'warning'
  | 'spreading'
  | 'holding'
  | 'dissolving';

export interface StatsToday {
  completed: number;
  early_exit: number;
  skipped: number;
  warn_skipped: number;
  total_sessions: number;
  compliance_pct: number;
}

export interface DayStats {
  date: string;        // "2026-06-16"
  completed: number;
  early_exit: number;
  skipped: number;
  warn_skipped: number;
}

export interface DaemonConfig {
  reminder_mode: 'soft' | 'fog';
  eraser_enabled: boolean;
  interval_secs: number;
  warning_secs: number;
  spreading_secs: number;
  holding_secs: number;
  dissolving_secs: number;
  hotkey: string;
  auto_start: boolean;
}
