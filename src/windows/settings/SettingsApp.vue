<template>
  <div class="panel">
    <header class="topbar">
      <div class="brand" data-tauri-drag-region><span class="brand-icon" aria-hidden="true">◌</span> Blink <span class="page-name">设置</span></div>
      <div class="header-actions">
        <span class="save-status" :class="{ error: saveState === 'error' }" role="status">{{ saveLabel }}</span>
        <button v-if="saveState === 'error'" class="text-button" @click="retrySave">重试</button>
        <button class="close-button" aria-label="关闭设置" @click="closePanel">×</button>
      </div>
    </header>

    <main class="body">
      <div class="intro"><h1>给眼睛留一点休息时间</h1><p>按你的节奏，提醒自己暂时移开视线。</p></div>
      <p v-if="loadError" class="notice error" role="alert">设置暂时无法读取。<button class="text-button" @click="load">重新加载</button></p>
      <p v-if="actionError" class="notice error" role="alert">{{ actionError }}</p>
      <fieldset class="settings-fields" :disabled="!loaded || previewBusy">
        <div class="main-grid">
          <div class="choices">
            <section class="card interval-card" aria-labelledby="interval-title">
              <div class="section-heading"><h2 id="interval-title">提醒间隔</h2><span class="eyebrow">你的节奏</span></div>
              <div class="interval-value"><span>每隔</span><strong>{{ testInterval ?? intervalMinutes }}</strong><span>{{ testInterval !== null ? '秒' : '分钟' }}，休息一下</span></div>
              <input v-model.number="config.interval_secs" :disabled="testInterval !== null" type="range" min="60" max="9000" step="60" aria-label="提醒间隔（分钟）" :aria-valuetext="`${intervalMinutes} 分钟`" />
              <div class="range-labels"><span>1 分钟</span><span>150 分钟</span></div>
              <p v-if="testInterval !== null" class="test-note">本次测试每 {{ testInterval }} 秒触发，日常间隔仍为 {{ intervalMinutes }} 分钟。</p>
              <p v-else class="description">按使用电脑的时间计算，锁屏和睡眠时暂停计时。</p>
            </section>

            <section aria-labelledby="mode-title">
              <div class="section-heading mode-heading"><h2 id="mode-title">提醒方式</h2><span class="eyebrow">选一种适合你的</span></div>
              <div class="mode-options">
                <label v-for="mode in MODES" :key="mode.value" class="mode-card" :class="{ selected: config.reminder_mode === mode.value }">
                  <input v-model="config.reminder_mode" type="radio" name="reminder-mode" :value="mode.value" />
                  <span class="thumbnail" aria-hidden="true"><svg viewBox="0 0 32 24" fill="none"><rect x="1" y="1" width="30" height="20" rx="3"/><path d="M11 23h10M16 21v2"/></svg><span>效果图待补充</span></span>
                  <span class="mode-name">{{ mode.title }}</span>
                  <span class="mode-summary">{{ mode.summary }}</span>
                </label>
              </div>
            </section>
          </div>

          <section class="card effect-card" aria-labelledby="effect-title">
            <span class="eyebrow">当前提醒</span><h2 id="effect-title" class="effect-title">{{ selectedMode.title }}</h2>
            <p class="effect-description">{{ config.reminder_mode === 'soft' ? '屏幕边缘亮起柔和微光，约 3 秒后消退。屏幕内容保持清晰。' : '先以边缘微光预告 3 秒，再让雾气逐渐覆盖屏幕，20 秒内自动散去。' }}</p>
            <button class="preview-button" :disabled="previewBusy" @click="preview">{{ previewBusy ? '正在准备…' : '体验一次' }}<span aria-hidden="true">↗</span></button>
            <p class="preview-note">立即播放当前选中的提醒效果</p>

            <div v-if="config.reminder_mode === 'fog'" class="effect-options">
              <p v-if="environment?.reduce_transparency" class="notice">系统已开启“降低透明度”。雾气会保留纹理，不模糊背景，擦开后可见真实屏幕。</p>
              <label class="switch-row"><span><strong>黑板擦</strong><small>局部擦开雾气，露出屏幕内容</small></span><input v-model="config.eraser_enabled" type="checkbox" role="switch" /></label>
              <template v-if="config.eraser_enabled">
                <div v-if="environment?.platform === 'macos'" class="permission" :class="{ missing: !environment.accessibility_trusted }">
                  <span>{{ environment.accessibility_trusted ? '辅助功能权限已开启' : '使用黑板擦需要辅助功能权限' }}</span>
                  <button v-if="!environment.accessibility_trusted" class="text-button" @click="openPermission">前往系统设置 ↗</button>
                </div>
                <label class="modifier-row"><span>按键临时拿起</span><select v-model="config.eraser_modifier" aria-label="按键临时拿起黑板擦"><option :value="null">不使用</option><option value="ctrl">Ctrl</option><option value="shift">Shift</option><option value="alt">{{ environment?.platform === 'macos' ? 'Option' : 'Alt' }}</option><option value="meta">{{ environment?.platform === 'macos' ? 'Command' : 'Windows' }}</option></select></label>
              </template>
              <label class="switch-row"><span><strong>系统通知</strong><small>起雾前和自然结束后，无声提醒</small></span><input v-model="config.fog_notifications" type="checkbox" role="switch" /></label>
            </div>
            <div v-else class="soft-note"><span aria-hidden="true">◌</span><p>短暂提醒，保持专注。<br />适合阅读、写作和日常工作。</p></div>
          </section>
        </div>

        <div class="bottom-grid">
          <section class="card" aria-labelledby="pause-title">
            <div class="section-heading"><h2 id="pause-title">暂时停止提醒</h2><span class="status-dot" :class="{ paused: isPaused }" aria-hidden="true" /></div>
            <p class="description pause-description">{{ isPaused ? `已暂停，将于 ${pauseUntilFormatted} 恢复` : '需要一段不被打扰的时间？' }}</p>
            <div class="pause-actions"><button v-if="isPaused" class="secondary-button" :disabled="pauseBusy" @click="resume">现在恢复</button><template v-else><button v-for="opt in PAUSE_OPTS" :key="opt.secs" class="secondary-button" :disabled="pauseBusy" @click="triggerPause(opt.secs)">{{ opt.label }}</button></template></div>
          </section>
          <section class="card" aria-labelledby="general-title">
            <h2 id="general-title">通用设置</h2>
            <label v-if="environment?.auto_start_supported" class="switch-row startup"><span><strong>登录后启动 Blink</strong><small>打开电脑后，自动开始计时</small></span><input v-model="config.auto_start" type="checkbox" role="switch" /></label>
            <p v-else class="description general-description">此平台暂不支持设置自动启动。</p>
            <p class="description">关闭此面板后，Blink 仍会在{{ environment?.platform === 'macos' ? '菜单栏' : '系统托盘' }}运行。</p>
          </section>
        </div>
      </fieldset>
      <footer><span>Blink · 给眼睛一点空隙</span><span>v0.1.0</span></footer>
    </main>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive, computed, watch, onMounted, onUnmounted } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { getConfig, saveConfig, pauseBlink } from '../../composables/useBlink';
import type { DaemonConfig } from '../../types/blink';

const MODES = [
  { value: 'soft', title: '边缘微光', summary: '轻轻亮起，内容清晰' },
  { value: 'fog', title: '雾气玻璃', summary: '雾气渐起，短暂歇息' },
] as const;
const PAUSE_OPTS = [{ label: '1 小时', secs: 3600 }, { label: '3 小时', secs: 10800 }, { label: '今天', secs: -1 }];
interface SettingsEnvironment { platform: string; accessibility_trusted: boolean; reduce_transparency: boolean; auto_start_supported: boolean }
const environment = ref<SettingsEnvironment | null>(null);
const config = reactive<DaemonConfig>({ reminder_mode: 'soft', fog_notifications: true, eraser_enabled: false, eraser_modifier: null, interval_secs: 1200, warning_secs: 5, spreading_secs: 20, holding_secs: 20, dissolving_secs: 10, auto_start: true });
const loaded = ref(false), loadError = ref(false), actionError = ref('');
const testInterval = ref<number | null>(null);
const selectedMode = computed(() => MODES.find(mode => mode.value === config.reminder_mode)!);
const intervalMinutes = computed(() => Math.round(config.interval_secs / 60));
const saveState = ref<'idle' | 'pending' | 'saving' | 'saved' | 'error'>('idle');
const saveLabel = computed(() => ({ idle: '更改将自动保存', pending: '等待保存…', saving: '正在保存…', saved: '已保存', error: '未能保存，请重试' })[saveState.value]);
let persisted = '';
let saveTimer: ReturnType<typeof setTimeout> | undefined;
let saving: Promise<void> | null = null;
let disposed = false;
// Serialize writes so rapid changes cannot let an older configuration win.
async function flushSave() {
  clearTimeout(saveTimer);
  if (!loaded.value) return;
  if (saving) return saving;
  saving = (async () => {
    while (JSON.stringify(config) !== persisted) {
      const snapshot = { ...config };
      saveState.value = 'saving';
      try { await saveConfig(snapshot); }
      catch (error) { saveState.value = 'error'; console.error('Settings save failed', error); throw error; }
      persisted = JSON.stringify(snapshot);
    }
    saveState.value = 'saved';
  })();
  try { await saving; } finally { saving = null; }
}
async function retrySave() { try { await flushSave(); } catch { /* The status offers retry. */ } }
watch(config, () => {
  if (!loaded.value) return;
  saveState.value = 'pending';
  clearTimeout(saveTimer);
  saveTimer = setTimeout(() => { void retrySave(); }, 300);
}, { flush: 'sync' });

const previewBusy = ref(false);
async function preview() {
  actionError.value = ''; previewBusy.value = true;
  try { await flushSave(); await invoke('preview_reminder'); }
  catch (error) { console.error('Preview failed', error); actionError.value = saveState.value === 'error' ? '设置尚未保存，请重试后再体验。' : '暂时无法播放提醒，请稍后再试。'; }
  finally { previewBusy.value = false; }
}
const pauseUntilTs = ref(0), now = ref(Date.now()), pauseBusy = ref(false);
const isPaused = computed(() => pauseUntilTs.value > now.value);
const pauseUntilFormatted = computed(() => new Date(pauseUntilTs.value).toLocaleTimeString('zh-CN', { hour: '2-digit', minute: '2-digit', hour12: false }));
async function refreshStatus() {
  const [deadline, env] = await Promise.all([invoke<number | null>('get_pause'), invoke<SettingsEnvironment>('get_settings_environment')]);
  pauseUntilTs.value = deadline === null ? 0 : deadline * 1000;
  environment.value = env;
}
async function triggerPause(seconds: number) {
  pauseBusy.value = true; actionError.value = '';
  if (seconds === -1) {
    const midnight = new Date(); midnight.setDate(midnight.getDate() + 1); midnight.setHours(0, 0, 0, 0);
    seconds = Math.max(1, Math.floor((midnight.getTime() - Date.now()) / 1000));
  }
  try { await pauseBlink(seconds); await refreshStatus(); }
  catch (error) { console.error('Pause update failed', error); actionError.value = '暂停状态未能更新，请重试。'; }
  finally { pauseBusy.value = false; }
}
async function resume() { await triggerPause(0); }
async function openPermission() {
  actionError.value = '';
  try { await invoke('open_accessibility_settings'); }
  catch { actionError.value = '请手动打开系统设置 → 隐私与安全性 → 辅助功能，允许 Blink。'; }
}
async function load() {
  loadError.value = false;
  try {
    const [remote, interval] = await Promise.all([getConfig(), invoke<number | null>('get_test_interval')]);
    if (disposed) return;
    Object.assign(config, remote); persisted = JSON.stringify(config); testInterval.value = interval;
    await refreshStatus(); loaded.value = true;
  } catch (error) { console.error('Settings load failed', error); loadError.value = true; }
}
let ticker: ReturnType<typeof setInterval> | undefined;
function refreshVisibleStatus() {
  now.value = Date.now();
  if (document.visibilityState === 'visible') void refreshStatus().catch(error => console.error('Status refresh failed', error));
}
onMounted(() => { void load(); ticker = setInterval(refreshVisibleStatus, 2000); window.addEventListener('focus', refreshVisibleStatus); });
onUnmounted(() => { disposed = true; clearInterval(ticker); clearTimeout(saveTimer); window.removeEventListener('focus', refreshVisibleStatus); });
async function closePanel() {
  try { await flushSave(); await invoke('close_panel'); }
  catch { actionError.value = '设置尚未保存，关闭前请重试。'; }
}
</script>

<style scoped>
* { box-sizing: border-box; }
.panel { height: 100vh; width: 100%; display: flex; flex-direction: column; background: #141a21; color: #e9eef3; font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', sans-serif; font-size: 14px; }
.topbar { min-height: 58px; display: flex; align-items: center; justify-content: space-between; padding: 0 23px; border-bottom: 1px solid #ffffff0d; }
.brand { display: flex; gap: 10px; align-items: center; font-size: 18px; font-weight: 650; flex: 1; align-self: stretch; }
.brand-icon { color: #a5d5e3; font-size: 27px; }.page-name { color: #94a2b1; font-size: 13px; font-weight: 400; margin-left: 4px; padding-left: 15px; border-left: 1px solid #ffffff20; }
.header-actions { display: flex; gap: 10px; align-items: center; }.save-status { color: #9baab7; font-size: 12px; }
button, input, select { font: inherit; }button { cursor: pointer; }button:disabled { cursor: default; opacity: .5; }button:focus-visible, input:focus-visible, select:focus-visible { outline: 2px solid #9acddd; outline-offset: 4px; }
.close-button { background: none; border: 0; color: #bac6d0; font-size: 25px; width: 30px; height: 32px; border-radius: 6px; }.close-button:hover { background: #ffffff0d; }
.body { overflow-y: auto; flex: 1; padding: 25px 26px 16px; }.intro { margin-bottom: 23px; }h1 { font-size: 24px; letter-spacing: .4px; font-weight: 600; margin: 0 0 8px; }p { margin: 0; line-height: 1.65; }.intro p { color: #99a8b6; font-size: 14px; }
.settings-fields { border: 0; padding: 0; margin: 0; min-width: 0; }.main-grid { display: grid; grid-template-columns: 1.15fr 1fr; gap: 18px; align-items: stretch; }.card { background: #1c242e; border: 1px solid #ffffff0d; border-radius: 14px; padding: 19px; }h2 { font-size: 15px; font-weight: 600; margin: 0; }.section-heading { display: flex; align-items: center; justify-content: space-between; gap: 8px; }.eyebrow { font-size: 11px; color: #97a8b8; letter-spacing: .3px; }.description { font-size: 12px; color: #a4b2bf; }
.interval-value { display: flex; gap: 7px; align-items: baseline; margin: 16px 0 9px; color: #b7c3ce; font-size: 13px; }.interval-value strong { font-size: 33px; line-height: 1.1; font-weight: 550; color: #d4edf3; font-variant-numeric: tabular-nums; }input[type=range] { width: 100%; accent-color: #a4d1de; margin: 7px 0; }.range-labels { display: flex; justify-content: space-between; color: #91a1b0; font-size: 11px; margin-bottom: 13px; }.test-note { color: #d9c69a; font-size: 12px; }
.mode-heading { margin: 21px 1px 12px; }.mode-options { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }.mode-card { position: relative; display: flex; flex-direction: column; padding: 9px; border: 1px solid #ffffff17; border-radius: 12px; cursor: pointer; background: #19212b; }.mode-card.selected { border-color: #a4d1de; background: #22323e; }.mode-card:has(input:focus-visible) { outline: 2px solid #a4d1de; outline-offset: 3px; }.mode-card input { position: absolute; top: 17px; right: 16px; accent-color: #a4d1de; width: 14px; height: 14px; margin: 0; }.thumbnail { height: 83px; background: #101720; border: 1px dashed #7c94a938; border-radius: 7px; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 7px; color: #8296a8; font-size: 10px; }.thumbnail svg { width: 27px; height: 22px; stroke: #73899b; stroke-width: 1; }.mode-name { font-size: 14px; font-weight: 550; margin: 11px 2px 4px; }.mode-summary { color: #a5b3c0; font-size: 11px; margin: 0 2px 3px; }
.effect-title { font-size: 22px; margin-top: 10px; }.effect-description { color: #b4c2cf; font-size: 13px; margin: 12px 0 18px; }.preview-button { width: 100%; display: flex; align-items: center; justify-content: space-between; padding: 11px 15px; background: #b9dfeb; color: #182e39; border: none; border-radius: 9px; font-weight: 600; }.preview-button:hover { background: #c9e9f2; }.preview-note { font-size: 11px; color: #93a4b4; text-align: center; margin-top: 8px; }.effect-options { margin-top: 18px; border-top: 1px solid #ffffff0e; padding-top: 4px; }.switch-row { display: flex; gap: 10px; align-items: center; justify-content: space-between; padding: 13px 0; }.switch-row strong { display: block; font-size: 13px; font-weight: 500; }.switch-row small { display: block; font-size: 11px; color: #a4b2bf; margin-top: 5px; line-height: 1.6; }.switch-row input { appearance: none; flex-shrink: 0; width: 34px; height: 20px; background: #526170; border-radius: 20px; position: relative; cursor: pointer; margin: 0; }.switch-row input:before { content: ''; position: absolute; width: 14px; height: 14px; left: 3px; top: 3px; background: #dce5ed; border-radius: 50%; transition: transform .15s; }.switch-row input:checked { background: #94c4d4; }.switch-row input:checked:before { transform: translateX(14px); background: #172b36; }.permission { font-size: 11px; color: #a7c9bb; line-height: 1.6; margin-bottom: 10px; }.permission.missing { color: #e0c395; }.permission button { display: block; margin-top: 3px; }.modifier-row { display: flex; align-items: center; justify-content: space-between; gap: 8px; font-size: 12px; color: #b7c4cf; margin: 10px 0 4px; }select { color: #d3dfe8; background: #151d26; border: 1px solid #ffffff25; border-radius: 6px; padding: 5px 7px; font-size: 12px; max-width: 125px; }.soft-note { color: #a3b7c6; display: flex; align-items: center; gap: 12px; border-top: 1px solid #ffffff10; margin-top: 24px; padding-top: 24px; font-size: 12px; }.soft-note>span { font-size: 32px; color: #85adb9; }
.bottom-grid { display: grid; grid-template-columns: 1.15fr 1fr; gap: 18px; margin-top: 18px; }.pause-description { margin: 10px 0 12px; }.status-dot { height: 7px; width: 7px; border-radius: 50%; background: #9bc9b8; }.status-dot.paused { background: #ddc28d; }.pause-actions { display: flex; gap: 7px; }.secondary-button { font-size: 12px; border: 1px solid #ffffff20; color: #c5d2de; background: #ffffff04; border-radius: 7px; padding: 7px 12px; }.secondary-button:hover { background: #ffffff0c; }.general-description { margin: 12px 0 6px; }.startup { padding-bottom: 7px; }.notice { font-size: 12px; color: #c4d4df; background: #ffffff06; border-radius: 7px; padding: 10px; margin: 10px 0; }.error { color: #f1b2a8; }.text-button { padding: 0; border: 0; background: none; color: #b9dfeb; font-size: inherit; text-decoration: underline; text-underline-offset: 3px; }footer { display: flex; justify-content: space-between; color: #8999a7; font-size: 11px; margin-top: 20px; }
@media (max-width: 600px) { .main-grid, .bottom-grid { grid-template-columns: 1fr; }.body { padding: 20px; }.save-status { max-width: 110px; }.eyebrow { font-size: 11px; } }
</style>
