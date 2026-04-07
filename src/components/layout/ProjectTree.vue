<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { getTreeShortcut } from '../../config/treeShortcuts';

type TreeNode = {
  name: string;
  path: string;
  isDir: boolean;
  children?: TreeNode[];
};

const props = withDefaults(
  defineProps<{
    rootPath: string;
    nodes: TreeNode[];
    expandedPaths: string[];
    selectedPath: string | null;
    bookmarks?: string[];
    revealPath?: string | null;
    gitEnabled?: boolean;
    gitDecorations?: Record<string, string>;
  }>(),
  { gitEnabled: false, gitDecorations: () => ({}) },
);

const GIT_KIND_PRIORITY: Record<string, number> = {
  conflict: 60,
  mixed: 50,
  staged: 40,
  modified: 30,
  untracked: 20,
  ignored: 10,
};

const dirChildPrefix = (dirPath: string) =>
  dirPath.endsWith('/') || dirPath.endsWith('\\') ? dirPath : `${dirPath}${dirPath.includes('\\') ? '\\' : '/'}`;

const gitRowClass = (node: TreeNode): string | null => {
  if (!props.gitEnabled) return null;
  const dec = props.gitDecorations;
  if (!dec || !Object.keys(dec).length) return null;

  if (!node.isDir) {
    const k = dec[node.path];
    return k ? `git-${k}` : null;
  }

  const prefix = dirChildPrefix(node.path);
  let bestKind: string | null = null;
  let bestPri = 0;
  let anyUnder = false;
  let anyNonIgnored = false;
  for (const [p, kind] of Object.entries(dec)) {
    if (p.length <= prefix.length || !p.startsWith(prefix)) continue;
    anyUnder = true;
    if (kind !== 'ignored') anyNonIgnored = true;
    const pri = GIT_KIND_PRIORITY[kind] ?? 0;
    if (pri > bestPri) {
      bestPri = pri;
      bestKind = kind;
    }
  }

  const direct = dec[node.path];
  const directPri = direct ? GIT_KIND_PRIORITY[direct] ?? 0 : 0;

  if (direct && directPri >= bestPri) {
    return `git-${direct}`;
  }
  if (bestKind && anyNonIgnored) {
    return `git-ancestor-${bestKind}`;
  }
  if (anyUnder && bestKind === 'ignored' && !anyNonIgnored) {
    return 'git-folder-ignored';
  }
  if (direct) {
    return `git-${direct}`;
  }
  return null;
};

const emit = defineEmits<{
  (e: 'toggle-folder', path: string): void;
  (e: 'open-file', path: string): void;
  (e: 'tree-action', payload: { action: string; path: string; isDir: boolean }): void;
}>();

type FlatNode = TreeNode & { depth: number };
const expandedSet = computed(() => new Set(props.expandedPaths));

const flattenTree = (nodes: TreeNode[], depth = 0): FlatNode[] => {
  const flat: FlatNode[] = [];
  for (const node of nodes) {
    flat.push({ ...node, depth });
    if (node.isDir && node.children?.length && expandedSet.value.has(node.path)) {
      flat.push(...flattenTree(node.children, depth + 1));
    }
  }
  return flat;
};

const flatNodes = computed(() => flattenTree(props.nodes));
const bookmarkedSet = computed(() => new Set(props.bookmarks ?? []));
const highlightedPath = ref<string | null>(null);

const contextMenu = ref({
  open: false,
  x: 0,
  y: 0,
  path: '',
  isDir: false,
});
const gitSubmenuOpen = ref(false);

const contextMenuGitKind = computed(() => {
  if (!contextMenu.value.open || contextMenu.value.isDir || !props.gitEnabled) return null;
  return props.gitDecorations[contextMenu.value.path] ?? '';
});

const showGitSubmenu = computed(
  () => props.gitEnabled && !contextMenu.value.isDir && contextMenuGitKind.value !== 'ignored',
);

const gitLeafUntracked = computed(() => contextMenuGitKind.value === 'untracked');

const closeContextMenu = () => {
  contextMenu.value.open = false;
  gitSubmenuOpen.value = false;
};

const onNodeClick = (node: TreeNode) => {
  if (node.isDir) {
    emit('toggle-folder', node.path);
    return;
  }
  emit('open-file', node.path);
};

const isExpanded = (path: string) => expandedSet.value.has(path);
const isBookmarked = (path: string) => bookmarkedSet.value.has(path);

const onNodeContextMenu = (event: MouseEvent, node: TreeNode) => {
  event.preventDefault();
  gitSubmenuOpen.value = false;
  contextMenu.value = {
    open: true,
    x: event.clientX,
    y: event.clientY,
    path: node.path,
    isDir: node.isDir,
  };
};

const onGlobalPointerDown = (event: PointerEvent) => {
  const target = event.target as HTMLElement | null;
  if (!target) return;
  if (target.closest('.tree-context-menu')) return;
  closeContextMenu();
};

const onWindowKeydown = (event: KeyboardEvent) => {
  if (event.key === 'Escape') closeContextMenu();
};

const triggerAction = (action: string) => {
  if (!contextMenu.value.path) return;
  emit('tree-action', {
    action,
    path: contextMenu.value.path,
    isDir: contextMenu.value.isDir,
  });
  closeContextMenu();
};

const revealNode = async (path: string) => {
  await nextTick();
  const safePath = path.replaceAll('\\', '\\\\').replaceAll('"', '\\"');
  const el = document.querySelector(`.project-tree .file-item[data-path="${safePath}"]`) as HTMLElement | null;
  if (!el) return;
  el.scrollIntoView({ block: 'nearest' });
  highlightedPath.value = path;
  window.setTimeout(() => {
    if (highlightedPath.value === path) highlightedPath.value = null;
  }, 1400);
};

watch(
  () => [props.revealPath, flatNodes.value.length] as const,
  async ([path]) => {
    if (!path) return;
    await revealNode(path);
  },
);

onMounted(() => {
  window.addEventListener('pointerdown', onGlobalPointerDown);
  window.addEventListener('keydown', onWindowKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener('pointerdown', onGlobalPointerDown);
  window.removeEventListener('keydown', onWindowKeydown);
});
</script>

<template>
  <div class="project-tree" tabindex="0" @pointerdown="($event.currentTarget as HTMLElement)?.focus()">
    <div class="tree-header">{{ rootPath || 'Project' }}</div>
    <div class="tree-content ide-scrollbar">
      <div
        v-for="node in flatNodes"
        :key="node.path"
        class="file-item"
        :class="[
          { selected: selectedPath === node.path, reveal: highlightedPath === node.path },
          gitRowClass(node),
        ]"
        :data-path="node.path"
        :style="{ paddingLeft: `${12 + node.depth * 14}px` }"
        @click="onNodeClick(node)"
        @contextmenu="onNodeContextMenu($event, node)"
      >
        <span class="expander">
          <template v-if="node.isDir">
            <FontAwesomeIcon
              :icon="isExpanded(node.path) ? ['fas', 'chevron-down'] : ['fas', 'chevron-right']"
              style="font-size: 10px;"
            />
          </template>
        </span>
        <span class="icon">
          <FontAwesomeIcon :icon="node.isDir ? ['fas', 'folder'] : ['fas', 'file']" />
        </span>
        <span class="label">{{ node.name }}</span>
        <span v-if="isBookmarked(node.path)" class="bookmark-dot" title="Bookmarked">★</span>
      </div>
    </div>
    <div
      v-if="contextMenu.open"
      class="tree-context-menu"
      :style="{ left: `${contextMenu.x}px`, top: `${contextMenu.y}px` }"
      @pointerdown.stop
    >
      <button class="ctx-item" @click="triggerAction('new-file')"><span>New File</span><span class="shortcut">{{ getTreeShortcut('new-file') }}</span></button>
      <button class="ctx-item" @click="triggerAction('new-folder')"><span>New Folder</span></button>
      <button class="ctx-item" @click="triggerAction('rename')"><span>Rename</span><span class="shortcut">{{ getTreeShortcut('rename') }}</span></button>
      <button class="ctx-item" @click="triggerAction('change-type')"><span>Change File Type</span></button>
      <div class="ctx-sep" />
      <button class="ctx-item" @click="triggerAction('open-file-dir')"><span>Open Current File Directory</span></button>
      <button class="ctx-item" @click="triggerAction('open-folder-dir')"><span>Open Current Folder</span></button>
      <div class="ctx-sep" />
      <button class="ctx-item" @click="triggerAction('copy')"><span>Copy</span><span class="shortcut">{{ getTreeShortcut('copy') }}</span></button>
      <button class="ctx-item" @click="triggerAction('cut')"><span>Cut</span><span class="shortcut">{{ getTreeShortcut('cut') }}</span></button>
      <button class="ctx-item" @click="triggerAction('paste')"><span>Paste</span><span class="shortcut">{{ getTreeShortcut('paste') }}</span></button>
      <button class="ctx-item" @click="triggerAction('copy-path')"><span>Copy Path</span><span class="shortcut">{{ getTreeShortcut('copy-path') }}</span></button>
      <div v-if="showGitSubmenu" class="ctx-sep" />
      <div
        v-if="showGitSubmenu"
        class="ctx-git-wrap"
        @mouseenter="gitSubmenuOpen = true"
        @mouseleave="gitSubmenuOpen = false"
      >
        <div class="ctx-item ctx-git-trigger">
          <span>Git</span>
          <span class="ctx-chevron">›</span>
        </div>
        <div v-show="gitSubmenuOpen" class="ctx-git-flyout" @pointerdown.stop>
          <button type="button" class="ctx-item" @click="triggerAction('git-diff')">Diff (unstaged)</button>
          <button
            type="button"
            class="ctx-item"
            :disabled="gitLeafUntracked"
            @click="triggerAction('git-diff-staged')"
          >
            Diff (staged)
          </button>
          <button type="button" class="ctx-item" @click="triggerAction('git-diff-head')">Diff vs HEAD</button>
          <div class="ctx-sep" />
          <button type="button" class="ctx-item" @click="triggerAction('git-stage')">Stage</button>
          <button
            type="button"
            class="ctx-item"
            :disabled="gitLeafUntracked"
            @click="triggerAction('git-unstage')"
          >
            Unstage
          </button>
          <div class="ctx-sep" />
          <button type="button" class="ctx-item danger" @click="triggerAction('git-discard')">Rollback…</button>
        </div>
      </div>
      <div class="ctx-sep" />
      <button class="ctx-item" @click="triggerAction('format')"><span>Format Code</span><span class="shortcut">{{ getTreeShortcut('format') }}</span></button>
      <button class="ctx-item" @click="triggerAction('bookmark')"><span>Toggle Bookmark</span><span class="shortcut">{{ getTreeShortcut('bookmark') }}</span></button>
      <button class="ctx-item danger" @click="triggerAction('delete')"><span>Delete</span><span class="shortcut">{{ getTreeShortcut('delete') }}</span></button>
    </div>
  </div>
</template>

<style scoped>
.project-tree {
  width: var(--ide-project-width);
  background-color: var(--ide-bg-main);
  border-right: 1px solid var(--ide-border);
  display: flex;
  flex-direction: column;
  min-width: 0;
}
.project-tree:focus,
.project-tree:focus-visible {
  outline: none;
}
.tree-header {
  height: 32px;
  display: flex;
  align-items: center;
  padding: 8px 12px;
  font-size: calc(var(--ide-tree-font-size, 12px) * 0.92);
  font-weight: 600;
  letter-spacing: 0.02em;
  text-transform: uppercase;
  color: var(--ide-text-muted);
  border-bottom: 1px solid var(--ide-border);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.tree-content {
  overflow: auto;
}
.file-item {
  min-height: 24px;
  display: flex;
  align-items: center;
  font-size: var(--ide-tree-font-size, 12px);
  cursor: pointer;
  white-space: nowrap;
}
.file-item:hover { background-color: var(--ide-hover); }
.file-item.selected {
  background: color-mix(in srgb, var(--ide-accent) 18%, transparent);
}
.file-item.reveal {
  animation: tree-reveal-pulse 1.25s ease-out 1;
}
.expander {
  width: 12px;
  color: var(--ide-text-muted);
  text-align: center;
  margin-right: 2px;
}
.icon {
  width: 14px;
  margin-right: 6px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--ide-text-muted);
}
.label {
  overflow: hidden;
  text-overflow: ellipsis;
}
.bookmark-dot {
  margin-left: auto;
  padding-right: 8px;
  color: color-mix(in srgb, var(--ide-accent) 75%, #ffd45a 25%);
  font-size: 10px;
}

/* Git file / folder colors (tokens from themes.scss) */
.file-item.git-ignored .label,
.file-item.git-folder-ignored .label {
  color: var(--ide-git-ignored);
}
.file-item.git-untracked .label,
.file-item.git-ancestor-untracked .label {
  color: var(--ide-git-untracked);
}
.file-item.git-modified .label,
.file-item.git-ancestor-modified .label {
  color: var(--ide-git-modified);
}
.file-item.git-staged .label,
.file-item.git-ancestor-staged .label {
  color: var(--ide-git-staged);
}
.file-item.git-mixed .label,
.file-item.git-ancestor-mixed .label {
  color: var(--ide-git-mixed);
}
.file-item.git-conflict .label,
.file-item.git-ancestor-conflict .label {
  color: var(--ide-git-conflict);
}
.file-item.git-ancestor-untracked .icon {
  color: color-mix(in srgb, var(--ide-git-ancestor) 55%, var(--ide-git-untracked) 45%);
}
.file-item.git-ancestor-modified .icon {
  color: color-mix(in srgb, var(--ide-git-ancestor) 55%, var(--ide-git-modified) 45%);
}
.file-item.git-ancestor-staged .icon {
  color: color-mix(in srgb, var(--ide-git-ancestor) 55%, var(--ide-git-staged) 45%);
}
.file-item.git-ancestor-mixed .icon {
  color: color-mix(in srgb, var(--ide-git-ancestor) 55%, var(--ide-git-mixed) 45%);
}
.file-item.git-ancestor-conflict .icon {
  color: color-mix(in srgb, var(--ide-git-ancestor) 55%, var(--ide-git-conflict) 45%);
}
.file-item.git-untracked .icon {
  color: color-mix(in srgb, var(--ide-text-muted) 42%, var(--ide-git-untracked) 58%);
}
.file-item.git-modified .icon {
  color: color-mix(in srgb, var(--ide-text-muted) 42%, var(--ide-git-modified) 58%);
}
.file-item.git-staged .icon {
  color: color-mix(in srgb, var(--ide-text-muted) 42%, var(--ide-git-staged) 58%);
}
.file-item.git-mixed .icon {
  color: color-mix(in srgb, var(--ide-text-muted) 42%, var(--ide-git-mixed) 58%);
}
.file-item.git-conflict .icon {
  color: color-mix(in srgb, var(--ide-text-muted) 42%, var(--ide-git-conflict) 58%);
}
.file-item.git-ignored .icon,
.file-item.git-folder-ignored .icon {
  color: var(--ide-git-ignored);
  opacity: 0.85;
}
.tree-context-menu {
  position: fixed;
  min-width: 220px;
  max-width: 280px;
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  z-index: 250;
  padding: 6px 0;
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
}
.ctx-item {
  width: 100%;
  padding: 0 10px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  text-align: left;
  font-size: 12px;
  color: var(--ide-text);
  cursor: pointer;
}
.shortcut {
  color: var(--ide-text-muted);
  font-size: 11px;
}
.ctx-item:hover { background: var(--ide-hover); }
.ctx-item.danger { color: #e06b6b; }
.ctx-item:disabled {
  opacity: 0.45;
  cursor: not-allowed;
  pointer-events: none;
}
.ctx-git-wrap {
  position: relative;
}
.ctx-git-trigger {
  cursor: default;
}
.ctx-chevron {
  margin-left: auto;
  padding-left: 10px;
  color: var(--ide-text-muted);
  font-size: 14px;
  line-height: 1;
}
.ctx-git-flyout {
  position: absolute;
  left: 100%;
  top: 0;
  margin-left: -1px;
  min-width: 208px;
  max-width: 280px;
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  z-index: 260;
  padding: 6px 0;
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
}
.ctx-sep {
  height: 1px;
  margin: 6px 8px;
  background: var(--ide-border);
}
@keyframes tree-reveal-pulse {
  0% { background: color-mix(in srgb, var(--ide-accent) 46%, transparent); }
  100% { background: transparent; }
}
</style>