<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from "vue";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { countsFor } from "../lib/view";
import { shortDate } from "../lib/format";
import { t } from "../lib/i18n";
import { initStore, persistSettings, refreshActaData, store } from "../lib/store";
import {
  onHudAnim,
  onDataChanged,
  pickDataFolder,
  readHistory,
  refreshData,
  restoreHistory,
  setHudScale,
  showWindow,
} from "../lib/api";
import AppIcon from "../components/AppIcon.vue";
import LogoWordmark from "../components/LogoWordmark.vue";
import SelectMenu from "../components/SelectMenu.vue";
import type { IconName } from "../types/icons";
import type { HistoryEntry } from "../types/history";

const APP_VERSION = "0.10.0";

type SectionId = "data" | "todoWidget" | "notesWidget" | "hud" | "general" | "about";
const active = ref<SectionId>("data");

const SECTIONS: Array<{ id: SectionId; icon: IconName; label: () => string }> = [
  { id: "data", icon: "database", label: () => t("navData") },
  { id: "todoWidget", icon: "todo", label: () => t("navTodoWidget") },
  { id: "notesWidget", icon: "note", label: () => t("navNotesWidget") },
  { id: "hud", icon: "hud", label: () => t("navHud") },
  { id: "general", icon: "sliders", label: () => t("navGeneral") },
  { id: "about", icon: "info", label: () => t("navAbout") },
];

const platform = navigator.platform.toLowerCase().includes("win") ? "win32" : "darwin";
const win = getCurrentWindow();

const counts = computed(() => countsFor(store.data));
const syncedAt = computed(() => {
  const iso = store.data?.syncedAt;
  if (!iso) return "—";
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "—";
  return `${shortDate(iso)} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
});
const folderShort = computed(() => {
  const folder = store.settings.dataFolder;
  if (!folder) return t("noFolder");
  const parts = folder.split(/[\\/]/).filter(Boolean);
  return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : folder;
});

const INTERVALS = [
  { value: 15, label: () => `15 ${t("seconds")}` },
  { value: 30, label: () => `30 ${t("seconds")}` },
  { value: 60, label: () => `1 ${t("minutes")}` },
  { value: 300, label: () => `5 ${t("minutes")}` },
];

const STEALTH_DELAYS = [
  { value: 5, label: () => `5 ${t("seconds")}` },
  { value: 10, label: () => `10 ${t("seconds")}` },
  { value: 15, label: () => `15 ${t("seconds")}` },
  { value: 30, label: () => `30 ${t("seconds")}` },
  { value: 60, label: () => `1 ${t("minutes")}` },
  { value: 120, label: () => `2 ${t("minutes")}` },
];

const intervalOptions = computed(() => INTERVALS.map((o) => ({ value: o.value, label: o.label() })));
const stealthDelayOptions = computed(() => STEALTH_DELAYS.map((o) => ({ value: o.value, label: o.label() })));

const THEME_OPTIONS = computed(() => [
  { value: "auto", label: t("themeAuto") },
  { value: "light", label: t("themeLight") },
  { value: "dark", label: t("themeDark") },
]);

const HUD_SCALE_MIN = 0.2;
const HUD_SCALE_MAX = 1.5;

/** 滑块拖动中的实时预览：后端立即重排 HUD 窗口，Handy 以脚底为锚变大变小。 */
function previewHudScale(): void {
  void setHudScale(store.settings.hud.scale);
}

/** 贴边过渡动画播放中：Handy 相关设置暂时不可操作。 */
const hudBusy = ref(false);

const LANGUAGE_OPTIONS = [
  { value: "zh", label: "简体中文" },
  { value: "en", label: "English" },
];

// Handy 身体颜色：应用色板预设（与设计令牌同名），auto 跟随主题墨色。
// 字体预设与字号：值与 settings.rs 的白名单 / 缩放范围对应。
const FONT_OPTIONS = computed(() => [
  { value: "system", label: t("fontSystem") },
  { value: "serif", label: t("fontSerif") },
  { value: "kai", label: t("fontKai") },
  { value: "rounded", label: t("fontRounded") },
]);
const FONT_SIZE_OPTIONS = computed(() => [
  { value: 0.9, label: t("fontScaleSmall") },
  { value: 1.0, label: t("fontScaleStandard") },
  { value: 1.1, label: t("fontScaleLarge") },
  { value: 1.25, label: t("fontScaleXL") },
]);

const COLOR_OPTIONS = computed(() => [
  { value: "auto", label: t("colorAuto") },
  { value: "sage", label: t("colorSage") },
  { value: "amber", label: t("colorAmber") },
  { value: "violet", label: t("colorViolet") },
  { value: "danger", label: t("colorClay") },
]);

async function save(): Promise<void> {
  await persistSettings(store.settings);
}

async function chooseFolder(): Promise<void> {
  const picked = await pickDataFolder();
  if (!picked) return;
  store.settings.dataFolder = picked;
  await save();
}

async function reload(): Promise<void> {
  await refreshData();
  await refreshActaData();
}

// --- 修改历史 ----------------------------------------------------------------
// 勾选与编辑都会留档；在这里回看每次写入，必要时恢复到改动前的模样。

const history = ref<HistoryEntry[]>([]);
const restoringId = ref<string | null>(null);
const restoreError = ref("");

async function loadHistory(): Promise<void> {
  const folder = store.settings.dataFolder;
  if (!folder) {
    history.value = [];
    return;
  }
  try {
    history.value = await readHistory(folder);
  } catch {
    history.value = [];
  }
}

async function restore(entry: HistoryEntry): Promise<void> {
  if (restoringId.value || !store.settings.dataFolder) return;
  restoringId.value = entry.id;
  restoreError.value = "";
  try {
    await restoreHistory(store.settings.dataFolder, entry.id);
    await loadHistory();
  } catch (error) {
    restoreError.value = `${t("historyRestoreFail")}：${String(error)}`;
  } finally {
    restoringId.value = null;
  }
}

function historyTime(iso: string): string {
  const date = new Date(iso);
  if (Number.isNaN(date.getTime())) return "";
  return `${shortDate(iso)} ${String(date.getHours()).padStart(2, "0")}:${String(date.getMinutes()).padStart(2, "0")}`;
}

function historyIcon(kind: HistoryEntry["kind"]): IconName {
  return kind.startsWith("todo") ? "todo" : "note";
}

async function minimize(): Promise<void> {
  await win.minimize();
}
async function toggleMaximize(): Promise<void> {
  await win.toggleMaximize();
}
async function closeWindow(): Promise<void> {
  await win.close(); // Rust intercepts close and hides instead.
}

// 最大化状态下，Windows 控件应显示「还原」而非「最大化」。
const maximized = ref(false);
let unlistenResized: (() => void) | null = null;

onMounted(async () => {
  maximized.value = await win.isMaximized().catch(() => false);
  unlistenResized = await win.onResized(async () => {
    maximized.value = await win.isMaximized().catch(() => false);
  });
  void onHudAnim((payload) => {
    hudBusy.value = payload.phase !== "end";
  });
  await initStore();
  await loadHistory();
  // 小组件里的写入也会留档：数据一变就同步历史列表。
  void onDataChanged(() => void loadHistory());
  await new Promise((r) => setTimeout(r, 60));
  await showWindow("main");
});
onBeforeUnmount(() => {
  unlistenResized?.();
});
</script>

<template>
  <div class="shell" :data-platform="platform">
    <div class="titlebar" data-tauri-drag-region>
      <span class="brand"><LogoWordmark :height="28" /></span>
      <span v-if="platform === 'darwin'" class="spacer" />
      <div v-else class="window-controls">
        <button title="最小化" @click="minimize">
          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M2.6 6h6.8" /></svg>
        </button>
        <button :title="maximized ? '还原' : '最大化'" @click="toggleMaximize">
          <svg v-if="!maximized" viewBox="0 0 12 12" aria-hidden="true"><rect x="3" y="3" width="6" height="6" rx="1.3" /></svg>
          <svg v-else viewBox="0 0 12 12" aria-hidden="true">
            <rect x="2.6" y="4.4" width="5" height="5" rx="1.2" />
            <path d="M4.6 2.6h2.9a1.9 1.9 0 0 1 1.9 1.9v2.9" />
          </svg>
        </button>
        <button title="关闭" class="close" @click="closeWindow">
          <svg viewBox="0 0 12 12" aria-hidden="true"><path d="M3.2 3.2l5.6 5.6M8.8 3.2l-5.6 5.6" /></svg>
        </button>
      </div>
    </div>

    <div class="layout">
      <nav class="side-nav">
        <button
          v-for="section in SECTIONS"
          :key="section.id"
          :class="{ active: active === section.id }"
          @click="active = section.id"
        >
          <AppIcon :name="section.icon" :size="15" />
          <span>{{ section.label() }}</span>
          <i />
        </button>
      </nav>

      <main class="content">
        <!-- 数据源 -->
        <section v-if="active === 'data'" class="panel">
          <header>
            <h3>{{ t("navData") }}</h3>
            <p>{{ t("dataFolderDesc") }}</p>
          </header>
          <div class="group">
            <div class="row">
              <span class="row-copy">
                <b>{{ t("dataFolder") }}</b>
                <small class="mono">{{ folderShort }}</small>
              </span>
              <span class="row-actions">
                <button class="settings-button secondary" @click="chooseFolder">
                  <AppIcon name="folder" :size="13" />{{ t("pickFolder") }}
                </button>
                <button class="settings-button" :disabled="!store.settings.dataFolder" @click="reload">
                  <AppIcon name="refresh" :size="13" />{{ t("rescan") }}
                </button>
              </span>
            </div>
            <div class="row">
              <span class="row-copy">
                <b>{{ t("lastSync") }}</b>
                <small>{{ syncedAt }}<template v-if="store.data"> · {{ t("itemCounts")(counts.notes, counts.todos) }}</template></small>
              </span>
            </div>
            <div class="row">
              <span class="row-copy">
                <b>{{ t("refreshInterval") }}</b>
                <small>{{ t("refreshIntervalDesc") }}</small>
              </span>
              <SelectMenu v-model="store.settings.refreshIntervalSecs" :options="intervalOptions" @update:model-value="save" />
            </div>
          </div>
          <p v-if="store.dataError" class="status error">{{ store.dataError }}</p>
          <p v-else-if="store.data && store.data.warnings.length > 0" class="status warn">
            {{ t("dataWarnings")(store.data.warnings.length) }}
          </p>
          <p v-else-if="store.settings.dataFolder && store.data" class="status ok">
            {{ t("itemCounts")(counts.notes, counts.todos) }}
          </p>

          <!-- 修改历史 -->
          <div class="history">
            <div class="history-head">
              <b>{{ t("historyTitle") }}</b>
              <small>{{ t("historyDesc") }}</small>
            </div>
            <p v-if="restoreError" class="status error">{{ restoreError }}</p>
            <div class="group history-list">
              <p v-if="history.length === 0" class="history-empty">{{ t("historyEmpty") }}</p>
              <div v-for="entry in history" :key="entry.id" class="history-row">
                <span class="history-icon"><AppIcon :name="historyIcon(entry.kind)" :size="13" /></span>
                <span class="history-copy">
                  <b>{{ t("historySummary")(entry.kind, entry.title, entry.completed) }}</b>
                  <small>{{ historyTime(entry.time) }}</small>
                </span>
                <button
                  v-if="entry.before"
                  class="settings-button secondary"
                  :disabled="restoringId !== null"
                  @click="restore(entry)"
                >
                  {{ t("historyRestore") }}
                </button>
              </div>
            </div>
          </div>
        </section>

        <!-- 待办小组件 -->
        <section v-else-if="active === 'todoWidget'" class="panel">
          <header>
            <h3>{{ t("navTodoWidget") }}</h3>
            <p>{{ t("widgetHint") }}</p>
          </header>
          <div class="group">
            <div class="row">
              <span class="row-copy"><b>{{ t("enableWidget") }}</b><small>{{ t("enableWidgetDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.todoWidget.enabled" @click="store.settings.todoWidget.enabled = !store.settings.todoWidget.enabled; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("opacity") }}</b><small>{{ t("opacityDesc") }}</small></span>
              <input v-model.number="store.settings.todoWidget.opacity" class="slider" type="range" min="0.3" max="1" step="0.05" @change="save" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("alwaysOnTop") }}</b><small>{{ t("alwaysOnTopDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.todoWidget.alwaysOnTop" @click="store.settings.todoWidget.alwaysOnTop = !store.settings.todoWidget.alwaysOnTop; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("widgetSnapToEdge") }}</b><small>{{ t("widgetSnapToEdgeDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.todoWidget.snapToEdge" @click="store.settings.todoWidget.snapToEdge = !store.settings.todoWidget.snapToEdge; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("showCompleted") }}</b><small>{{ t("showCompletedDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.todoWidget.showCompleted" @click="store.settings.todoWidget.showCompleted = !store.settings.todoWidget.showCompleted; save()" />
            </div>
          </div>
        </section>

        <!-- 笔记小组件 -->
        <section v-else-if="active === 'notesWidget'" class="panel">
          <header>
            <h3>{{ t("navNotesWidget") }}</h3>
            <p>{{ t("widgetHint") }}</p>
          </header>
          <div class="group">
            <div class="row">
              <span class="row-copy"><b>{{ t("enableWidget") }}</b><small>{{ t("enableWidgetDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.notesWidget.enabled" @click="store.settings.notesWidget.enabled = !store.settings.notesWidget.enabled; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("opacity") }}</b><small>{{ t("opacityDesc") }}</small></span>
              <input v-model.number="store.settings.notesWidget.opacity" class="slider" type="range" min="0.3" max="1" step="0.05" @change="save" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("alwaysOnTop") }}</b><small>{{ t("alwaysOnTopDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.notesWidget.alwaysOnTop" @click="store.settings.notesWidget.alwaysOnTop = !store.settings.notesWidget.alwaysOnTop; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("widgetSnapToEdge") }}</b><small>{{ t("widgetSnapToEdgeDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.notesWidget.snapToEdge" @click="store.settings.notesWidget.snapToEdge = !store.settings.notesWidget.snapToEdge; save()" />
            </div>
          </div>
        </section>

        <!-- 悬浮窗 -->
        <section v-else-if="active === 'hud'" class="panel">
          <header>
            <h3>{{ t("navHud") }}</h3>
            <p>{{ t("hudHint") }}</p>
          </header>
          <div class="group" :class="{ busy: hudBusy }">
            <div class="row">
              <span class="row-copy"><b>{{ t("enableWidget") }}</b><small>{{ t("hudEnableDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.hud.enabled" @click="store.settings.hud.enabled = !store.settings.hud.enabled; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("hudSize") }}</b><small>{{ t("hudSizeDesc") }}</small></span>
              <span class="scale-row">
                <input
                  v-model.number="store.settings.hud.scale"
                  class="slider"
                  type="range"
                  :min="HUD_SCALE_MIN"
                  :max="HUD_SCALE_MAX"
                  step="0.05"
                  @input="previewHudScale"
                  @change="save"
                />
                <b class="scale-pct">{{ Math.round(store.settings.hud.scale * 100) }}%</b>
              </span>
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("hudColor") }}</b><small>{{ t("hudColorDesc") }}</small></span>
              <SelectMenu v-model="store.settings.hud.color" :options="COLOR_OPTIONS" @update:model-value="save" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("alwaysOnTop") }}</b><small>{{ t("hudAlwaysOnTopDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.hud.alwaysOnTop" @click="store.settings.hud.alwaysOnTop = !store.settings.hud.alwaysOnTop; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("hudSnapToEdge") }}</b><small>{{ t("hudSnapToEdgeDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.hud.snapToEdge" @click="store.settings.hud.snapToEdge = !store.settings.hud.snapToEdge; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("hudStealth") }}</b><small>{{ t("hudStealthDesc") }}</small></span>
              <button class="switch" role="switch" :aria-checked="store.settings.hud.stealth" @click="store.settings.hud.stealth = !store.settings.hud.stealth; save()" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("hudStealthDelay") }}</b><small>{{ t("hudStealthDelayDesc") }}</small></span>
              <SelectMenu v-model="store.settings.hud.stealthDelaySecs" :options="stealthDelayOptions" @update:model-value="save" />
            </div>
          </div>
        </section>

        <!-- 通用 -->
        <section v-else-if="active === 'general'" class="panel">
          <header>
            <h3>{{ t("navGeneral") }}</h3>
          </header>
          <div class="group">
            <div class="row">
              <span class="row-copy"><b>{{ t("theme") }}</b></span>
              <SelectMenu v-model="store.settings.theme" :options="THEME_OPTIONS" @update:model-value="save" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("language") }}</b></span>
              <SelectMenu v-model="store.settings.language" :options="LANGUAGE_OPTIONS" @update:model-value="save" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("fontFamily") }}</b><small>{{ t("fontFamilyDesc") }}</small></span>
              <SelectMenu v-model="store.settings.font" :options="FONT_OPTIONS" @update:model-value="save" />
            </div>
            <div class="row">
              <span class="row-copy"><b>{{ t("fontSize") }}</b><small>{{ t("fontSizeDesc") }}</small></span>
              <SelectMenu :model-value="store.settings.fontScale" :options="FONT_SIZE_OPTIONS" @update:model-value="(v) => { store.settings.fontScale = Number(v); save(); }" />
            </div>
          </div>
        </section>

        <!-- 关于 -->
        <section v-else class="panel">
          <header>
            <h3>{{ t("navAbout") }}</h3>
          </header>
          <div class="about-mark"><LogoWordmark :height="52" /></div>
          <div class="group">
            <div class="row">
              <span class="row-copy"><b>{{ t("aboutTitle") }}</b><small>{{ t("aboutDesc") }}</small></span>
            </div>
          </div>
          <div class="meta-grid">
            <div><small>{{ t("version") }}</small><b>v{{ APP_VERSION }}</b></div>
            <div><small>{{ t("author") }}</small><b>MogroWang Studio</b></div>
            <div><small>{{ t("license") }}</small><b>MIT</b></div>
            <div><small>{{ t("basedOn") }}</small><b>Acta · 行记</b></div>
          </div>
        </section>
      </main>
    </div>
  </div>
</template>

<style scoped>
.shell {
  height: 100vh;
  display: grid;
  grid-template-rows: 52px minmax(0, 1fr);
  background: var(--sidebar);
}

/* --- titlebar --- */
.titlebar {
  display: flex;
  align-items: center;
  border-bottom: 1px solid rgba(42, 48, 41, .08);
  color: var(--ink);
}
.shell[data-platform="darwin"] .titlebar { padding-left: 84px; }
.shell[data-platform="win32"] .titlebar { padding-left: 18px; }
.brand { display: inline-flex; color: var(--ink); }
.brand :deep(svg) { display: block; }
.spacer { flex: 1; align-self: stretch; }

.window-controls { margin-left: auto; display: flex; align-self: stretch; }
.window-controls button {
  width: 46px;
  border: 0;
  border-radius: 0;
  background: transparent;
  display: grid;
  place-items: center;
  cursor: default;
  transition: background .15s ease, color .15s ease;
}
.window-controls button:hover { background: rgba(42, 48, 41, .09); }
.window-controls button:active { background: rgba(42, 48, 41, .16); transition-duration: .05s; }
.window-controls button.close:hover { background: #c42b1c; color: #fff; }
.window-controls button.close:active { background: #b02517; color: #fff; }
/* 控件字形与 AppIcon 同一线条语言：12 见方、1px 圆头描边。 */
.window-controls svg {
  width: 11px;
  height: 11px;
  fill: none;
  stroke: currentColor;
  stroke-width: 1;
  stroke-linecap: round;
  stroke-linejoin: round;
}

/* --- layout --- */
.layout {
  min-height: 0;
  display: grid;
  grid-template-columns: 210px minmax(0, 1fr);
}
.side-nav {
  padding: 16px 11px;
  border-right: 1px solid var(--line);
  background: var(--sidebar);
  overflow: hidden auto;
  display: flex;
  flex-direction: column;
  gap: 3px;
}
.side-nav button {
  height: 42px;
  padding: 0 11px;
  border: 0;
  border-radius: 10px;
  background: transparent;
  color: var(--muted);
  display: flex;
  align-items: center;
  gap: 10px;
  text-align: left;
  cursor: pointer;
  font-size: 13px;
  transition: background .15s ease, color .15s ease, transform .18s var(--ease-out);
}
.side-nav button:hover { color: var(--ink); background: color-mix(in srgb, var(--white) 55%, transparent); transform: translateX(2px); }
.side-nav button.active { background: var(--white); color: var(--ink); box-shadow: 0 1px 3px rgba(47, 50, 43, .05); }
.side-nav button.active svg { color: var(--sage); }
.side-nav i { margin-left: auto; width: 5px; height: 5px; border-radius: 50%; background: var(--sage); opacity: 0; }
.side-nav button.active i { opacity: 1; }

.content { min-width: 0; overflow: hidden auto; padding: 34px 40px 42px; background: var(--paper); }

.panel { animation: panelIn .35s var(--ease-out) both; }
.panel > header { margin-bottom: 25px; }
.panel > header h3 { margin: 0 0 7px; font: 600 28px/1.2 var(--font-display); letter-spacing: -.02em; }
.panel > header p { margin: 0; color: var(--muted); font-size: 12.5px; line-height: 1.65; max-width: 60ch; }

.group {
  margin: 0 0 22px;
  border: 1px solid var(--line);
  border-radius: 14px;
  background: color-mix(in srgb, var(--white) 40%, transparent);
  transition: opacity .2s ease;
}
/* 贴边过渡动画播放期间，Handy 的设置暂时不可操作。 */
.group.busy { pointer-events: none; opacity: .55; }
.row {
  min-height: 58px;
  padding: 12px 15px;
  display: flex;
  align-items: center;
  gap: 14px;
}
.row + .row { border-top: 1px solid var(--line); }
.row-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 4px; }
.row-copy b { font-size: 13px; }
.row-copy small { color: var(--faint); font-size: 11px; line-height: 1.45; }
.row-actions { display: flex; gap: 8px; flex: 0 0 auto; }

.mono { font-family: ui-monospace, "SFMono-Regular", Consolas, monospace; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

.slider { width: 170px; accent-color: var(--sage); }

/* Handy 大小：无极滑块 + 当前百分比的组合行。 */
.scale-row { display: flex; align-items: center; gap: 10px; }
.scale-row .slider { width: 150px; }
.scale-pct { min-width: 44px; text-align: right; font-size: 12.5px; font-variant-numeric: tabular-nums; }

.status {
  margin: 0 0 18px;
  min-height: 31px;
  padding: 9px 11px;
  border-radius: 9px;
  font-size: 12px;
  line-height: 1.55;
  background: var(--panel);
  color: var(--muted);
  white-space: pre-wrap;
}
.status.ok { color: var(--sage); background: var(--sage-2); }
.status.error { color: var(--priority-high-ink); background: var(--priority-high-bg); }
.status.warn { color: color-mix(in srgb, var(--amber) 74%, var(--ink)); background: color-mix(in srgb, var(--amber-soft) 58%, var(--white)); }

/* --- 修改历史 --- */
.history { margin-top: 22px; }
.history-head { margin: 0 2px 10px; display: flex; align-items: baseline; gap: 10px; }
.history-head b { font-size: 15px; font-weight: 650; letter-spacing: -.01em; }
.history-head small { color: var(--faint); font-size: 11px; }
.history-list { max-height: 268px; overflow: hidden auto; }
.history-empty { margin: 0; padding: 18px; color: var(--faint); font-size: 12px; text-align: center; }
.history-row { padding: 10px 15px; display: flex; align-items: center; gap: 12px; }
.history-row + .history-row { border-top: 1px solid var(--line); }
.history-icon { flex: 0 0 auto; display: inline-flex; color: var(--faint); }
.history-copy { min-width: 0; flex: 1; display: flex; flex-direction: column; gap: 3px; }
.history-copy b { font-size: 12.5px; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.history-copy small { color: var(--faint); font-size: 11px; font-variant-numeric: tabular-nums; }

.about-mark {
  margin-bottom: 18px;
  padding: 18px;
  border: 1px solid var(--line);
  border-radius: 12px;
  background: color-mix(in srgb, var(--white) 40%, transparent);
  display: grid;
  place-items: center;
  color: var(--ink);
}
.meta-grid { display: grid; grid-template-columns: repeat(4, minmax(0, 1fr)); gap: 9px; }
.meta-grid div { padding: 13px; border: 1px solid var(--line); border-radius: 11px; background: color-mix(in srgb, var(--white) 40%, transparent); }
.meta-grid small { display: block; margin-bottom: 4px; color: var(--faint); font-size: 11px; }
.meta-grid b { font-size: 13px; }

@keyframes panelIn {
  0% { opacity: 0; transform: translateY(12px); }
  100% { opacity: 1; transform: translateY(0); }
}
</style>
