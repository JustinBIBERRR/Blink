<template>
  <div class="panel">
    <!-- Sticky header -->
    <div class="header-wrap">
      <div class="drag-bar" data-tauri-drag-region></div>
      <header>
        <div class="brand">
          <span class="eye">👁</span>
          <strong>Blink</strong>
          <span class="sep">·</span>
          <span class="subtitle">Eye Health</span>
        </div>
        <button class="close-btn" @click="closePanel">×</button>
      </header>
      <div class="divider" />
    </div>

    <!-- Scrollable body -->
    <div class="body">
      <!-- Today -->
      <section class="section">
        <div class="row-spread">
          <span class="label-caps">TODAY</span>
          <span class="count-text">{{ today.completed }} / {{ today.total_sessions || 8 }} sessions</span>
        </div>
        <div class="progress-track">
          <div class="progress-fill" :style="{ width: today.compliance_pct + '%' }" />
        </div>
        <div class="note">{{ today.compliance_pct }}% compliance · {{ Math.max(0, 8 - today.total_sessions) }} remaining</div>
      </section>
      <div class="divider" />

      <!-- This Week -->
      <section class="section">
        <div class="label-caps">THIS WEEK</div>
        <div class="bar-chart">
          <div v-for="(day, i) in week" :key="day.date" class="bar-col">
            <div class="bar-wrap">
              <div
                class="bar"
                :class="barClass(day)"
                :style="{ height: barHeight(day) + '%' }"
              />
            </div>
            <span class="day-label">{{ DAY_LABELS[i] }}</span>
          </div>
        </div>
      </section>
      <div class="divider" />

      <!-- This Month -->
      <section class="section">
        <div class="label-caps">THIS MONTH</div>
        <div class="heatmap">
          <span v-for="(d, i) in 'MTWTFSS'.split('')" :key="'header-' + i" class="heat-header">{{ d }}</span>
          <div
            v-for="(cell, i) in monthPadded"
            :key="i"
            class="heat-cell"
            :class="heatClass(cell)"
            :title="cell ? cell.date : ''"
          />
        </div>
      </section>
      <div class="divider" />

      <!-- Stat Cards -->
      <section class="section cards-row">
        <div class="stat-card">
          <span class="card-num green-text">{{ totals.completed }}</span>
          <span class="card-label">Completed</span>
        </div>
        <div class="stat-card">
          <span class="card-num blue-text">{{ totals.early_exit }}</span>
          <span class="card-label">Early Exit</span>
        </div>
        <div class="stat-card">
          <span class="card-num red-text">{{ totals.skipped }}</span>
          <span class="card-label">Skipped</span>
        </div>
        <div class="stat-card">
          <span class="card-num amber-text">{{ totals.warn_skipped }}</span>
          <span class="card-label">Warn Skip</span>
        </div>
      </section>

      <footer>All data stored locally · 90-day rolling window</footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getStatsToday, getStatsWeek, getStatsMonth } from '../../composables/useBlink';
import type { StatsToday, DayStats } from '../../types/blink';

const today = ref<StatsToday>({
  completed: 0, early_exit: 0, skipped: 0, warn_skipped: 0,
  total_sessions: 0, compliance_pct: 0,
});
const week = ref<DayStats[]>([]);
const month = ref<DayStats[]>([]);

const DAY_LABELS = ['M', 'T', 'W', 'T', 'F', 'S', 'S'];

onMounted(async () => {
  [today.value, week.value, month.value] = await Promise.all([
    getStatsToday(),
    getStatsWeek(),
    getStatsMonth(),
  ]);
});

const totals = computed(() => ({
  completed: month.value.reduce((s, d) => s + d.completed, 0),
  early_exit: month.value.reduce((s, d) => s + d.early_exit, 0),
  skipped: month.value.reduce((s, d) => s + d.skipped, 0),
  warn_skipped: month.value.reduce((s, d) => s + d.warn_skipped, 0),
}));

function dayTotal(day: DayStats): number {
  return day.completed + day.early_exit + day.skipped + day.warn_skipped;
}

function barHeight(day: DayStats): number {
  const total = dayTotal(day);
  if (total === 0) return 6;
  return Math.max(6, Math.round((day.completed / Math.max(total, 8)) * 100));
}

function barClass(day: DayStats): string {
  const total = dayTotal(day);
  if (total === 0) return 'empty';
  const rate = day.completed / total;
  if (rate >= 0.75) return 'green';
  if (rate >= 0.45) return 'amber';
  return 'red';
}

function heatClass(day: DayStats | null): string {
  if (!day) return 'future';
  const total = dayTotal(day);
  if (total === 0) return 'future';
  const rate = day.completed / total;
  if (rate >= 0.8) return 'green-hi';
  if (rate >= 0.5) return 'green-lo';
  if (rate >= 0.25) return 'amber';
  return 'red';
}

// Pad month data so first day falls on correct weekday (Mon=0)
const monthPadded = computed<(DayStats | null)[]>(() => {
  if (!month.value.length) return [];
  const firstDate = new Date(month.value[0].date + 'T00:00:00');
  // getDay(): 0=Sun, 1=Mon... convert to Mon=0
  const firstDay = (firstDate.getDay() + 6) % 7;
  return [...Array(firstDay).fill(null), ...month.value];
});

async function closePanel() {
  await invoke('close_panel');
}
</script>

<style scoped>
.panel {
  width: 380px;
  height: 560px;
  background: #1e1e2e;
  color: #cdd6f4;
  font-family: Inter, 'Segoe UI', sans-serif;
  display: flex;
  flex-direction: column;
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.07);
  overflow: hidden;
}

/* Sticky header */
.header-wrap {
  flex-shrink: 0;
  position: relative;
  background: #1e1e2e;
  z-index: 10;
}
.drag-bar {
  position: absolute;
  top: 0; left: 0; right: 56px;
  height: 52px;
  cursor: move;
}
header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 20px;
  height: 52px;
}
.brand { display: flex; align-items: center; gap: 5px; font-size: 14px; }
.eye { color: #89b4fa; }
.sep { color: #45475a; }
.subtitle { color: #6c7086; font-size: 12px; }
.close-btn {
  position: relative; z-index: 1;
  background: none; border: none;
  color: #6c7086; font-size: 18px;
  cursor: pointer; line-height: 1; padding: 4px 8px;
}
.close-btn:hover { color: #f38ba8; }
.divider { height: 1px; background: rgba(255, 255, 255, 0.06); }

/* Scrollable body */
.body {
  flex: 1;
  overflow-y: auto;
}

.section { padding: 12px 20px; }
.row-spread {
  display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;
}
.label-caps {
  font-size: 10px; text-transform: uppercase; letter-spacing: 1.5px;
  color: #6c7086; font-weight: 600; font-family: 'JetBrains Mono', monospace;
}
.count-text { font-size: 12px; color: #bac2de; }
.progress-track {
  height: 6px; background: rgba(255, 255, 255, 0.08);
  border-radius: 3px; overflow: hidden;
}
.progress-fill {
  height: 100%;
  background: linear-gradient(90deg, #89b4fa, #a6e3a1);
  border-radius: 3px;
  transition: width 0.4s ease;
}
.note { font-size: 11px; color: #6c7086; margin-top: 6px; }

/* Bar chart */
.bar-chart {
  display: flex; align-items: flex-end; gap: 5px;
  height: 72px; margin-top: 8px;
}
.bar-col { display: flex; flex-direction: column; align-items: center; gap: 3px; flex: 1; }
.bar-wrap { flex: 1; width: 100%; display: flex; align-items: flex-end; }
.bar { width: 100%; border-radius: 2px 2px 0 0; min-height: 4px; transition: height 0.3s; }
.bar.green { background: #a6e3a1; }
.bar.amber { background: #f9e2af; }
.bar.red { background: #f38ba8; }
.bar.empty { background: rgba(255, 255, 255, 0.06); }
.day-label { font-size: 9px; color: #45475a; }

/* Heatmap */
.heatmap {
  display: grid; grid-template-columns: repeat(7, 1fr); gap: 3px; margin-top: 8px;
}
.heat-header { font-size: 9px; color: #45475a; text-align: center; padding-bottom: 2px; }
.heat-cell { height: 20px; border-radius: 3px; }
.heat-cell.green-hi { background: rgba(166, 227, 161, 0.9); }
.heat-cell.green-lo { background: rgba(166, 227, 161, 0.45); }
.heat-cell.amber { background: rgba(249, 226, 175, 0.55); }
.heat-cell.red { background: rgba(243, 139, 168, 0.35); }
.heat-cell.future { background: rgba(255, 255, 255, 0.04); }

/* Cards */
.cards-row {
  display: grid; grid-template-columns: repeat(4, 1fr); gap: 6px;
}
.stat-card {
  background: rgba(255, 255, 255, 0.04); border-radius: 8px; padding: 10px 8px;
  text-align: center; display: flex; flex-direction: column; align-items: center; gap: 4px;
}
.card-num { font-size: 22px; font-weight: 700; line-height: 1; }
.card-label { font-size: 9px; color: #6c7086; }
.green-text { color: #a6e3a1; }
.blue-text  { color: #89b4fa; }
.red-text   { color: #f38ba8; }
.amber-text { color: #f9e2af; }

footer {
  text-align: center; font-size: 10px; color: #45475a;
  padding: 10px 20px 14px;
}
</style>
