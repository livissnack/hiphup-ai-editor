<script setup lang="ts">
import { nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Terminal } from '@xterm/xterm';
import { FitAddon } from '@xterm/addon-fit';

type TerminalTab = { id: string; title: string };

const props = defineProps<{
  open: boolean;
  treeFontSize: number;
  theme: 'dark' | 'light' | 'monaco';
}>();

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void;
}>();

const containerRef = ref<HTMLElement | null>(null);
const term = ref<Terminal | null>(null);
const fit = ref<FitAddon | null>(null);
const tabs = ref<TerminalTab[]>([]);
const activeId = ref<string | null>(null);
const panelHeight = ref(260);
let unlisten: null | (() => void) = null;

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

const startResize = (event: PointerEvent) => {
  event.preventDefault();
  const startY = event.clientY;
  const startH = panelHeight.value;
  const onMove = (e: PointerEvent) => {
    const deltaY = e.clientY - startY;
    panelHeight.value = clamp(startH - deltaY, 180, 680);
  };
  const onUp = () => {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    localStorage.setItem('ide.terminalPanelHeight', String(panelHeight.value));
    void refreshSize();
  };
  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp, { once: true });
};

const resetViewport = () => {
  if (!term.value) return;
  // reset() clears both viewport and scrollback more reliably than clear()
  // when reusing a single xterm instance across multiple terminal tabs.
  term.value.reset();
};

const applyTerminalTheme = () => {
  if (!term.value) return;
  const style = getComputedStyle(document.documentElement);
  const foreground = style.getPropertyValue('--ide-text').trim() || '#bcbec4';
  const background = style.getPropertyValue('--ide-bg-editor').trim() || 'transparent';
  const cursor = style.getPropertyValue('--ide-text').trim() || '#bcbec4';
  term.value.options.theme = {
    foreground,
    background,
    cursor,
    selectionBackground: 'rgba(53, 116, 240, 0.25)',
  };
};

const ensureTerminal = async () => {
  if (term.value || !containerRef.value) return;

  const t = new Terminal({
    fontFamily: 'JetBrains Mono, Menlo, Monaco, Courier New, monospace',
    fontSize: 12,
    cursorBlink: true,
    scrollback: 5000,
    theme: {
      background: 'transparent',
      foreground: '#bcbec4',
      cursor: '#bcbec4',
      selectionBackground: 'rgba(53, 116, 240, 0.25)',
    },
  });
  const f = new FitAddon();
  t.loadAddon(f);
  t.open(containerRef.value);
  f.fit();
  t.focus();

  t.onData((data) => {
    if (!activeId.value) return;
    void invoke('terminal_write', { id: activeId.value, data });
  });

  t.attachCustomKeyEventHandler((ev) => {
    const isMac = navigator.platform.toLowerCase().includes('mac');
    const ctrlOrCmd = isMac ? ev.metaKey : ev.ctrlKey;

    // Cursor-like terminal copy/paste: Ctrl+Shift+C/V
    if (ev.type === 'keydown' && ev.shiftKey && ev.ctrlKey && (ev.key === 'C' || ev.key === 'c')) {
      const sel = t.getSelection();
      if (sel) void navigator.clipboard.writeText(sel);
      return false;
    }
    if (ev.type === 'keydown' && ev.shiftKey && ev.ctrlKey && (ev.key === 'V' || ev.key === 'v')) {
      void navigator.clipboard.readText().then((text) => {
        if (!text) return;
        if (!activeId.value) return;
        void invoke('terminal_write', { id: activeId.value, data: text });
      });
      return false;
    }

    // Prevent editor/global shortcuts from stealing focus when terminal is active.
    if (ev.type === 'keydown' && ctrlOrCmd && ['f', 's', 'o'].includes(ev.key.toLowerCase())) {
      return false;
    }
    return true;
  });

  term.value = t;
  fit.value = f;
  applyTerminalTheme();
};

const refreshSize = async () => {
  if (!fit.value || !term.value) return;
  fit.value.fit();
  const cols = term.value.cols;
  const rows = term.value.rows;
  if (activeId.value) {
    await invoke('terminal_resize', { id: activeId.value, cols, rows });
  }
};

const createTerminal = async () => {
  const cwd = (window as any).__IDE_WORKSPACE_PATH__ as string | undefined;
  const result = await invoke<{ id: string; title: string }>('terminal_create', { cwd: cwd || null });
  tabs.value = [...tabs.value, result];
  activeId.value = result.id;
  // Single xterm view is reused across tabs; hard reset prevents leftover lines.
  resetViewport();
  await nextTick();
  await ensureTerminal();
  await refreshSize();
};

const closeTerminal = async (id: string) => {
  await invoke('terminal_kill', { id });
  const idx = tabs.value.findIndex((t) => t.id === id);
  tabs.value = tabs.value.filter((t) => t.id !== id);
  if (activeId.value !== id) return;
  const next = tabs.value[idx] ?? tabs.value[idx - 1] ?? null;
  activeId.value = next?.id ?? null;
  if (!activeId.value) {
    term.value?.clear();
  } else {
    await refreshSize();
  }
};

const killActive = async () => {
  if (!activeId.value) return;
  await closeTerminal(activeId.value);
};

const clearActive = async () => {
  // Send Ctrl+L to the active PTY for shell-native clear behavior,
  // then clear xterm buffer to avoid stale scrollback.
  if (activeId.value) {
    try {
      await invoke('terminal_write', { id: activeId.value, data: '\x0c' });
    } catch {
      // ignore and still clear client buffer
    }
  }
  resetViewport();
  term.value?.focus();
};

const focusActive = async () => {
  emit('update:open', true);
  await nextTick();
  await ensureTerminal();
  term.value?.focus();
  await refreshSize();
};

const renameActive = () => {
  if (!activeId.value) return;
  const current = tabs.value.find((t) => t.id === activeId.value)?.title ?? 'Terminal';
  const nextName = window.prompt('Rename terminal', current);
  if (!nextName) return;
  tabs.value = tabs.value.map((t) => (t.id === activeId.value ? { ...t, title: nextName } : t));
};

const activate = async (id: string) => {
  activeId.value = id;
  // Hard reset shared viewport when switching tab to avoid mixed lines.
  resetViewport();
  term.value?.focus();
  await refreshSize();
};

onMounted(async () => {
  const saved = Number(localStorage.getItem('ide.terminalPanelHeight') || 260);
  panelHeight.value = clamp(saved, 180, 680);
  unlisten = await listen<{ id: string; data: string }>('terminal-output', (event) => {
    if (event.payload.id !== activeId.value) return;
    term.value?.write(event.payload.data);
  });

  window.addEventListener('resize', refreshSize);
});

onBeforeUnmount(() => {
  window.removeEventListener('resize', refreshSize);
  unlisten?.();
  unlisten = null;
  term.value?.dispose();
  term.value = null;
});

watch(
  () => props.open,
  async (open) => {
    if (!open) return;
    await nextTick();
    await ensureTerminal();
    if (tabs.value.length === 0) {
      await createTerminal();
      return;
    }
    applyTerminalTheme();
    await refreshSize();
    term.value?.focus();
  },
);

watch(
  () => props.theme,
  () => {
    applyTerminalTheme();
  },
);

defineExpose({
  createTerminal,
  clearActive,
  closeTerminal,
  killActive,
  focusActive,
  renameActive,
});
</script>

<template>
  <section
    class="terminal-panel"
    :hidden="!open"
    :style="{ '--terminal-font-size': `${props.treeFontSize}px`, height: `${panelHeight}px` }"
  >
    <div class="terminal-resizer" title="Resize Terminal panel" @pointerdown="startResize" />
    <header class="ide-panel-header terminal-header">
      <span class="ide-panel-title">
        <FontAwesomeIcon :icon="['fas', 'terminal']" />
        &nbsp;Terminal
      </span>

      <div class="terminal-tabs">
        <div
          v-for="t in tabs"
          :key="t.id"
          class="terminal-tab"
          :class="{ active: t.id === activeId }"
          role="tab"
          tabindex="0"
          @click="activate(t.id)"
          @keydown.enter.prevent="activate(t.id)"
          @keydown.space.prevent="activate(t.id)"
        >
          <span class="name">{{ t.title }}</span>
          <button class="icon-btn" title="Close" @click.stop="closeTerminal(t.id)">
            <FontAwesomeIcon :icon="['fas', 'xmark']" />
          </button>
        </div>
        <button class="terminal-tab add" type="button" title="New Terminal" @click="createTerminal">
          <FontAwesomeIcon :icon="['fas', 'plus']" />
        </button>
      </div>

      <div class="terminal-actions">
        <button class="icon-action" type="button" title="Rename" @click="renameActive">
          <FontAwesomeIcon :icon="['fas', 'pen']" />
        </button>
        <button class="icon-action" type="button" title="Kill" @click="killActive">
          <FontAwesomeIcon :icon="['fas', 'trash']" />
        </button>
        <button class="icon-action" type="button" title="Clear" @click="clearActive">
          <FontAwesomeIcon :icon="['fas', 'broom']" />
        </button>
        <button class="icon-action" type="button" title="Hide" @click="emit('update:open', false)">
          <FontAwesomeIcon :icon="['fas', 'eye-slash']" />
        </button>
      </div>
    </header>

    <div class="terminal-body">
      <div
        v-if="activeId"
        ref="containerRef"
        class="xterm-host"
        @pointerdown="focusActive"
      />
      <div v-else class="terminal-empty">
        <div class="terminal-empty-text">No active terminal</div>
        <button class="terminal-empty-btn" type="button" @click="createTerminal">New Terminal</button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.terminal-panel {
  min-height: 180px;
  background: var(--ide-bg-editor);
  border-top: 1px solid var(--ide-border);
  display: flex;
  flex-direction: column;
  min-height: 0;
  position: relative;
  z-index: 5;
  font-size: var(--terminal-font-size, 12px);
}

.terminal-resizer {
  height: 5px;
  border-bottom: 1px solid color-mix(in srgb, var(--ide-border) 72%, transparent);
  cursor: ns-resize;
  background: transparent;
}

.terminal-resizer:hover {
  background: color-mix(in srgb, var(--ide-accent) 10%, transparent);
}

.terminal-panel[hidden] {
  display: none !important;
}

.terminal-header {
  gap: 12px;
}

.terminal-tabs {
  display: flex;
  align-items: center;
  gap: 6px;
  min-width: 0;
  flex: 1;
  overflow: auto;
}

.terminal-tab {
  height: 24px;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  padding: 0 8px;
  border-radius: 0;
  background: transparent;
  color: var(--ide-text-muted);
  cursor: pointer;
  border: 1px solid transparent;
  flex: 0 0 auto;
  user-select: none;
  font-size: 0.92em;
}

.terminal-tab:focus-visible {
  outline: 2px solid color-mix(in srgb, var(--ide-accent) 55%, transparent);
  outline-offset: 2px;
}

.icon-btn {
  width: 18px;
  height: 18px;
  border-radius: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: inherit;
  cursor: pointer;
}

.icon-btn:hover {
  background: var(--ide-hover);
}

.terminal-tab.active {
  color: var(--ide-text);
  border-color: var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-elevated) 60%, transparent);
}

.terminal-tab:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}

.terminal-tab .name {
  max-width: 160px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.terminal-tab.add {
  font-weight: 700;
}

.terminal-actions {
  display: flex;
  gap: 8px;
}

.icon-action {
  width: 28px;
  height: 28px;
  border-radius: 0;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--ide-text-muted);
  cursor: pointer;
}

.icon-action:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}

.terminal-body {
  flex: 1;
  min-height: 0;
  padding: 8px 10px 10px;
  background: var(--ide-bg-editor);
}

.xterm-host {
  width: 100%;
  height: 100%;
  cursor: text;
}

.terminal-empty {
  width: 100%;
  height: 100%;
  border: 1px dashed color-mix(in srgb, var(--ide-border) 72%, transparent);
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 10px;
  color: var(--ide-text-muted);
}

.terminal-empty-text {
  font-size: 0.95em;
}

.terminal-empty-btn {
  height: 26px;
  padding: 0 10px;
  border: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-elevated) 70%, transparent);
  color: var(--ide-text);
  cursor: pointer;
  border-radius: 0;
  font-size: 0.9em;
}

.terminal-empty-btn:hover {
  background: var(--ide-hover);
}

.xterm-host :deep(.xterm) {
  height: 100%;
  padding: 6px 6px 6px 8px;
}

.xterm-host :deep(.xterm .xterm-viewport) {
  background: transparent;
  scrollbar-width: thin;
  scrollbar-color: var(--ide-scrollbar-thumb) var(--ide-scrollbar-track);
}

.xterm-host :deep(.xterm .xterm-screen) {
  background: transparent;
}

.xterm-host :deep(.xterm textarea) {
  background: transparent;
}

.xterm-host :deep(.xterm .xterm-viewport::-webkit-scrollbar) {
  width: 10px;
  height: 10px;
}

.xterm-host :deep(.xterm .xterm-viewport::-webkit-scrollbar-track) {
  background: var(--ide-scrollbar-track);
}

.xterm-host :deep(.xterm .xterm-viewport::-webkit-scrollbar-thumb) {
  background: var(--ide-scrollbar-thumb);
  border: 2px solid transparent;
  background-clip: padding-box;
  border-radius: 10px;
  min-height: 28px;
}

.xterm-host :deep(.xterm .xterm-viewport::-webkit-scrollbar-thumb:hover) {
  background: var(--ide-scrollbar-thumb-hover);
  border: 2px solid transparent;
  background-clip: padding-box;
}
</style>

