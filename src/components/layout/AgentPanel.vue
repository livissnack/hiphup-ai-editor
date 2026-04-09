<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { computed, ref } from 'vue';
import { useAgentRun } from '../../agent/agent';
import { lineDiff } from '../../agent/lineDiff';

const props = defineProps<{
  open: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
}>();

const goal = ref('');
const { run, loading, error, start, refresh, stop } = useAgentRun();
const canStart = computed(() => !loading.value && goal.value.trim().length > 0);
const searchLoading = ref(false);
const searchError = ref('');
const candidateMatches = ref<Array<{ path: string; line: number; text: string }>>([]);
const selectedPath = ref('');
const selectedContent = ref('');
const draftPatch = ref('');
const applying = ref(false);
const generateLoading = ref(false);

const normalizedDisk = computed(() => selectedContent.value.replace(/\r\n/g, '\n'));
const normalizedDraft = computed(() => draftPatch.value.replace(/\r\n/g, '\n'));
const hasDraftDiff = computed(
  () =>
    Boolean(selectedPath.value) &&
    normalizedDisk.value !== normalizedDraft.value,
);

const diffPreview = computed(() => {
  if (!hasDraftDiff.value) {
    return { ops: [] as Array<{ kind: string; text: string }>, truncated: false };
  }
  return lineDiff(selectedContent.value, draftPatch.value, 800);
});

type ChatResult = { content: string; model: string; switched: boolean; fallbackFailures?: string[] };
type LoadedAiConfig = { baseUrl: string; apiKey: string; model: string };

function extractFirstCodeFence(text: string): string | null {
  const normalized = text.replace(/\r\n/g, '\n');
  const re = /```(?:[a-zA-Z0-9._+-]+)?\s*\n?([\s\S]*?)```/;
  const m = normalized.match(re);
  return m ? m[1] : null;
}

const startRun = async () => {
  await start(goal.value);
};

const discoverCandidates = async () => {
  if (!goal.value.trim()) return;
  searchLoading.value = true;
  searchError.value = '';
  try {
    const rows = await invoke<Array<{ path: string; line: number; text: string }>>('agent_search_code', {
      query: goal.value,
      path: 'src',
    });
    candidateMatches.value = rows || [];
    if (candidateMatches.value.length) {
      selectedPath.value = candidateMatches.value[0].path;
      await loadSelectedFile();
    }
  } catch (e) {
    searchError.value = String(e);
  } finally {
    searchLoading.value = false;
  }
};

const loadSelectedFile = async () => {
  if (!selectedPath.value) return;
  try {
    const text = await invoke<string>('agent_read_file', { path: selectedPath.value });
    selectedContent.value = text;
    if (!draftPatch.value.trim()) {
      draftPatch.value = text;
    }
  } catch (e) {
    searchError.value = String(e);
  }
};

const resetDraftToOriginal = () => {
  draftPatch.value = selectedContent.value;
};

const generateDraftFromModel = async () => {
  if (!selectedPath.value || !goal.value.trim()) return;
  const working = draftPatch.value.trim() ? draftPatch.value : selectedContent.value;
  if (!working.trim()) {
    searchError.value = 'No file content to edit. Load a file first.';
    return;
  }
  generateLoading.value = true;
  searchError.value = '';
  try {
    const cfg = await invoke<LoadedAiConfig>('load_ai_config');
    if (!cfg.apiKey?.trim()) {
      searchError.value = 'Add an API key in the AI panel before generating a draft.';
      return;
    }
    const userMessage =
      `File path: ${selectedPath.value}\n\nTask:\n${goal.value.trim()}\n\nCurrent file content:\n${working}`;
    const payload = {
      model: (cfg.model || 'gpt-4o-mini').trim(),
      messages: [
        {
          role: 'system',
          content:
            'You are a coding assistant. The user sends a file path, a task, and the full current file text. Reply with only the complete replacement file inside a single markdown fenced code block (``` with an optional language tag). No explanation outside the fence.',
        },
        { role: 'user', content: userMessage },
      ],
      temperature: 0.2,
      stream: false,
    };
    const result = await invoke<ChatResult>('aihub_chat', { payload });
    const fenced = extractFirstCodeFence(result.content);
    draftPatch.value = fenced ?? result.content.trim();
  } catch (e) {
    searchError.value = String(e);
  } finally {
    generateLoading.value = false;
  }
};

const applyDraft = async () => {
  if (!selectedPath.value) return;
  const ok = window.confirm(
    `Overwrite this file with the draft?\n\n${selectedPath.value}`,
  );
  if (!ok) return;
  applying.value = true;
  searchError.value = '';
  try {
    await invoke('agent_write_file', { path: selectedPath.value, content: draftPatch.value });
    selectedContent.value = draftPatch.value;
  } catch (e) {
    searchError.value = String(e);
  } finally {
    applying.value = false;
  }
};
</script>

<template>
  <aside class="tool-window right" :class="{ open: props.open }">
    <div class="tool-window-header">
      <span class="title">Agent</span>
      <div class="actions">
        <button class="action-btn close drawer-close-btn" title="Hide" @click="emit('close')">
          <span class="drawer-hide-icon right">
            <span class="drawer-body"></span>
            <span class="drawer-rail"></span>
            <span class="drawer-arrow"></span>
          </span>
        </button>
      </div>
    </div>
    <section class="tool-window-content agent-body ide-scrollbar">
      <label class="field">
        <span>Goal</span>
        <textarea v-model="goal" rows="3" placeholder="Describe the coding task..." />
      </label>
      <div class="row">
        <button class="mini-btn primary" :disabled="!canStart" @click="startRun">{{ loading ? 'Running...' : 'Run Agent' }}</button>
        <button class="mini-btn" :disabled="!run?.id" @click="refresh">Refresh</button>
        <button class="mini-btn danger" :disabled="!run?.id" @click="stop">Stop</button>
      </div>
      <div class="row">
        <button class="mini-btn" :disabled="!goal.trim() || searchLoading" @click="discoverCandidates">
          {{ searchLoading ? 'Scanning...' : 'Discover Files' }}
        </button>
      </div>
      <div v-if="error" class="error">{{ error }}</div>
      <div v-if="searchError" class="error">{{ searchError }}</div>
      <div v-if="run" class="run">
        <div class="meta">
          <span class="meta-id">Run: {{ run.id }}</span>
          <span class="meta-status" :class="run.status">{{ run.status }}</span>
        </div>
        <div class="steps">
          <div v-for="step in run.steps" :key="`${run.id}-${step.index}`" class="step">
            <div class="step-title">{{ step.index }}. {{ step.title }} <span class="badge">{{ step.status }}</span></div>
            <div class="step-detail">{{ step.detail }}</div>
          </div>
        </div>
        <div v-if="run.summary" class="summary">{{ run.summary }}</div>
      </div>
      <div v-if="candidateMatches.length" class="run">
        <div class="meta">
          <span class="meta-id">Candidate files</span>
          <span class="meta-status done">{{ candidateMatches.length }}</span>
        </div>
        <label class="field">
          <span>Target File</span>
          <select v-model="selectedPath" class="select" @change="loadSelectedFile">
            <option v-for="m in candidateMatches" :key="`${m.path}:${m.line}`" :value="m.path">
              {{ m.path }}:{{ m.line }}
            </option>
          </select>
        </label>
        <label class="field">
          <span>Draft Patch (editable)</span>
          <textarea v-model="draftPatch" rows="10" />
        </label>
        <div v-if="hasDraftDiff" class="diff-preview">
          <div class="diff-head">
            <span>Diff preview</span>
            <span v-if="diffPreview.truncated" class="diff-trunc">First 800 lines / side</span>
          </div>
          <div class="diff-body ide-scrollbar">
            <div
              v-for="(op, idx) in diffPreview.ops"
              :key="idx"
              :class="['diff-line', op.kind]"
            >
              <span class="diff-mark" aria-hidden="true">{{
                op.kind === 'add' ? '+' : op.kind === 'del' ? '−' : ' '
              }}</span>
              <span class="diff-text">{{ op.text }}</span>
            </div>
          </div>
        </div>
        <div v-else-if="selectedPath" class="diff-empty">No changes vs file on disk.</div>
        <div class="row">
          <button class="mini-btn" :disabled="!selectedPath" @click="resetDraftToOriginal">Reset</button>
          <button
            class="mini-btn"
            :disabled="!selectedPath || !goal.trim() || generateLoading"
            @click="generateDraftFromModel"
          >
            {{ generateLoading ? 'Generating...' : 'Generate draft' }}
          </button>
          <button class="mini-btn primary" :disabled="!selectedPath || applying" @click="applyDraft">
            {{ applying ? 'Applying...' : 'Apply To File' }}
          </button>
        </div>
      </div>
    </section>
  </aside>
</template>

<style scoped>
.tool-window {
  width: var(--ide-toolwindow-width);
  display: flex;
  flex-direction: column;
  background-color: var(--ide-bg-main);
  height: 100%;
  box-sizing: border-box;
  z-index: 25;
  opacity: 0;
  transform: translateX(8px);
  transition: opacity 0.16s ease, transform 0.16s ease;
  pointer-events: none;
}
.tool-window.right {
  border-left: 1px solid var(--ide-border);
  box-shadow: -6px 0 16px rgba(0, 0, 0, 0.2);
}
.tool-window.open {
  opacity: 1;
  transform: translateX(0);
  pointer-events: auto;
}
.tool-window-header {
  height: 32px;
  border-bottom: 1px solid var(--ide-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 10px;
}
.title {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--ide-text-muted);
}
.actions {
  display: inline-flex;
  align-items: center;
  gap: 2px;
}
.action-btn {
  width: 22px;
  height: 22px;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--ide-text-muted);
  cursor: pointer;
}
.action-btn:hover { background: var(--ide-hover); color: var(--ide-text); }
.drawer-hide-icon {
  position: relative;
  width: 12px;
  height: 12px;
  display: inline-block;
}
.drawer-hide-icon .drawer-body {
  position: absolute;
  left: 1px;
  top: 1px;
  width: 10px;
  height: 10px;
  border: 1px solid color-mix(in srgb, var(--ide-text-muted) 72%, transparent);
  border-radius: 2px;
}
.drawer-hide-icon .drawer-rail {
  position: absolute;
  top: 1px;
  right: 2px;
  width: 1px;
  height: 10px;
  background: color-mix(in srgb, var(--ide-text-muted) 70%, transparent);
}
.drawer-hide-icon .drawer-arrow {
  position: absolute;
  top: 4px;
  left: 5px;
  width: 4px;
  height: 4px;
  border-top: 1px solid color-mix(in srgb, var(--ide-text-muted) 84%, transparent);
  border-right: 1px solid color-mix(in srgb, var(--ide-text-muted) 84%, transparent);
  transform: rotate(45deg);
}
.tool-window-content { flex: 1; overflow-y: auto; padding: 12px; }
.agent-body { display: grid; gap: 10px; }
.field { display: grid; gap: 6px; }
.field span { font-size: 11px; color: var(--ide-text-muted); }
textarea {
  min-height: 72px;
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-editor);
  color: var(--ide-text);
  padding: 8px;
  font-size: var(--ide-tree-font-size, 12px);
  resize: vertical;
}
.select {
  width: 100%;
  height: 28px;
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-editor);
  color: var(--ide-text);
  font-size: var(--ide-tree-font-size, 12px);
  padding: 0 8px;
}
.row { display: flex; gap: 6px; }
.mini-btn {
  height: 24px;
  padding: 0 10px;
  border-radius: 6px;
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-main);
  color: var(--ide-text-muted);
  cursor: pointer;
  font-size: 11px;
  font-weight: 500;
  transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease, transform 0.08s ease;
}
.mini-btn:hover:not(:disabled) {
  background: var(--ide-hover);
  color: var(--ide-text);
  border-color: color-mix(in srgb, var(--ide-text-muted) 40%, var(--ide-border));
}
.mini-btn:active:not(:disabled) { transform: translateY(0.5px); }
.mini-btn.primary { background: var(--ide-accent); color: #fff; border-color: transparent; }
.mini-btn.danger { color: #ef8a8a; border-color: #9f3b3b; }
.mini-btn:disabled { opacity: 0.55; cursor: not-allowed; }
.error { color: #ef6b6b; font-size: 11px; }
.run {
  border: 1px solid var(--ide-border);
  border-radius: 8px;
  padding: 8px;
  display: grid;
  gap: 8px;
  background: color-mix(in srgb, var(--ide-bg-elevated) 40%, transparent);
}
.meta {
  font-size: 11px;
  color: var(--ide-text-muted);
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
}
.meta-id {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.meta-status {
  border: 1px solid var(--ide-border);
  border-radius: 999px;
  padding: 1px 8px;
  text-transform: uppercase;
  font-size: 10px;
  letter-spacing: 0.03em;
}
.meta-status.running { color: #8cc6ff; border-color: color-mix(in srgb, #8cc6ff 45%, var(--ide-border)); }
.meta-status.done { color: #84d7a1; border-color: color-mix(in srgb, #84d7a1 45%, var(--ide-border)); }
.meta-status.failed { color: #ef8a8a; border-color: color-mix(in srgb, #ef8a8a 45%, var(--ide-border)); }
.meta-status.stopped { color: #e7ca7f; border-color: color-mix(in srgb, #e7ca7f 45%, var(--ide-border)); }
.step { border: 1px solid color-mix(in srgb, var(--ide-border) 65%, transparent); padding: 6px; }
.step-title { font-size: 12px; color: var(--ide-text); }
.badge { font-size: 10px; color: var(--ide-text-muted); margin-left: 6px; }
.step-detail { font-size: 11px; color: var(--ide-text-muted); margin-top: 4px; }
.summary { font-size: 12px; color: var(--ide-text); }
.diff-preview {
  border: 1px solid var(--ide-border);
  border-radius: 6px;
  overflow: hidden;
  background: var(--ide-bg-editor);
}
.diff-head {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 6px 8px;
  font-size: 11px;
  color: var(--ide-text-muted);
  border-bottom: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-elevated) 55%, transparent);
}
.diff-trunc {
  font-size: 10px;
  opacity: 0.85;
}
.diff-body {
  max-height: 220px;
  overflow: auto;
  font-family: ui-monospace, SFMono-Regular, Menlo, Consolas, monospace;
  font-size: 11px;
  line-height: 1.45;
}
.diff-line {
  display: flex;
  gap: 6px;
  padding: 0 6px;
  white-space: pre-wrap;
  word-break: break-word;
}
.diff-mark {
  flex: 0 0 1em;
  text-align: center;
  opacity: 0.75;
  user-select: none;
}
.diff-text { flex: 1; min-width: 0; }
.diff-line.same { color: color-mix(in srgb, var(--ide-text) 72%, transparent); }
.diff-line.add { background: color-mix(in srgb, #2d6a4f 18%, transparent); color: var(--ide-text); }
.diff-line.del { background: color-mix(in srgb, #9b2226 16%, transparent); color: var(--ide-text); }
.diff-empty {
  font-size: 11px;
  color: var(--ide-text-muted);
  padding: 4px 2px;
}
</style>
