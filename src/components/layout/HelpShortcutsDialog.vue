<script setup lang="ts">
import { computed, ref } from 'vue';
import { formatShortcutForDisplay } from '../../utils/shortcutDisplay';

type ShortcutItem = {
  category: string;
  key: string;
  action: string;
  description: string;
};

const props = defineProps<{
  open: boolean;
  shortcuts: ShortcutItem[];
}>();
const emit = defineEmits<{ (e: 'close'): void }>();

const query = ref('');
const filtered = computed(() => {
  const q = query.value.trim().toLowerCase();
  if (!q) return props.shortcuts;
  return props.shortcuts.filter((s) =>
    s.key.toLowerCase().includes(q)
    || s.action.toLowerCase().includes(q)
    || s.description.toLowerCase().includes(q)
    || s.category.toLowerCase().includes(q),
  );
});

const grouped = computed(() => {
  const map = new Map<string, ShortcutItem[]>();
  for (const s of filtered.value) {
    const arr = map.get(s.category) ?? [];
    arr.push(s);
    map.set(s.category, arr);
  }
  return Array.from(map.entries());
});
</script>

<template>
  <div v-if="props.open" class="shortcuts-mask" @click.self="emit('close')">
    <section class="shortcuts-dialog">
      <header class="shortcuts-header">
        <h3>Keyboard Shortcuts</h3>
        <button class="close-btn" type="button" @click="emit('close')">x</button>
      </header>
      <div class="shortcuts-toolbar">
        <input v-model="query" class="search-input" type="text" placeholder="Search shortcut/action/description...">
        <div class="count">{{ filtered.length }} shortcuts</div>
      </div>
      <div class="shortcuts-body ide-scrollbar">
        <template v-if="grouped.length">
          <section v-for="[category, items] in grouped" :key="category" class="group">
            <div class="group-title">{{ category }}</div>
            <div v-for="item in items" :key="`${category}-${item.key}-${item.action}`" class="row">
              <div class="action">{{ item.action }}</div>
              <div class="desc">{{ item.description }}</div>
              <div class="key">{{ formatShortcutForDisplay(item.key) }}</div>
            </div>
          </section>
        </template>
        <div v-else class="empty">No shortcuts match your search.</div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.shortcuts-mask { position: fixed; inset: 0; z-index: 171; background: rgba(0, 0, 0, 0.46); display: flex; align-items: center; justify-content: center; }
.shortcuts-dialog { width: min(980px, calc(100vw - 28px)); height: min(78vh, 820px); background: var(--ide-bg-main); border: 1px solid var(--ide-border); display: flex; flex-direction: column; }
.shortcuts-header { height: 40px; border-bottom: 1px solid var(--ide-border); display: flex; align-items: center; justify-content: space-between; padding: 0 12px; }
.shortcuts-header h3 { margin: 0; font-size: 13px; color: var(--ide-text); }
.close-btn { width: 24px; height: 24px; color: var(--ide-text-muted); cursor: pointer; }
.close-btn:hover { background: var(--ide-hover); color: var(--ide-text); }
.shortcuts-toolbar { height: 38px; border-bottom: 1px solid var(--ide-border); display: flex; align-items: center; gap: 10px; padding: 0 12px; }
.search-input { flex: 1; height: 28px; border: 1px solid var(--ide-border); background: var(--ide-bg-editor); color: var(--ide-text); font-size: 12px; padding: 0 8px; }
.count { font-size: 11px; color: var(--ide-text-muted); }
.shortcuts-body { flex: 1; min-height: 0; overflow: auto; padding: 8px 10px; }
.group { margin-bottom: 10px; border: 1px solid color-mix(in srgb, var(--ide-border) 55%, transparent); }
.group-title { height: 28px; display: flex; align-items: center; padding: 0 10px; font-size: 12px; font-weight: 600; color: var(--ide-text); background: color-mix(in srgb, var(--ide-hover) 70%, transparent); border-bottom: 1px solid color-mix(in srgb, var(--ide-border) 55%, transparent); }
.row { min-height: 30px; display: grid; grid-template-columns: 260px minmax(0, 1fr) 180px; align-items: center; gap: 10px; padding: 0 10px; border-top: 1px solid color-mix(in srgb, var(--ide-border) 40%, transparent); }
.row:first-of-type { border-top: 0; }
.action { font-size: 12px; color: var(--ide-text); }
.desc { font-size: 12px; color: var(--ide-text-muted); }
.key { justify-self: end; font-size: 11px; color: var(--ide-text); border: 1px solid var(--ide-border); padding: 2px 6px; background: var(--ide-bg-editor); font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace; }
.empty { color: var(--ide-text-muted); font-size: 12px; padding: 14px; text-align: center; }
</style>
