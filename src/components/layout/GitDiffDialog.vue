<script setup lang="ts">
import { computed } from 'vue';

const props = defineProps<{
  open: boolean;
  path: string;
  loading: boolean;
  diffText: string;
  files: string[];
  currentIndex: number;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'prev'): void;
  (e: 'next'): void;
}>();

type DiffRow = {
  kind: 'meta' | 'ctx' | 'add' | 'del';
  oldNum: number | null;
  newNum: number | null;
  left: string;
  right: string;
  text?: string;
};

const rows = computed<DiffRow[]>(() => {
  if (!props.diffText) return [];
  const out: DiffRow[] = [];
  let oldNum = 0;
  let newNum = 0;
  for (const line of props.diffText.split('\n')) {
    if (line.startsWith('@@')) {
      const m = line.match(/^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@/);
      if (m) {
        oldNum = Number(m[1] || 0);
        newNum = Number(m[2] || 0);
      }
      out.push({ kind: 'meta', oldNum: null, newNum: null, left: '', right: '', text: line });
      continue;
    }
    if (line.startsWith('diff --git') || line.startsWith('index ') || line.startsWith('--- ') || line.startsWith('+++ ') || line.startsWith('\\ No newline')) {
      out.push({ kind: 'meta', oldNum: null, newNum: null, left: '', right: '', text: line });
      continue;
    }
    if (line.startsWith('+')) {
      out.push({ kind: 'add', oldNum: null, newNum, left: '', right: line.slice(1) });
      newNum += 1;
      continue;
    }
    if (line.startsWith('-')) {
      out.push({ kind: 'del', oldNum, newNum: null, left: line.slice(1), right: '' });
      oldNum += 1;
      continue;
    }
    out.push({ kind: 'ctx', oldNum, newNum, left: line.startsWith(' ') ? line.slice(1) : line, right: line.startsWith(' ') ? line.slice(1) : line });
    oldNum += 1;
    newNum += 1;
  }
  return out;
});

const hasPrev = computed(() => props.currentIndex > 0);
const hasNext = computed(() => props.currentIndex >= 0 && props.currentIndex < props.files.length - 1);
</script>

<template>
  <div v-if="open" class="diff-mask" @click.self="emit('close')">
    <section class="diff-dialog">
      <header class="diff-header">
        <div class="title">Diff Preview</div>
        <div class="nav">
          <button class="nav-btn" :disabled="!hasPrev" @click="emit('prev')">Prev</button>
          <button class="nav-btn" :disabled="!hasNext" @click="emit('next')">Next</button>
        </div>
        <div class="file" :title="path">{{ path || 'No file selected' }}</div>
        <button class="close-btn" @click="emit('close')">x</button>
      </header>

      <div class="diff-body ide-scrollbar">
        <div v-if="loading" class="empty">Loading diff...</div>
        <template v-else-if="rows.length">
          <div class="grid-wrap">
            <div class="grid">
            <div class="header left-h">Old</div>
            <div class="header right-h">New</div>
            <template v-for="(r, idx) in rows" :key="`${idx}-${r.kind}`">
              <div v-if="r.kind === 'meta'" class="meta-row">{{ r.text }}</div>
              <template v-else>
                <div class="side left" :class="r.kind">
                  <span class="ln">{{ r.oldNum ?? '' }}</span>
                  <span class="txt">{{ r.left }}</span>
                </div>
                <div class="side right" :class="r.kind">
                  <span class="ln">{{ r.newNum ?? '' }}</span>
                  <span class="txt">{{ r.right }}</span>
                </div>
              </template>
            </template>
            </div>
          </div>
        </template>
        <div v-else class="empty">No diff content available.</div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.diff-mask {
  position: fixed;
  inset: 0;
  z-index: 140;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
}
.diff-dialog {
  width: min(1020px, calc(100vw - 24px));
  height: min(78vh, 860px);
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  border-radius: 0;
  box-shadow: 0 16px 42px rgba(0, 0, 0, 0.38);
  display: flex;
  flex-direction: column;
  color: var(--ide-text);
}
.diff-header {
  flex-shrink: 0;
  height: 42px;
  border-bottom: 1px solid var(--ide-border);
  display: grid;
  grid-template-columns: auto auto minmax(0, 1fr) auto;
  align-items: center;
  gap: 10px;
  padding: 0 12px;
}
.title {
  color: var(--ide-text);
  font-size: 12px;
  font-weight: 600;
}
.file {
  color: var(--ide-text-muted);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.nav { display: flex; gap: 6px; }
.nav-btn {
  height: 24px;
  padding: 0 10px;
  border-radius: 0;
  border: 1px solid var(--ide-border);
  color: var(--ide-text);
  background: transparent;
  cursor: pointer;
  font-size: 11px;
}
.nav-btn:hover:not(:disabled) { background: var(--ide-hover); }
.nav-btn:disabled { opacity: 0.45; cursor: not-allowed; }
.close-btn {
  width: 24px;
  height: 24px;
  border-radius: 0;
  color: var(--ide-text-muted);
  cursor: pointer;
}
.close-btn:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}
.diff-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 0;
}
.empty {
  padding: 16px;
  color: var(--ide-text-muted);
  font-size: 12px;
}
/* Constrain grid to viewport width; inner cells wrap long lines */
.grid-wrap {
  min-width: 0;
  width: 100%;
  max-width: 100%;
  box-sizing: border-box;
}
.grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 1fr);
  width: 100%;
  max-width: 100%;
  min-width: 0;
  border: 1px solid var(--ide-border);
  margin: 12px;
  box-sizing: border-box;
}
.header {
  position: sticky;
  top: 0;
  z-index: 2;
  height: 30px;
  display: flex;
  align-items: center;
  padding: 0 10px;
  background: var(--ide-bg-elevated);
  color: var(--ide-text-muted);
  font-size: 11px;
  font-weight: 600;
  border-bottom: 1px solid var(--ide-border);
  box-shadow: 0 1px 0 color-mix(in srgb, var(--ide-bg-main) 88%, transparent);
}
.left-h { border-right: 1px solid var(--ide-border); }
.meta-row {
  grid-column: 1 / -1;
  padding: 5px 10px;
  font-size: 11px;
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  color: color-mix(in srgb, var(--ide-accent) 75%, var(--ide-text-muted));
  background: color-mix(in srgb, var(--ide-bg-elevated) 65%, var(--ide-bg-main));
  border-top: 1px solid var(--ide-border);
  word-break: break-word;
  overflow-wrap: anywhere;
  min-width: 0;
}
.side {
  min-width: 0;
  max-width: 100%;
  display: grid;
  grid-template-columns: 44px minmax(0, 1fr);
  align-items: start;
  border-top: 1px solid color-mix(in srgb, var(--ide-border) 55%, transparent);
}
.side.left { border-right: 1px solid var(--ide-border); }
.ln {
  padding: 3px 6px 3px 8px;
  color: var(--ide-text-muted);
  font-size: 11px;
  text-align: right;
  user-select: none;
  border-right: 1px solid color-mix(in srgb, var(--ide-border) 70%, transparent);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  line-height: 1.5;
}
.txt {
  min-width: 0;
  max-width: 100%;
  padding: 3px 8px;
  box-sizing: border-box;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
  font-size: 12px;
  line-height: 1.5;
  tab-size: 4;
  color: var(--ide-text);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
}
.side.add {
  background: color-mix(in srgb, var(--ide-git-untracked) 14%, transparent);
}
.side.del {
  background: color-mix(in srgb, var(--ide-git-conflict) 12%, transparent);
}
</style>

