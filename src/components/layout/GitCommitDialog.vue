<script setup lang="ts">
type ChangeItem = {
  path: string;
  code: string;
  selected: boolean;
};

const props = defineProps<{
  open: boolean;
  message: string;
  loading: boolean;
  changes: ChangeItem[];
  errorText: string;
  amend: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'update:message', value: string): void;
  (e: 'update:amend', value: boolean): void;
  (e: 'toggle-change', path: string): void;
  (e: 'select-all'): void;
  (e: 'select-none'): void;
  (e: 'open-diff', path: string): void;
  (e: 'refresh'): void;
  (e: 'rollback-selected'): void;
  (e: 'confirm'): void;
  (e: 'confirm-and-push'): void;
}>();
</script>

<template>
  <!-- v-show avoids tearing down the whole overlay on each close (reduces WebView / layout jitter). -->
  <div v-show="open" class="commit-mask" @click.self="emit('close')">
    <section class="commit-dialog">
      <header class="commit-header">
        <h3>Commit Changes</h3>
        <button class="close-btn" @click="emit('close')">x</button>
      </header>

      <div class="commit-body">
        <aside class="changes-pane">
          <div class="changes-toolbar">
            <div class="title">Changes</div>
            <div class="toolbar-actions">
              <button class="tool-btn text" title="Select All" @click="emit('select-all')">All</button>
              <button class="tool-btn text" title="Select None" @click="emit('select-none')">None</button>
              <button class="tool-btn" title="Refresh" @click="emit('refresh')">
                <FontAwesomeIcon :icon="['fas', 'rotate-right']" />
              </button>
              <button class="tool-btn" title="Rollback Selected" @click="emit('rollback-selected')">
                <FontAwesomeIcon :icon="['fas', 'trash']" />
              </button>
            </div>
          </div>
          <div class="changes-list ide-scrollbar">
            <div v-if="changes.length === 0" class="empty">No local changes detected.</div>
            <div v-for="item in changes" :key="item.path" class="change-row">
              <input
                class="change-check"
                type="checkbox"
                :checked="item.selected"
                @click.stop
                @change="emit('toggle-change', item.path)"
              >
              <span class="status">{{ item.code }}</span>
              <button
                class="path-btn"
                type="button"
                :title="item.path"
                @dblclick.prevent.stop="emit('open-diff', item.path)"
              >
                {{ item.path }}
              </button>
            </div>
          </div>
        </aside>

        <section class="commit-pane">
          <label class="label">Commit Message</label>
          <textarea
            class="message"
            :value="message"
            placeholder="Describe your changes..."
            @input="emit('update:message', ($event.target as HTMLTextAreaElement).value)"
          />
          <label class="amend">
            <input type="checkbox" :checked="amend" @change="emit('update:amend', ($event.target as HTMLInputElement).checked)">
            <span>Amend commit</span>
          </label>
          <div v-if="errorText" class="error">{{ errorText }}</div>
        </section>
      </div>

      <footer class="commit-footer">
        <div class="commit-footer-actions">
          <button
            type="button"
            class="btn-commit btn-commit--primary"
            :disabled="loading || !message.trim()"
            @click="emit('confirm')"
          >
            <FontAwesomeIcon v-if="!loading" :icon="['fas', 'check']" class="btn-commit-ico" />
            <span>{{ loading ? 'Committing…' : 'Commit' }}</span>
          </button>
          <button
            type="button"
            class="btn-commit btn-commit--secondary"
            :disabled="loading || !message.trim()"
            @click="emit('confirm-and-push')"
          >
            <FontAwesomeIcon v-if="!loading" :icon="['fas', 'upload']" class="btn-commit-ico" />
            <span>{{ loading ? 'Working…' : 'Commit and Push' }}</span>
          </button>
        </div>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.commit-mask {
  position: fixed;
  inset: 0;
  z-index: 130;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
  isolation: isolate;
  backface-visibility: hidden;
  contain: paint;
}
.commit-dialog {
  width: 860px;
  max-width: calc(100vw - 32px);
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  border-radius: 0;
  box-shadow: 0 16px 42px rgba(0, 0, 0, 0.35);
  display: flex;
  flex-direction: column;
  min-height: min(480px, 88vh);
  max-height: min(90vh, 720px);
}
.commit-header {
  height: 42px;
  flex-shrink: 0;
  padding: 0 12px;
  border-bottom: 1px solid var(--ide-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}
.commit-header h3 {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
  color: var(--ide-text);
}
.close-btn {
  width: 24px;
  height: 24px;
  border-radius: 0;
  color: var(--ide-text-muted);
  cursor: pointer;
}
.close-btn:hover { background: var(--ide-hover); color: var(--ide-text); }
.commit-body {
  display: grid;
  grid-template-columns: 330px minmax(0, 1fr);
  flex: 1 1 0;
  min-height: 0;
  min-width: 0;
  overflow: hidden;
}
.changes-pane {
  border-right: 1px solid var(--ide-border);
  background: var(--ide-bg-main);
  display: flex;
  flex-direction: column;
  min-height: 0;
}
.changes-toolbar {
  height: 34px;
  border-bottom: 1px solid var(--ide-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px 0 10px;
}
.changes-toolbar .title {
  font-size: 12px;
  color: var(--ide-text);
  font-weight: 600;
}
.toolbar-actions { display: flex; gap: 4px; }
.tool-btn {
  width: 24px;
  height: 24px;
  border-radius: 0;
  color: var(--ide-text-muted);
  cursor: pointer;
}
.tool-btn.text {
  width: auto;
  padding: 0 6px;
  font-size: 11px;
}
.tool-btn:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}
.changes-list {
  flex: 1;
  min-height: 0;
  overflow: auto;
}
.change-row {
  display: grid;
  grid-template-columns: 18px 28px minmax(0, 1fr);
  align-items: center;
  gap: 6px;
  height: 28px;
  padding: 0 8px 0 10px;
  border-bottom: 1px solid color-mix(in srgb, var(--ide-border) 45%, transparent);
  cursor: pointer;
}
.change-row:hover { background: var(--ide-hover); }
.change-check { margin: 0; }
.status {
  color: var(--ide-text-muted);
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 11px;
}
.path {
  color: var(--ide-text);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.path-btn {
  min-width: 0;
  width: 100%;
  text-align: left;
  color: var(--ide-text);
  font-size: 12px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
  cursor: default;
}
.commit-pane {
  display: flex;
  flex-direction: column;
  padding: 14px 14px 16px;
  gap: 10px;
  min-height: 0;
}
.label { font-size: 12px; color: var(--ide-text-muted); }
.message {
  flex: 1;
  min-height: 100px;
  max-height: 220px;
  resize: vertical;
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-elevated);
  color: var(--ide-text);
  border-radius: 0;
  padding: 10px 12px;
  font-size: 12px;
  line-height: 1.5;
}
.message:focus {
  outline: 1px solid color-mix(in srgb, var(--ide-accent) 55%, var(--ide-border));
  outline-offset: -1px;
}
.empty {
  padding: 10px;
  color: var(--ide-text-muted);
  font-size: 12px;
}
.error { color: #ff6b6b; font-size: 12px; }
.amend {
  display: inline-flex;
  align-items: center;
  gap: 8px;
  font-size: 12px;
  color: var(--ide-text-muted);
}

.commit-footer {
  flex-shrink: 0;
  border-top: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-elevated) 35%, var(--ide-bg-main));
  padding: 10px 12px;
}

.commit-footer-actions {
  display: flex;
  justify-content: flex-end;
  align-items: center;
  gap: 10px;
}

.btn-commit {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 8px;
  height: 32px;
  padding: 0 18px;
  border-radius: 0;
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.02em;
  cursor: pointer;
  border: 1px solid transparent;
  transition: background 0.12s ease, border-color 0.12s ease, color 0.12s ease, opacity 0.12s ease;
}

.btn-commit-ico {
  font-size: 12px;
  opacity: 0.95;
}

.btn-commit--primary {
  background: var(--ide-accent);
  color: #fff;
  border-color: color-mix(in srgb, var(--ide-accent) 88%, #000 12%);
  min-width: 112px;
}

.btn-commit--primary:hover:not(:disabled) {
  filter: brightness(1.08);
}

.btn-commit--secondary {
  background: transparent;
  color: var(--ide-text);
  border-color: var(--ide-border);
  min-width: 148px;
}

.btn-commit--secondary:hover:not(:disabled) {
  background: var(--ide-hover);
  border-color: color-mix(in srgb, var(--ide-accent) 35%, var(--ide-border));
  color: var(--ide-text);
}

.btn-commit:disabled {
  opacity: 0.48;
  cursor: not-allowed;
  filter: none;
}
</style>

