<template>
  <div class="panel">
    <!-- Sticky header -->
    <div class="header-wrap">
      <div class="drag-bar" data-tauri-drag-region />
      <header>
        <span class="title">Settings</span>
        <button class="close-btn" @click="closePanel">×</button>
      </header>
      <div class="divider" />
    </div>

    <!-- Scrollable body -->
    <div class="body">
      <!-- Break Interval -->
      <section class="section">
        <div class="label-caps">BREAK INTERVAL</div>
        <div class="card">
          <div class="row-spread">
            <span class="field-label">Every</span>
            <span class="pill-value">{{ intervalMinutes }} min</span>
          </div>
          <input
            type="range"
            min="60"
            max="9000"
            step="60"
            v-model.number="config.interval_secs"
            class="slider"
          />
          <div class="range-row">
            <span>1 min</span><span>150 min</span>
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
        </div>
      </section>

      <!-- Pause -->
      <section class="section">
        <div class="label-caps">PAUSE BLINK</div>

        <div class="pause-status">
          <span class="status-dot" :class="isPaused ? 'dot-amber' : 'dot-green'" />
          <span class="status-label" :class="isPaused ? 'col-amber' : 'col-green'">
            {{ isPaused ? `Paused until ${pauseUntilFormatted}` : 'Active' }}
          </span>
          <button v-if="isPaused" class="resume-link" @click="resume">Resume</button>
        </div>

        <div class="pills-row">
          <button
            v-for="opt in PAUSE_OPTS"
            :key="opt.label"
            class="pill-btn"
            :class="{ selected: isPaused && activePause === opt.label }"
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

      <button class="save-btn" :class="{ saved: saveFlash }" @click="save">
        {{ saveFlash ? '✓ Saved' : 'Save' }}
      </button>
      <footer>blink-rs v0.1.0 · MIT · Zero telemetry</footer>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { sendNotification } from '@tauri-apps/plugin-notification';
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

const intervalMinutes = computed(() => Math.round(config.interval_secs / 60));
const saveFlash = ref(false);

// --- Pause state ---
const PAUSE_OPTS = [
  { label: '1 Hour', secs: 3600 },
  { label: '3 Hours', secs: 10800 },
  { label: 'Today Off', secs: -1 },
] as const;

const PAUSE_KEY = 'blink_pause_until';
const pauseUntilTs = ref(0);
const activePause = ref<string | null>(null);
const now = ref(Date.now());
let ticker: ReturnType<typeof setInterval>;

const isPaused = computed(() => pauseUntilTs.value > now.value);
const pauseUntilFormatted = computed(() => {
  if (!isPaused.value) return '';
  return new Date(pauseUntilTs.value).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
});

function loadPauseState() {
  const stored = localStorage.getItem(PAUSE_KEY);
  if (stored) {
    const data = JSON.parse(stored) as { ts: number; label: string };
    if (data.ts > Date.now()) {
      pauseUntilTs.value = data.ts;
      activePause.value = data.label;
    } else {
      localStorage.removeItem(PAUSE_KEY);
    }
  }
}

async function triggerPause(opt: { label: string; secs: number }) {
  let secs = opt.secs;
  if (secs === -1) {
    const midnight = new Date();
    midnight.setDate(midnight.getDate() + 1);
    midnight.setHours(0, 0, 0, 0);
    secs = Math.max(1, Math.floor((midnight.getTime() - Date.now()) / 1000));
  }
  await pauseBlink(secs);
  const ts = Date.now() + secs * 1000;
  localStorage.setItem(PAUSE_KEY, JSON.stringify({ ts, label: opt.label }));
  pauseUntilTs.value = ts;
  activePause.value = opt.label;
}

async function resume() {
  await pauseBlink(0);
  localStorage.removeItem(PAUSE_KEY);
  pauseUntilTs.value = 0;
  activePause.value = null;
}

onMounted(async () => {
  const remote = await getConfig();
  Object.assign(config, remote);
  loadPauseState();
  ticker = setInterval(() => { now.value = Date.now(); }, 1000);
});

onUnmounted(() => clearInterval(ticker));

async function save() {
  await saveConfig({ ...config });
  saveFlash.value = true;
  setTimeout(() => { saveFlash.value = false; }, 2000);
  sendNotification({ title: 'Blink', body: 'Settings saved.' });
}

async function closePanel() {
  await invoke('close_panel');
}
</script>

<style scoped>
.panel {
  width: 340px;
  height: 490px;
  background: #1e1e2e;
  color: #cdd6f4;
  font-family: Inter, 'Segoe UI', sans-serif;
  display: flex;
  flex-direction: column;
  border-radius: 14px;
  border: 1px solid rgba(255, 255, 255, 0.08);
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
  top: 0; left: 0; right: 48px;
  height: 48px; cursor: move;
}
header {
  display: flex; align-items: center; justify-content: space-between;
  padding: 0 20px; height: 48px;
}
.title { font-size: 14px; font-weight: 600; }
.close-btn {
  position: relative; z-index: 1;
  background: none; border: none; color: #6c7086;
  font-size: 18px; cursor: pointer; padding: 4px 8px; line-height: 1;
}
.close-btn:hover { color: #f38ba8; }
.divider { height: 1px; background: rgba(255, 255, 255, 0.06); }

/* Scrollable body */
.body {
  flex: 1;
  overflow-y: auto;
  padding-bottom: 4px;
}

.section { padding: 10px 20px; }
.label-caps {
  font-size: 10px; text-transform: uppercase; letter-spacing: 1.5px;
  color: #6c7086; font-weight: 600; font-family: 'JetBrains Mono', monospace;
  margin-bottom: 6px;
}
.card { background: #181825; border-radius: 8px; padding: 12px; }
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
.slider { width: 100%; margin: 10px 0 4px; accent-color: #89b4fa; }
.range-row { display: flex; justify-content: space-between; font-size: 10px; color: #45475a; }
.keys-row { display: flex; align-items: center; gap: 5px; }
kbd {
  background: rgba(255, 255, 255, 0.08);
  border: 1px solid rgba(255, 255, 255, 0.14);
  border-radius: 4px; padding: 3px 8px;
  font-family: 'JetBrains Mono', monospace; font-size: 11px; color: #cdd6f4;
}
.plus { color: #45475a; font-size: 12px; }

/* Pause */
.pause-status { display: flex; align-items: center; gap: 6px; margin-bottom: 8px; font-size: 12px; }
.status-dot { width: 6px; height: 6px; border-radius: 50%; flex-shrink: 0; }
.dot-green { background: #a6e3a1; box-shadow: 0 0 4px #a6e3a1; }
.dot-amber { background: #f9e2af; box-shadow: 0 0 4px #f9e2af; }
.status-label { flex: 1; }
.col-green { color: #a6e3a1; }
.col-amber { color: #f9e2af; }
.resume-link {
  background: none; border: none; color: #6c7086; font-size: 11px;
  cursor: pointer; text-decoration: underline; padding: 0;
}
.resume-link:hover { color: #89b4fa; }
.pills-row { display: flex; gap: 6px; }
.pill-btn {
  border-radius: 20px; padding: 5px 12px; font-size: 12px; cursor: pointer;
  background: rgba(255, 255, 255, 0.05);
  border: 1px solid rgba(255, 255, 255, 0.1);
  color: #6c7086; transition: all 0.15s;
}
.pill-btn.selected {
  background: rgba(249, 226, 175, 0.12);
  border-color: rgba(249, 226, 175, 0.4);
  color: #f9e2af;
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
  display: block; width: calc(100% - 40px); margin: 8px 20px 0;
  padding: 9px; background: rgba(137, 180, 250, 0.15);
  border: 1px solid rgba(137, 180, 250, 0.4);
  color: #89b4fa; border-radius: 8px; font-size: 13px;
  cursor: pointer; transition: background 0.15s, color 0.15s;
}
.save-btn:hover { background: rgba(137, 180, 250, 0.25); }
.save-btn.saved {
  background: rgba(166, 227, 161, 0.15);
  border-color: rgba(166, 227, 161, 0.4);
  color: #a6e3a1;
}
footer {
  text-align: center; font-size: 10px; color: #45475a;
  padding: 10px 20px 14px; margin-top: auto;
}
</style>
