<template>
  <div class="panel">
    <div class="drag-bar" data-tauri-drag-region />
    <header>
      <span class="title">Settings</span>
      <button class="close-btn" @click="closePanel">×</button>
    </header>
    <div class="divider" />

    <!-- Break Interval -->
    <section class="section">
      <div class="label-caps">BREAK INTERVAL</div>
      <div class="card">
        <div class="row-spread">
          <span class="field-label">Every</span>
          <span class="pill-value">{{ Math.round(config.interval_secs / 60) }} min</span>
        </div>
        <input
          type="range"
          min="1200"
          max="3600"
          step="300"
          v-model.number="config.interval_secs"
          class="slider"
        />
        <div class="range-row">
          <span>20 min</span><span>60 min</span>
        </div>
        <p class="note">Medical rec: 20 min · AAO 20-20-20 Rule</p>
      </div>
    </section>

    <!-- Intervention Duration (locked) -->
    <section class="section">
      <div class="label-caps">INTERVENTION DURATION</div>
      <div class="card">
        <div class="locked-row">
          <span class="field-dim">Blur spreading</span>
          <span class="locked-val">20s 🔒</span>
        </div>
        <div class="locked-row mt4">
          <span class="field-dim">Blur holding</span>
          <span class="locked-val">20s 🔒</span>
        </div>
        <p class="note">Fixed — 20-20-20 standard · not adjustable</p>
      </div>
    </section>

    <!-- Hotkey -->
    <section class="section">
      <div class="label-caps">ESCAPE HOTKEY</div>
      <div class="card">
        <p class="field-label" style="margin-bottom:8px">Skip intervention at any time</p>
        <div class="keys-row">
          <kbd>Ctrl</kbd><span class="plus">+</span>
          <kbd>Shift</kbd><span class="plus">+</span>
          <kbd>Space</kbd>
        </div>
        <p class="note" style="margin-top:6px;cursor:default">Click to rebind (coming soon)</p>
      </div>
    </section>

    <!-- Pause -->
    <section class="section">
      <div class="label-caps">PAUSE BLINK</div>
      <div class="pills-row">
        <button
          v-for="opt in PAUSE_OPTS"
          :key="opt.secs"
          class="pill-btn"
          :class="{ selected: activePause === opt.secs }"
          @click="triggerPause(opt)"
        >{{ opt.label }}</button>
      </div>
    </section>

    <!-- System -->
    <section class="section">
      <div class="label-caps">SYSTEM</div>
      <div class="card">
        <div class="row-spread">
          <span class="field-label">Launch at Windows startup</span>
          <button
            class="toggle"
            :class="{ on: config.auto_start }"
            @click="config.auto_start = !config.auto_start"
          >
            <span class="thumb" />
          </button>
        </div>
      </div>
    </section>

    <button class="save-btn" @click="save">Save</button>
    <footer>blink-rs v0.1.0 · MIT · Zero telemetry</footer>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, onMounted } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { getConfig, saveConfig, pauseBlink } from '../../composables/useBlink';
import type { DaemonConfig } from '../../types/blink';

const config = reactive<DaemonConfig>({
  interval_secs: 1200,
  warning_secs: 5,
  spreading_secs: 20,
  holding_secs: 20,
  dissolving_secs: 10,
  hotkey: 'ctrl+shift+space',
  auto_start: true,
});

const activePause = ref<number | null>(null);

const PAUSE_OPTS = [
  { label: '1 Hour', secs: 3600 },
  { label: '3 Hours', secs: 10800 },
  { label: '☾  Today Off', secs: -1 },
] as const;

onMounted(async () => {
  const remote = await getConfig();
  Object.assign(config, remote);
});

async function triggerPause(opt: { label: string; secs: number }) {
  activePause.value = opt.secs;
  if (opt.secs === -1) {
    const now = new Date();
    const midnight = new Date(now);
    midnight.setDate(midnight.getDate() + 1);
    midnight.setHours(0, 0, 0, 0);
    const secs = Math.max(0, Math.floor((midnight.getTime() - now.getTime()) / 1000));
    await pauseBlink(secs);
  } else {
    await pauseBlink(opt.secs);
  }
}

async function save() {
  await saveConfig({ ...config });
  await closePanel();
}

async function closePanel() {
  await getCurrentWindow().hide();
}
</script>

<style scoped>
.panel {
  width: 340px;
  min-height: 490px;
  background: #1e1e2e;
  color: #cdd6f4;
  font-family: Inter, 'Segoe UI', sans-serif;
  display: flex;
  flex-direction: column;
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.08);
  overflow: hidden;
  position: relative;
}
.drag-bar {
  position: absolute;
  top: 0; left: 0; right: 48px;
  height: 48px; cursor: move;
}
header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 20px; height: 48px; flex-shrink: 0;
}
.title { font-size: 14px; font-weight: 600; }
.close-btn {
  background: none; border: none; color: #6c7086;
  font-size: 18px; cursor: pointer; padding: 0; line-height: 1;
}
.close-btn:hover { color: #f38ba8; }
.divider { height: 1px; background: rgba(255, 255, 255, 0.06); flex-shrink: 0; }
.section { padding: 10px 20px; }
.label-caps {
  font-size: 10px; text-transform: uppercase; letter-spacing: 1.5px;
  color: #6c7086; font-weight: 600; font-family: 'JetBrains Mono', monospace;
  margin-bottom: 6px;
}
.card {
  background: #181825; border-radius: 8px; padding: 12px;
}
.row-spread { display: flex; justify-content: space-between; align-items: center; }
.field-label { font-size: 13px; color: #bac2de; }
.field-dim { font-size: 12px; color: #6c7086; }
.locked-val { font-size: 12px; color: #45475a; }
.locked-row { display: flex; justify-content: space-between; }
.mt4 { margin-top: 6px; }
.note { font-size: 10px; color: #45475a; margin-top: 8px; font-style: italic; }
.pill-value {
  background: rgba(137, 180, 250, 0.12);
  border: 1px solid rgba(137, 180, 250, 0.35);
  color: #89b4fa; border-radius: 20px; padding: 2px 10px; font-size: 12px;
}
.slider {
  width: 100%; margin: 10px 0 4px;
  accent-color: #89b4fa;
}
.range-row {
  display: flex; justify-content: space-between;
  font-size: 10px; color: #45475a;
}
.keys-row { display: flex; align-items: center; gap: 5px; }
kbd {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 4px; padding: 3px 8px;
  font-family: 'JetBrains Mono', monospace; font-size: 11px; color: #cdd6f4;
}
.plus { color: #45475a; font-size: 12px; }
.pills-row { display: flex; gap: 6px; }
.pill-btn {
  border-radius: 20px; padding: 5px 12px; font-size: 12px; cursor: pointer;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #6c7086; transition: all 0.15s;
}
.pill-btn.selected {
  background: rgba(137, 180, 250, 0.15);
  border-color: rgba(137, 180, 250, 0.4);
  color: #89b4fa;
}
.toggle {
  width: 42px; height: 24px; border-radius: 12px; border: none;
  cursor: pointer; position: relative;
  background: rgba(255, 255, 255, 0.1); transition: background 0.2s;
}
.toggle.on { background: rgba(166, 227, 161, 0.6); }
.thumb {
  position: absolute; width: 18px; height: 18px; border-radius: 50%;
  background: #fff; top: 3px; left: 3px;
  transition: transform 0.2s; box-shadow: 0 1px 4px rgba(0, 0, 0, 0.4);
}
.toggle.on .thumb { transform: translateX(18px); }
.save-btn {
  margin: 8px 20px 0;
  padding: 9px; background: rgba(137, 180, 250, 0.15);
  border: 1px solid rgba(137, 180, 250, 0.4);
  color: #89b4fa; border-radius: 8px; font-size: 13px;
  cursor: pointer; transition: background 0.15s;
}
.save-btn:hover { background: rgba(137, 180, 250, 0.25); }
footer {
  text-align: center; font-size: 10px; color: #45475a;
  padding: 10px 20px 14px; margin-top: auto;
}
</style>
