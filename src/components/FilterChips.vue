<script setup lang="ts">
// 归类筛选 chips：「全部 + 各分类（+ 未归类）」，待办与笔记小组件共用。
import { computed } from "vue";
import type { ActaFolder } from "../types/acta";
import { folderColor, folderName } from "../lib/format";
import { t } from "../lib/i18n";

const props = defineProps<{
  folders: ActaFolder[];
  /** 当前筛选：null = 全部；"" = 未归类；否则为文件夹 id。 */
  modelValue: string | null;
  /** folderId 命不中任何分类的条目数，大于 0 时显示「未归类」筛项。 */
  unfiled?: number;
}>();

const emit = defineEmits<{ (e: "update:modelValue", value: string | null): void }>();

const items = computed(() => {
  const list = props.folders.map((f) => ({ id: f.id, name: folderName(f), color: folderColor(f) }));
  if ((props.unfiled ?? 0) > 0) {
    list.push({ id: "", name: folderName(null), color: "#999999" });
  }
  return list;
});
</script>

<template>
  <div class="chips">
    <button class="chip" :class="{ on: modelValue === null }" @click="emit('update:modelValue', null)">
      {{ t("filterAll") }}
    </button>
    <button
      v-for="item in items"
      :key="item.id || '__unfiled__'"
      class="chip"
      :class="{ on: modelValue === item.id }"
      @click="emit('update:modelValue', item.id)"
    >
      <i :style="{ background: item.color }" />{{ item.name }}
    </button>
  </div>
</template>

<style scoped>
.chips {
  display: flex;
  gap: 4px;
  overflow-x: auto;
  padding: 0 16px 8px;
  scrollbar-width: none;
}
.chips::-webkit-scrollbar { display: none; }

.chip {
  flex: 0 0 auto;
  height: 22px;
  padding: 0 9px;
  border: 0;
  border-radius: 999px;
  background: transparent;
  color: var(--muted);
  display: inline-flex;
  align-items: center;
  gap: 5px;
  font-size: 10px;
  font-weight: 600;
  cursor: pointer;
  transition: background .18s ease, color .18s ease;
}
.chip:hover { background: color-mix(in srgb, var(--sage-2) 55%, transparent); }
.chip.on { background: var(--sage-2); color: var(--sage); }
html[data-handy-theme="dark"] .chip.on { color: var(--ink); }
.chip i { width: 6px; height: 6px; border-radius: 50%; flex: 0 0 auto; }
</style>
