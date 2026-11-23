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
            <span class="pill-value">{{ testInterval !== null ? `${testInterval} sec · 本次测试` : `${intervalMinutes} min` }}</span>
          </div>
          <input
            :disabled="testInterval !== null"
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

      <section class="section">
        <div class="label-caps">提示方式</div>
        <div class="card">
          <label class="field-label">提示方式
            <select v-model="config.reminder_mode" aria-label="提示方式">
              <option value="soft">柔和边缘</option><option value="fog">雾气玻璃</option>
            </select>
          </label>
          <label v-if="config.reminder_mode === 'fog'" class="field-label"><input type="checkbox" v-model="config.eraser_enabled" /> 启用黑板擦，局部擦开雾气</label>
          <label v-if="config.reminder_mode === 'fog' && config.eraser_enabled" class="field-label">按住临时拾取
            <select v-model="config.eraser_modifier" aria-label="临时拾取修饰键">
              <option :value="null">关闭</option><option value="ctrl">Ctrl</option><option value="shift">Shift</option><option value="alt">Alt / Option</option><option value="meta">Windows / Command</option>
            </select>
          </label>
          <p v-if="config.reminder_mode === 'fog' && config.eraser_enabled" class="field-label">持擦时滚轮调节尺寸；松开临时拾取键归位。双击归位的黑板擦结束本轮。</p>
          <p class="field-label">{{ config.reminder_mode === 'soft' ? '外缘冷光轻亮约 3 秒，正文保持清晰。' : '3 秒边缘预告后，雾气凝结并在 20 秒内散去。' }}</p>
          <label v-if="config.reminder_mode === 'fog'" class="field-label">
            <input type="checkbox" v-model="config.fog_notifications" /> 无声系统通知
          </label>
          <button class="pill-btn" @click="preview">立即试一次</button>
          <p v-if="previewError" role="alert">{{ previewError }}</p>
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
  reminder_mode: 'soft',
  fog_notifications: true,
  eraser_enabled: false,
  eraser_modifier: null,
  interval_secs: 1200,
  warning_secs: 5,
  spreading_secs: 20,
  holding_secs: 20,
  dissolving_secs: 10,
  auto_start: true,
});

const testInterval = ref<number | null>(null);
const intervalMinutes = computed(() => Math.round(config.interval_secs / 60));
const saveFlash = ref(false);
const previewError = ref('');
async function preview() {
  previewError.value = '';
  try { await saveConfig({ ...config }); await invoke('preview_reminder'); }
  catch (error) { previewError.value = String(error); }
}

// --- Pause state ---
const PAUSE_OPTS = [
  { label: '1 Hour', secs: 3600 },
  { label: '3 Hours', secs: 10800 },
  { label: 'Today Off', secs: -1 },
] as const;

const pauseUntilTs = ref(0);

const now = ref(Date.now());
let ticker: ReturnType<typeof setInterval>;

const isPaused = computed(() => pauseUntilTs.value > now.value);
const pauseUntilFormatted = computed(() => {
  if (!isPaused.value) return '';
  return new Date(pauseUntilTs.value).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
});

async function loadPauseState() {
  const deadline = await invoke<number | null>('get_pause');
  pauseUntilTs.value = deadline === null ? 0 : deadline * 1000;
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
  await loadPauseState();
}

async function resume() {
  await pauseBlink(0);
  await loadPauseState();
}

onMounted(async () => {
  testInterval.value = await invoke<number | null>('get_test_interval');
  const remote = await getConfig();
  Object.assign(config, remote);
  await loadPauseState();
  ticker = setInterval(() => { now.value = Date.now(); void loadPauseState(); }, 1000);
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
