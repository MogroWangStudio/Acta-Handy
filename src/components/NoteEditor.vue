<script setup lang="ts">
// 笔记编辑器：标题 + Markdown 正文，输入停顿后自动写回 Acta 数据文件夹。
// 编辑期间通过 beginEdit/endEdit 挂起外部数据刷新，避免输入被打断。
import { onBeforeUnmount, onMounted, ref, watch } from "vue";
import type { ActaNote } from "../types/acta";
import { beginEdit, endEdit, saveNote } from "../lib/store";
import { t } from "../lib/i18n";

const props = defineProps<{ note: ActaNote }>();

const title = ref(props.note.title);
const body = ref(props.note.bodyMarkdown);
const status = ref<"idle" | "dirty" | "saving" | "saved" | "error">("idle");

let timer: ReturnType<typeof setTimeout> | null = null;
let savedFade: ReturnType<typeof setTimeout> | null = null;

function clearTimers(): void {
  if (timer) clearTimeout(timer);
  if (savedFade) clearTimeout(savedFade);
}

function scheduleSave(): void {
  status.value = "dirty";
  if (timer) clearTimeout(timer);
  timer = setTimeout(() => void flush(), 900);
}

async function flush(): Promise<void> {
  if (timer) {
    clearTimeout(timer);
    timer = null;
  }
  if (status.value !== "dirty") return;
  status.value = "saving";
  try {
    await saveNote(props.note.id, title.value, body.value);
    status.value = "saved";
    if (savedFade) clearTimeout(savedFade);
    savedFade = setTimeout(() => {
      if (status.value === "saved") status.value = "idle";
    }, 2200);
  } catch {
    status.value = "error";
  }
}

watch(
  () => props.note.id,
  () => {
    // 切换了笔记：丢弃未保存的草稿，载入新内容。
    clearTimers();
    title.value = props.note.title;
    body.value = props.note.bodyMarkdown;
    status.value = "idle";
  },
);

watch([title, body], () => scheduleSave());

// 编辑期间挂起自动刷新；卸载前把未保存的输入落盘。
onMounted(() => beginEdit());
onBeforeUnmount(() => {
  clearTimers();
  void flush();
  endEdit();
});
</script>

<template>
  <div class="note-editor">
    <header class="editor-head">
      <span class="save-status" :class="[status]">
        <template v-if="status === 'saving' || status === 'dirty'">{{ t("saving") }}</template>
        <template v-else-if="status === 'saved'">{{ t("saved") }}</template>
        <template v-else-if="status === 'error'">{{ t("saveFailed") }}</template>
        <template v-else>{{ t("saved") }}</template>
      </span>
    </header>
    <input v-model="title" class="editor-title" :placeholder="t('noteTitlePlaceholder')" spellcheck="false" />
    <textarea v-model="body" class="editor-body" :placeholder="t('noteBodyPlaceholder')" spellcheck="false" />
  </div>
</template>

<style scoped>
.note-editor {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;
  padding: 6px 16px 12px;
}
.editor-head {
  flex: 0 0 auto;
  display: flex;
  justify-content: flex-end;
  min-height: 16px;
}
.save-status {
  color: var(--faint);
  font-size: 8.5px;
  font-weight: 700;
  letter-spacing: .08em;
  text-transform: uppercase;
  transition: color .2s ease;
}
.save-status.dirty, .save-status.saving { color: var(--amber); }
.save-status.saved { color: var(--sage); }
.save-status.error { color: var(--priority-high-ink); }

.editor-title {
  flex: 0 0 auto;
  margin: 2px 0 6px;
  padding: 4px 0;
  border: 0;
  border-bottom: 1px solid transparent;
  background: transparent;
  color: var(--ink);
  font: 600 15px/1.35 var(--font-display);
  letter-spacing: -.01em;
  outline: none;
  user-select: text;
  -webkit-user-select: text;
}
.editor-title:focus { border-bottom-color: var(--line); }
.editor-title::placeholder { color: var(--faint); }

.editor-body {
  flex: 1;
  min-height: 0;
  padding: 2px 0 8px;
  border: 0;
  background: transparent;
  color: var(--ink);
  font: 400 11.5px/1.7 var(--font-body);
  outline: none;
  resize: none;
  overflow: hidden auto;
  user-select: text;
  -webkit-user-select: text;
}
.editor-body::placeholder { color: var(--faint); }
</style>
