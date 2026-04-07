<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import UiSelect from '../ui/UiSelect.vue';

type GitOverview = {
  is_repo: boolean;
  head: null | { branch: string; sha: string };
  locals: Array<{ name: string; sha: string; authored_unix: number; author: string; message: string }>;
  remotes: Array<{ name: string; sha: string; authored_unix: number; author: string; message: string }>;
  tags: Array<{ name: string; sha: string; authored_unix: number; author: string; message: string }>;
  log: Array<{ sha: string; authored_unix: number; rel_date: string; author: string; subject: string; decorations: string }>;
};

const GIT_OVERVIEW_MEMORY_CACHE = new Map<string, GitOverview>();

const props = defineProps<{
  open: boolean;
  workspacePath: string;
  treeFontSize: number;
}>();

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void;
  /** Fired after checkout or other operations that change the working tree / index. */
  (e: 'worktree-changed'): void;
}>();

type Section = 'head' | 'local' | 'remote' | 'tags';
const section = ref<Section>('head');
const loading = ref(false);
const branchSwitching = ref(false);
const errorText = ref<string | null>(null);
const overview = ref<GitOverview | null>(null);
const shaCopiedText = ref('');
const panelHeight = ref(260);
const lastLoadedWorkspace = ref('');
const refMenu = ref<{
  open: boolean;
  x: number;
  y: number;
  kind: 'local' | 'remote' | 'tag';
  name: string;
}>({
  open: false,
  x: 0,
  y: 0,
  kind: 'local',
  name: '',
});

const canLoad = computed(() => props.open && !!props.workspacePath);
const currentBranch = computed(() => overview.value?.head?.branch || '');
const branchOptions = computed(() => {
  const out: Array<{ value: string; label: string; disabled?: boolean }> = [];
  const locals = overview.value?.locals || [];
  const remotes = overview.value?.remotes || [];
  const tags = overview.value?.tags || [];

  if (locals.length) {
    out.push({ value: '__local__', label: 'Local branches', disabled: true });
    for (const b of locals) out.push({ value: `local::${b.name}`, label: b.name });
  }
  if (remotes.length) {
    out.push({ value: '__remote__', label: 'Remote branches', disabled: true });
    for (const b of remotes) out.push({ value: `remote::${b.name}`, label: b.name });
  }
  if (tags.length) {
    out.push({ value: '__tag__', label: 'Tags', disabled: true });
    for (const t of tags) out.push({ value: `tag::${t.name}`, label: t.name });
  }
  if (!out.length && currentBranch.value) {
    out.push({ value: `local::${currentBranch.value}`, label: currentBranch.value });
  }
  return out;
});
const currentHeadRefValue = computed(() => {
  const branch = currentBranch.value;
  if (!branch) return '';
  const locals = overview.value?.locals || [];
  const remotes = overview.value?.remotes || [];
  const tags = overview.value?.tags || [];
  if (locals.some((b) => b.name === branch)) return `local::${branch}`;
  if (remotes.some((b) => b.name === branch)) return `remote::${branch}`;
  if (tags.some((t) => t.name === branch)) return `tag::${branch}`;
  return `local::${branch}`;
});

const formatTimelineTime = (unixTs: number, fallback: string) => {
  if (!Number.isFinite(unixTs) || unixTs <= 0) return fallback || '';
  const d = new Date(unixTs * 1000);
  if (Number.isNaN(d.getTime())) return fallback || '';
  const weekdays = ['星期日', '星期一', '星期二', '星期三', '星期四', '星期五', '星期六'];
  const y = d.getFullYear();
  const m = String(d.getMonth() + 1).padStart(2, '0');
  const day = String(d.getDate()).padStart(2, '0');
  const wd = weekdays[d.getDay()] || '';
  const hour24 = d.getHours();
  const isPm = hour24 >= 12;
  const h12 = hour24 % 12 === 0 ? 12 : hour24 % 12;
  const hh = String(h12).padStart(2, '0');
  const mm = String(d.getMinutes()).padStart(2, '0');
  return `${y}/${m}/${day} ${wd} ${isPm ? '下午' : '上午'} ${hh}:${mm}`;
};

const load = async (opts?: { silent?: boolean }) => {
  if (!props.workspacePath) return;
  const silent = !!opts?.silent;
  if (!silent) loading.value = true;
  errorText.value = null;
  try {
    const result = await invoke<GitOverview>('git_overview', { path: props.workspacePath, limit: 120 });
    overview.value = result;
    GIT_OVERVIEW_MEMORY_CACHE.set(props.workspacePath, result);
    lastLoadedWorkspace.value = props.workspacePath;
  } catch (e) {
    overview.value = null;
    lastLoadedWorkspace.value = '';
    errorText.value = String(e);
  } finally {
    if (!silent) loading.value = false;
  }
};

const setSection = (next: Section) => {
  section.value = next;
};

const clamp = (v: number, min: number, max: number) => Math.min(max, Math.max(min, v));

const startResize = (event: PointerEvent) => {
  event.preventDefault();
  const startY = event.clientY;
  const startH = panelHeight.value;

  const onMove = (e: PointerEvent) => {
    const deltaY = e.clientY - startY;
    // Drag up => higher panel, drag down => lower panel.
    panelHeight.value = clamp(startH - deltaY, 180, 680);
  };
  const onUp = () => {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
    localStorage.setItem('ide.gitPanelHeight', String(panelHeight.value));
  };

  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp, { once: true });
};

const copyHeadSha = async () => {
  const sha = overview.value?.head?.sha || '';
  if (!sha.trim()) return;
  try {
    await navigator.clipboard.writeText(sha);
    shaCopiedText.value = 'SHA copied';
    window.setTimeout(() => {
      shaCopiedText.value = '';
    }, 1200);
  } catch {
    shaCopiedText.value = 'Copy failed';
    window.setTimeout(() => {
      shaCopiedText.value = '';
    }, 1600);
  }
};

const checkoutLocal = async (branch: string) => {
  await invoke<string>('git_exec', {
    path: props.workspacePath,
    args: ['checkout', branch],
  });
};

const checkoutRemote = async (remoteRef: string) => {
  // Try the cleanest "track remote branch" path first.
  try {
    await invoke<string>('git_exec', {
      path: props.workspacePath,
      args: ['checkout', '--track', remoteRef],
    });
    return;
  } catch {
    // Fallback: if local branch exists, checkout by local name.
    const localName = remoteRef.includes('/') ? remoteRef.split('/').slice(1).join('/') : remoteRef;
    await invoke<string>('git_exec', {
      path: props.workspacePath,
      args: ['checkout', localName],
    });
  }
};

const checkoutTag = async (tagName: string) => {
  await invoke<string>('git_exec', {
    path: props.workspacePath,
    args: ['checkout', `tags/${tagName}`],
  });
};

const checkoutRef = async (kind: 'local' | 'remote' | 'tag', name: string) => {
  if (!name || !props.workspacePath) return;
  if (kind === 'local' && name === currentBranch.value) return;
  branchSwitching.value = true;
  errorText.value = null;
  try {
    if (kind === 'local') await checkoutLocal(name);
    else if (kind === 'remote') await checkoutRemote(name);
    else await checkoutTag(name);
    await load();
    section.value = 'head';
    emit('worktree-changed');
  } catch (e) {
    const typeLabel = kind === 'local' ? 'branch' : kind === 'remote' ? 'remote branch' : 'tag';
    errorText.value = `Checkout ${typeLabel} failed: ${String(e)}`;
  } finally {
    branchSwitching.value = false;
  }
};

const checkoutFromHeadSelect = async (value: string) => {
  const [kindRaw, ...rest] = String(value || '').split('::');
  const name = rest.join('::');
  if (!name) return;
  const kind = kindRaw === 'remote' || kindRaw === 'tag' ? kindRaw : 'local';
  await checkoutRef(kind, name);
};

const openRefMenu = (event: MouseEvent, kind: 'local' | 'remote' | 'tag', name: string) => {
  event.preventDefault();
  refMenu.value = {
    open: true,
    x: event.clientX,
    y: event.clientY,
    kind,
    name,
  };
};

const closeRefMenu = () => {
  refMenu.value.open = false;
};

const onGlobalPointerDown = (event: PointerEvent) => {
  const target = event.target as HTMLElement | null;
  if (!target) return;
  if (target.closest('.ref-context-menu')) return;
  closeRefMenu();
};

const onGlobalKeydown = (e: KeyboardEvent) => {
  if (e.key === 'Escape') closeRefMenu();
};

const triggerRefMenuCheckout = async () => {
  if (!refMenu.value.open || !refMenu.value.name) return;
  const { kind, name } = refMenu.value;
  closeRefMenu();
  await checkoutRef(kind, name);
};

onMounted(() => {
  const saved = Number(localStorage.getItem('ide.gitPanelHeight') || 260);
  panelHeight.value = clamp(saved, 180, 680);
  if (props.workspacePath && !overview.value) {
    const cached = GIT_OVERVIEW_MEMORY_CACHE.get(props.workspacePath);
    if (cached) {
      overview.value = cached;
      lastLoadedWorkspace.value = props.workspacePath;
    }
  }
  if (canLoad.value) void load();
  window.addEventListener('pointerdown', onGlobalPointerDown);
  window.addEventListener('keydown', onGlobalKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener('pointerdown', onGlobalPointerDown);
  window.removeEventListener('keydown', onGlobalKeydown);
});

watch(
  () => [props.open, props.workspacePath] as const,
  ([open, workspace]) => {
    if (!open) return;
    if (!workspace) return;
    if (!overview.value) {
      const cached = GIT_OVERVIEW_MEMORY_CACHE.get(workspace);
      if (cached) {
        overview.value = cached;
        lastLoadedWorkspace.value = workspace;
      }
    }
    const workspaceChanged = workspace !== lastLoadedWorkspace.value;
    if (workspaceChanged) {
      const cached = GIT_OVERVIEW_MEMORY_CACHE.get(workspace);
      if (cached) {
        overview.value = cached;
        lastLoadedWorkspace.value = workspace;
        void load({ silent: true });
      } else {
        overview.value = null;
        void load({ silent: false });
      }
      return;
    }
    // Keep existing data visible while refreshing in background.
    void load({ silent: !!overview.value });
  },
);

watch(
  () => [section.value, props.open] as const,
  () => {
    closeRefMenu();
  },
);

defineExpose({
  refresh: load,
  setSection,
});
</script>

<template>
  <section
    class="git-panel"
    :hidden="!open"
    :style="{ '--git-font-size': `${props.treeFontSize}px`, height: `${panelHeight}px` }"
  >
    <div class="git-resizer" title="Resize Git panel" @pointerdown="startResize" />
    <header class="ide-panel-header git-header">
      <span class="ide-panel-title">
        <FontAwesomeIcon :icon="['fas', 'code-branch']" />
        &nbsp;Git
      </span>

      <div class="git-tabs" role="tablist" aria-label="Git sections">
        <button class="git-tab" :class="{ active: section === 'head' }" type="button" @click="section = 'head'">
          <FontAwesomeIcon :icon="['far', 'clock']" />
          <span>Head</span>
        </button>
        <button class="git-tab" :class="{ active: section === 'local' }" type="button" @click="section = 'local'">
          <FontAwesomeIcon :icon="['fas', 'code-branch']" />
          <span>Local</span>
        </button>
        <button class="git-tab" :class="{ active: section === 'remote' }" type="button" @click="section = 'remote'">
          <FontAwesomeIcon :icon="['fas', 'cloud']" />
          <span>Remote</span>
        </button>
        <button class="git-tab" :class="{ active: section === 'tags' }" type="button" @click="section = 'tags'">
          <FontAwesomeIcon :icon="['fas', 'tags']" />
          <span>Tags</span>
        </button>
      </div>

      <div class="git-actions">
        <button class="icon-action" type="button" title="Refresh" @click="load" :disabled="loading || !workspacePath">
          <FontAwesomeIcon :icon="['fas', 'rotate-right']" />
        </button>
        <button class="icon-action" type="button" title="Hide" @click="emit('update:open', false)">
          <FontAwesomeIcon :icon="['fas', 'eye-slash']" />
        </button>
      </div>
    </header>

    <div class="git-body ide-scrollbar">
      <div v-if="!workspacePath" class="hint">Open a folder to view Git information.</div>
      <div v-else-if="loading" class="hint">Loading…</div>
      <div v-else-if="errorText" class="hint error">{{ errorText }}</div>
      <template v-else-if="overview && !overview.is_repo">
        <div class="hint">This folder is not a Git repository.</div>
      </template>
      <template v-else-if="overview && overview.is_repo">
        <div class="topline">
          <button
            class="pill pill-clickable"
            type="button"
            :title="overview.head?.sha ? `Click to copy SHA: ${overview.head?.sha}` : 'No HEAD SHA'"
            @click="copyHeadSha"
          >
            <FontAwesomeIcon :icon="['fas', 'code-commit']" />
            <span>{{ overview.head?.sha.slice(0, 8) }}</span>
            <span v-if="shaCopiedText" class="pill-hint">{{ shaCopiedText }}</span>
          </button>
          <div class="branch-switch">
            <span class="branch-icon">
              <FontAwesomeIcon :icon="['fas', 'code-branch']" />
            </span>
            <UiSelect
              :model-value="currentHeadRefValue"
              :options="branchOptions"
              :disabled="branchSwitching || loading"
              placeholder="Select branch"
              @update:model-value="checkoutFromHeadSelect(String($event))"
            />
          </div>
        </div>

        <div class="grid">
          <div class="left">
            <div v-if="section === 'head'">
              <div class="section-title">Commit Log</div>
              <div v-for="c in overview.log" :key="c.sha" class="commit">
                <div class="timeline-dot" aria-hidden="true" />
                <div class="row1">
                  <span class="sha">{{ c.sha }}</span>
                  <span class="muted time">{{ formatTimelineTime(c.authored_unix, c.rel_date) }}</span>
                  <span class="author">{{ c.author }}</span>
                </div>
                <div class="row2">
                  <span class="subject">{{ c.subject }}</span>
                  <span v-if="c.decorations" class="decorations">{{ c.decorations }}</span>
                </div>
              </div>
            </div>

            <div v-else-if="section === 'local'">
              <div class="section-title">
                <FontAwesomeIcon :icon="['fas', 'code-branch']" />
                &nbsp;Local branches
              </div>
              <div
                v-for="r in overview.locals"
                :key="r.name"
                class="ref"
                @contextmenu="openRefMenu($event, 'local', r.name)"
              >
                <div class="name">{{ r.name }}</div>
                <div class="meta">
                  <span class="sha">{{ r.sha }}</span>
                  <span class="muted time">{{ formatTimelineTime(r.authored_unix, '') }}</span>
                  <span class="author">{{ r.author }}</span>
                  <span class="muted">{{ r.message }}</span>
                </div>
              </div>
            </div>

            <div v-else-if="section === 'remote'">
              <div class="section-title">
                <FontAwesomeIcon :icon="['fas', 'cloud']" />
                &nbsp;Remote branches
              </div>
              <div
                v-for="r in overview.remotes"
                :key="r.name"
                class="ref"
                @contextmenu="openRefMenu($event, 'remote', r.name)"
              >
                <div class="name">{{ r.name }}</div>
                <div class="meta">
                  <span class="sha">{{ r.sha }}</span>
                  <span class="muted time">{{ formatTimelineTime(r.authored_unix, '') }}</span>
                  <span class="author">{{ r.author }}</span>
                  <span class="muted">{{ r.message }}</span>
                </div>
              </div>
            </div>

            <div v-else>
              <div class="section-title">
                <FontAwesomeIcon :icon="['fas', 'tags']" />
                &nbsp;Tags
              </div>
              <div
                v-for="r in overview.tags"
                :key="r.name"
                class="ref"
                @contextmenu="openRefMenu($event, 'tag', r.name)"
              >
                <div class="name">{{ r.name }}</div>
                <div class="meta">
                  <span class="sha">{{ r.sha }}</span>
                  <span class="muted time">{{ formatTimelineTime(r.authored_unix, '') }}</span>
                  <span class="author">{{ r.author }}</span>
                  <span class="muted">{{ r.message }}</span>
                </div>
              </div>
            </div>
          </div>

          <div class="right">
            <div class="section-title">Info</div>
            <div class="kv">
              <div class="k">Workspace</div>
              <div class="v">{{ workspacePath }}</div>
            </div>
            <div class="kv">
              <div class="k">Head</div>
              <div class="v">{{ overview.head?.branch }} @ {{ overview.head?.sha.slice(0, 8) }}</div>
            </div>
            <div class="kv">
              <div class="k">Local</div>
              <div class="v">{{ overview.locals.length }}</div>
            </div>
            <div class="kv">
              <div class="k">Remote</div>
              <div class="v">{{ overview.remotes.length }}</div>
            </div>
            <div class="kv">
              <div class="k">Tags</div>
              <div class="v">{{ overview.tags.length }}</div>
            </div>
          </div>
        </div>
        <div
          v-if="refMenu.open"
          class="ref-context-menu"
          :style="{ left: `${refMenu.x}px`, top: `${refMenu.y}px` }"
        >
          <button class="ref-context-item" :disabled="branchSwitching" @click="triggerRefMenuCheckout">
            {{ refMenu.kind === 'remote' ? 'Track Remote Branch' : 'Checkout' }}
          </button>
        </div>
      </template>
    </div>
  </section>
</template>

<style scoped>
.git-panel {
  min-height: 180px;
  background: var(--ide-bg-editor);
  border-top: 1px solid var(--ide-border);
  display: flex;
  flex-direction: column;
  min-height: 0;
  position: relative;
  z-index: 5;
  font-size: var(--git-font-size, 12px);
}

.git-resizer {
  height: 5px;
  border-bottom: 1px solid color-mix(in srgb, var(--ide-border) 72%, transparent);
  cursor: ns-resize;
  background: transparent;
}
.git-resizer:hover {
  background: color-mix(in srgb, var(--ide-accent) 10%, transparent);
}

.git-panel[hidden] {
  display: none !important;
}

.git-header {
  gap: 12px;
}

.git-tabs {
  display: flex;
  gap: 6px;
  flex: 1;
  min-width: 0;
}

.git-tab {
  height: 24px;
  padding: 0 10px;
  border-radius: 0;
  background: transparent;
  color: var(--ide-text-muted);
  cursor: pointer;
  border: 1px solid transparent;
  user-select: none;
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 0.92em;
}

.git-tab:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}

.git-tab.active {
  color: var(--ide-text);
  border-color: var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-elevated) 60%, transparent);
}

.git-actions {
  display: flex;
  gap: 8px;
}

.git-body {
  flex: 1;
  min-height: 0;
  overflow: auto;
  padding: 10px;
  background: var(--ide-bg-editor);
}

.hint {
  color: var(--ide-text-muted);
  font-size: 0.95em;
  padding: 10px 2px;
}
.hint.error {
  color: color-mix(in srgb, #ff6b6b 70%, var(--ide-text));
}

.topline {
  display: flex;
  gap: 10px;
  align-items: center;
  margin-bottom: 10px;
}

.pill {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  height: 22px;
  padding: 0 10px;
  border: 1px solid var(--ide-border);
  border-radius: 0;
  background: color-mix(in srgb, var(--ide-bg-elevated) 55%, transparent);
  color: var(--ide-text);
  font-size: 0.9em;
}
.pill-clickable {
  cursor: pointer;
}
.pill-clickable:hover {
  background: var(--ide-hover);
}
.pill-hint {
  margin-left: 6px;
  color: var(--ide-text-muted);
  font-size: 0.9em;
}

.branch-switch {
  min-width: 180px;
  max-width: 260px;
  display: flex;
  align-items: center;
  gap: 6px;
}
.branch-icon {
  color: var(--ide-text-muted);
  font-size: 0.95em;
}
.branch-switch :deep(.ui-select) {
  width: 100%;
}
.branch-switch :deep(.ui-select-trigger) {
  height: 22px;
  font-size: 0.9em;
  padding: 0 8px;
}
.branch-switch :deep(.ui-select-popover) {
  min-width: 200px;
  z-index: 650;
}
.branch-switch :deep(.ui-select-option) {
  height: 26px;
}

.grid {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 260px;
  gap: 12px;
  min-height: 0;
}

.left {
  min-width: 0;
}

.right {
  min-width: 0;
  border-left: 1px solid var(--ide-border);
  padding-left: 12px;
}

.section-title {
  font-size: 0.86em;
  font-weight: 700;
  color: var(--ide-text-muted);
  text-transform: uppercase;
  letter-spacing: 0.02em;
  margin: 10px 0 8px;
}

.commit {
  position: relative;
  padding: 8px 10px;
  padding-left: 22px;
  border-radius: 0;
  border: 1px solid color-mix(in srgb, var(--ide-border) 65%, transparent);
  background: color-mix(in srgb, var(--ide-bg-elevated) 55%, transparent);
  margin-bottom: 8px;
}
.commit::before {
  content: '';
  position: absolute;
  left: 10px;
  top: -9px;
  bottom: -9px;
  width: 1px;
  background: color-mix(in srgb, var(--ide-border) 78%, transparent);
}
.commit:first-of-type::before {
  top: 12px;
}
.commit:last-of-type::before {
  bottom: calc(100% - 12px);
}
.timeline-dot {
  position: absolute;
  left: 7px;
  top: 11px;
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: color-mix(in srgb, var(--ide-accent) 72%, var(--ide-text));
}

.commit:hover {
  background: var(--ide-hover);
}

.row1 {
  display: flex;
  gap: 10px;
  align-items: baseline;
  flex-wrap: wrap;
}

.row2 {
  display: flex;
  gap: 10px;
  align-items: baseline;
  margin-top: 4px;
  flex-wrap: wrap;
}

.sha {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
  font-size: 0.9em;
  color: color-mix(in srgb, var(--ide-accent) 70%, var(--ide-text));
}

.subject {
  color: var(--ide-text);
}

.decorations {
  color: var(--ide-text-muted);
  font-size: 0.88em;
}
.time {
  white-space: nowrap;
}
.author {
  color: var(--ide-text);
  font-size: 0.92em;
}

.ref {
  padding: 8px 10px;
  border-radius: 0;
  border: 1px solid color-mix(in srgb, var(--ide-border) 65%, transparent);
  background: color-mix(in srgb, var(--ide-bg-elevated) 55%, transparent);
  margin-bottom: 8px;
}

.ref:hover {
  background: var(--ide-hover);
}

.ref .name {
  color: var(--ide-text);
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  margin-bottom: 4px;
}

.ref .meta {
  display: flex;
  gap: 10px;
  align-items: baseline;
  min-width: 0;
}

.ref-context-menu {
  position: fixed;
  z-index: 860;
  min-width: 180px;
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-main);
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
  padding: 4px;
}
.ref-context-item {
  width: 100%;
  height: 28px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--ide-text);
  text-align: left;
  padding: 0 8px;
  font-size: 12px;
  cursor: pointer;
}
.ref-context-item:hover:not(:disabled) {
  background: var(--ide-hover);
}
.ref-context-item:disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.kv {
  display: grid;
  grid-template-columns: 90px minmax(0, 1fr);
  gap: 8px;
  padding: 6px 0;
}

.k {
  color: var(--ide-text-muted);
  font-size: 0.88em;
}

.v {
  color: var(--ide-text);
  font-size: 0.95em;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>

