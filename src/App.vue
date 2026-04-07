<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import { invoke } from '@tauri-apps/api/core';
import { open } from '@tauri-apps/plugin-dialog';
import TitleBar from './components/layout/TitleBar.vue';
import LeftSideBar from './components/layout/LeftSideBar.vue';
import RightSideBar from './components/layout/RightSideBar.vue';
import ProjectTree from './components/layout/ProjectTree.vue';
import EditorMain from './components/layout/EditorMain.vue';
import ToolWindow from './components/layout/ToolWindow.vue'; // 右侧面板
import StatusBar from './components/layout/StatusBar.vue';
import SettingsDialog from './components/layout/SettingsDialog.vue';
import TerminalPanel from './components/layout/TerminalPanel.vue';
import GitPanel from './components/layout/GitPanel.vue';
import GitCommitDialog from './components/layout/GitCommitDialog.vue';
import GitDiffDialog from './components/layout/GitDiffDialog.vue';
import UiPromptDialog from './components/ui/UiPromptDialog.vue';
import HelpAboutDialog from './components/layout/HelpAboutDialog.vue';
import HelpShortcutsDialog from './components/layout/HelpShortcutsDialog.vue';
import { getMenuShortcutHelpItems } from './config/menuConfig';
import { treeShortcutHelpItems } from './config/treeShortcuts';
import { globalKeybindings, matchesShortcut, treeKeybindings } from './config/keybindings';

type TreeNode = {
  name: string;
  path: string;
  isDir: boolean;
  children?: TreeNode[];
  loaded?: boolean;
};
type OpenTab = {
  path: string;
  name: string;
  content: string;
  originalContent: string;
  encoding: string;
  lineEnding: 'LF' | 'CRLF';
  isBinaryImage?: boolean;
};
type ImageDataUrlPayload = {
  mime: string;
  dataUrl: string;
};
type LeftTool = 'explorer' | 'search' | 'source' | 'run' | 'extensions';
type EditorMainExpose = {
  runEditorAction: (action: 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'find' | 'selectAll' | 'format') => Promise<void> | void;
  setIndentation: (tabSize: number, insertSpaces: boolean) => void;
  getIndentation: () => { tabSize: number; insertSpaces: boolean };
  setLineEnding: (lineEnding: 'LF' | 'CRLF') => void;
  getLineEnding: () => 'LF' | 'CRLF';
  formatDocument: (tabSize: number, insertSpaces: boolean) => Promise<void> | void;
  getSelectionContext: () => {
    path: string;
    fileName: string;
    startLine: number;
    endLine: number;
    snippet: string;
  } | null;
};
type TerminalPanelExpose = {
  createTerminal: () => Promise<void> | void;
  clearActive: () => void;
  killActive: () => Promise<void> | void;
  focusActive: () => Promise<void> | void;
  renameActive: () => void;
};
type GitPanelExpose = {
  refresh: () => Promise<void> | void;
  setSection: (section: 'head' | 'local' | 'remote' | 'tags') => void;
};

const showProject = ref(true);
const leftTool = ref<LeftTool>('explorer');
const showAI = ref(false);
const showBottom = ref(false);
const bottomTool = ref<'terminal' | 'git'>('terminal');

const projectWidth = ref(260);
const aiWidth = ref(320);
const workspacePath = ref('');
const isGitRepo = ref(false);
const gitTreeDecorations = ref<Record<string, string>>({});
let gitTreeDecorationsTimer: ReturnType<typeof setTimeout> | null = null;
const treeNodes = ref<TreeNode[]>([]);
const expandedPaths = ref(new Set<string>());
const treeBookmarks = ref(new Set<string>());
const treeClipboard = ref<{ mode: 'copy' | 'cut'; path: string; isDir: boolean } | null>(null);
const treeRevealPath = ref<string | null>(null);
const treeSelectedPath = ref<string | null>(null);
const openTabs = ref<OpenTab[]>([]);
const activeFilePath = ref<string | null>(null);
const splitRightEnabled = ref(false);
const activeEditorGroup = ref<'primary' | 'secondary'>('primary');
const primaryTabPaths = ref<string[]>([]);
const secondaryTabPaths = ref<string[]>([]);
const activePrimaryPath = ref<string | null>(null);
const activeSecondaryPath = ref<string | null>(null);
const terminalLogs = ref<string[]>(['Ready.']);
const showSettings = ref(false);
type SettingsSection = 'appearance' | 'editor' | 'autosave' | 'shell';
const settingsInitialSection = ref<SettingsSection>('appearance');
const showGitCommitDialog = ref(false);
const gitCommitMessage = ref('');
type GitChangeItem = { path: string; code: string; selected: boolean; untracked: boolean };
const gitCommitChanges = ref<GitChangeItem[]>([]);
const gitCommitLoading = ref(false);
const gitCommitError = ref('');
const gitCommitAmend = ref(false);
const gitCommitDiffText = ref('');
const gitCommitDiffLoading = ref(false);
const gitCommitSelectedDiffPath = ref('');
const showGitDiffDialog = ref(false);
/** When set (e.g. project tree), diff dialog prev/next uses this list instead of commit dialog files. */
const gitDiffNavFilesOverride = ref<string[] | null>(null);
const gitDiffFileList = computed(() => gitCommitChanges.value.map((c) => c.path));
const gitDiffDialogFileList = computed(() => {
  const o = gitDiffNavFilesOverride.value;
  if (o?.length) return o;
  return gitDiffFileList.value;
});
const gitDiffDialogCurrentIndex = computed(() =>
  gitDiffDialogFileList.value.findIndex((p) => p === gitCommitSelectedDiffPath.value),
);
const theme = ref<'dark' | 'light' | 'monaco'>('dark');
const editorFontSize = ref(13);
const treeFontSize = ref(12);
const autoSaveDelay = ref(700);
let autoSaveTimer: number | null = null;
const primaryEditorRef = ref<EditorMainExpose | null>(null);
const secondaryEditorRef = ref<EditorMainExpose | null>(null);
const terminalRef = ref<TerminalPanelExpose | null>(null);
const gitPanelRef = ref<GitPanelExpose | null>(null);
let persistTimer: number | null = null;
let memoryInterval: number | null = null;
let aiStatusInterval: number | null = null;

// Status bar state (best-effort, IDE-like)
const gitBranch = ref('main');
const encoding = ref('UTF-8');
const lineEnding = ref<'LF' | 'CRLF'>('LF');
const cursorLine = ref(1);
const cursorCol = ref(1);
const indentTabSize = ref(2);
const indentInsertSpaces = ref(true);
const aiOnline = ref(true);
const notificationsCount = ref(0);
const memoryText = ref('—');
const memoryPercent = ref(0);
type NoticeLevel = 'info' | 'success' | 'warning' | 'error';
type NoticeItem = { id: number; level: NoticeLevel; message: string; detail?: string; expanded?: boolean };
const notices = ref<NoticeItem[]>([]);
let noticeIdSeed = 1;
const promptDialogOpen = ref(false);
const promptDialogMode = ref<'input' | 'confirm' | 'select'>('input');
const promptDialogTitle = ref('');
const promptDialogMessage = ref('');
const promptDialogValue = ref('');
const promptDialogPlaceholder = ref('');
const promptDialogOptions = ref<Array<{ label: string; value: string }>>([]);
const promptDialogConfirmText = ref('OK');
const promptDialogCancelText = ref('Cancel');
let promptDialogResolver: ((value: string | boolean | null) => void) | null = null;
const showHelpAbout = ref(false);
const showHelpShortcuts = ref(false);

const pushNotice = (level: NoticeLevel, message: string, duration = 3200, detail?: string) => {
  const item: NoticeItem = { id: noticeIdSeed++, level, message, detail, expanded: false };
  notices.value = [...notices.value, item].slice(-5);
  notificationsCount.value = notices.value.length;
  if (duration > 0) {
    window.setTimeout(() => {
      notices.value = notices.value.filter((n) => n.id !== item.id);
      notificationsCount.value = notices.value.length;
    }, duration);
  }
};

const dismissNotice = (id: number) => {
  notices.value = notices.value.filter((n) => n.id !== id);
  notificationsCount.value = notices.value.length;
};

const openInputDialog = (opts: {
  title: string;
  message?: string;
  defaultValue?: string;
  placeholder?: string;
  confirmText?: string;
  cancelText?: string;
}) => new Promise<string | null>((resolve) => {
  promptDialogMode.value = 'input';
  promptDialogTitle.value = opts.title;
  promptDialogMessage.value = opts.message ?? '';
  promptDialogValue.value = opts.defaultValue ?? '';
  promptDialogPlaceholder.value = opts.placeholder ?? '';
  promptDialogOptions.value = [];
  promptDialogConfirmText.value = opts.confirmText ?? 'OK';
  promptDialogCancelText.value = opts.cancelText ?? 'Cancel';
  promptDialogResolver = resolve;
  promptDialogOpen.value = true;
});

const openConfirmDialog = (opts: {
  title: string;
  message?: string;
  confirmText?: string;
  cancelText?: string;
}) => new Promise<boolean>((resolve) => {
  promptDialogMode.value = 'confirm';
  promptDialogTitle.value = opts.title;
  promptDialogMessage.value = opts.message ?? '';
  promptDialogValue.value = '';
  promptDialogPlaceholder.value = '';
  promptDialogOptions.value = [];
  promptDialogConfirmText.value = opts.confirmText ?? 'Confirm';
  promptDialogCancelText.value = opts.cancelText ?? 'Cancel';
  promptDialogResolver = resolve;
  promptDialogOpen.value = true;
});

const openSelectDialog = (opts: {
  title: string;
  message?: string;
  options: Array<{ label: string; value: string }>;
  defaultValue?: string;
  confirmText?: string;
  cancelText?: string;
}) => new Promise<string | null>((resolve) => {
  promptDialogMode.value = 'select';
  promptDialogTitle.value = opts.title;
  promptDialogMessage.value = opts.message ?? '';
  promptDialogOptions.value = opts.options;
  promptDialogValue.value = opts.defaultValue ?? opts.options[0]?.value ?? '';
  promptDialogPlaceholder.value = '';
  promptDialogConfirmText.value = opts.confirmText ?? 'OK';
  promptDialogCancelText.value = opts.cancelText ?? 'Cancel';
  promptDialogResolver = resolve;
  promptDialogOpen.value = true;
});

const resolvePromptDialog = (value: string | boolean | null) => {
  const resolver = promptDialogResolver;
  promptDialogResolver = null;
  promptDialogOpen.value = false;
  resolver?.(value);
};

const onPromptDialogConfirm = () => {
  if (promptDialogMode.value === 'confirm') {
    resolvePromptDialog(true);
    return;
  }
  resolvePromptDialog(promptDialogValue.value);
};

const onPromptDialogCancel = () => {
  if (promptDialogMode.value === 'confirm') {
    resolvePromptDialog(false);
    return;
  }
  resolvePromptDialog(null);
};

const toggleNotice = (id: number) => {
  notices.value = notices.value.map((n) => (n.id === id ? { ...n, expanded: !n.expanded } : n));
};

const shortcutItems = computed(() => [
  ...getMenuShortcutHelpItems(),
  ...treeShortcutHelpItems,
  { category: 'Monaco Editor', key: 'Ctrl+/', action: 'Toggle Line Comment', description: 'Toggle line comment on selection/current line' },
  { category: 'Monaco Editor', key: 'Ctrl+F', action: 'Find', description: 'Open find widget in editor' },
  { category: 'Monaco Editor', key: 'Ctrl+H', action: 'Replace', description: 'Open replace widget in editor' },
  { category: 'Monaco Editor', key: 'F3 / Enter', action: 'Find Next', description: 'Jump to next search match' },
  { category: 'Monaco Editor', key: 'Shift+F3 / Shift+Enter', action: 'Find Previous', description: 'Jump to previous search match' },
  { category: 'Monaco Editor', key: 'Alt+Enter', action: 'Select All Matches', description: 'Select all matches in current document' },
  { category: 'Monaco Editor', key: 'Escape', action: 'Close Find/Replace', description: 'Close search widget' },
  { category: 'Monaco Editor', key: 'Shift+Alt+F', action: 'Format Document', description: 'Format entire document (Monaco default)' },
  { category: 'Monaco Editor', key: 'F2', action: 'Rename Symbol', description: 'Rename symbol under cursor (language support required)' },
  { category: 'Monaco Editor', key: 'F12', action: 'Go to Definition', description: 'Jump to symbol definition (language support required)' },
  { category: 'Monaco Editor', key: 'Alt+F12', action: 'Peek Definition', description: 'Inline definition preview (language support required)' },
  { category: 'Monaco Editor', key: 'Shift+F12', action: 'Go to References', description: 'List references for current symbol' },
  { category: 'Monaco Editor', key: 'Ctrl+Space', action: 'Trigger Suggest', description: 'Open completion suggestions' },
  { category: 'Monaco Editor', key: 'Ctrl+Shift+Space', action: 'Parameter Hints', description: 'Show parameter hints' },
  { category: 'Monaco Editor', key: 'Ctrl+L', action: 'Select Line', description: 'Select current line' },
  { category: 'Monaco Editor', key: 'Ctrl+D', action: 'Add Selection to Next Match', description: 'Multi-cursor next match selection' },
  { category: 'Monaco Editor', key: 'Ctrl+Shift+K', action: 'Delete Line / 删除行', description: 'Delete current line' },
  { category: 'Monaco Editor', key: 'Alt+Up / Alt+Down', action: 'Move Line Up/Down', description: 'Move current line or selection' },
  { category: 'Monaco Editor', key: 'Shift+Alt+Up', action: 'Copy Line Up', description: 'Duplicate current line/selection upward' },
  { category: 'Monaco Editor', key: 'Shift+Alt+Down', action: 'Copy Line Down', description: 'Duplicate current line/selection downward' },
  { category: 'Monaco Editor', key: 'Ctrl+]', action: 'Indent Line', description: 'Increase indentation' },
  { category: 'Monaco Editor', key: 'Ctrl+[', action: 'Outdent Line', description: 'Decrease indentation' },
  { category: 'Monaco Editor', key: 'Ctrl+G', action: 'Go to Line', description: 'Jump to line/column' },
  { category: 'Monaco Editor', key: 'Ctrl+P', action: 'Quick Open (Editor)', description: 'Open quick file/symbol palette (if enabled in host)' },
  { category: 'Monaco Editor', key: 'Ctrl+Shift+P', action: 'Command Palette', description: 'Open command palette (if enabled in host)' },
  { category: 'AI', key: 'Ctrl+Shift+L', action: 'Add Selection to AI', description: 'Send selected snippet to AI chat' },
  { category: 'Tabs', key: 'Ctrl+W', action: 'Close Tab', description: 'Close current tab' },
  { category: 'Tabs', key: 'Ctrl+Shift+W', action: 'Close All Tabs', description: 'Close all tabs' },
  { category: 'Tabs', key: 'Ctrl+Alt+W', action: 'Close Other Tabs', description: 'Close tabs except current' },
  { category: 'Tabs', key: 'Ctrl+Alt+S', action: 'Close Saved Tabs', description: 'Close clean tabs only' },
  { category: 'Tabs', key: 'Ctrl+\\', action: 'Split Right', description: 'Split editor to right' },
  { category: 'Tabs', key: 'Ctrl+Alt+\\', action: 'Move to Next Group', description: 'Move tab to next editor group' },
  { category: 'Tabs', key: 'Ctrl+Shift+\\', action: 'Close Group', description: 'Close active editor group' },
]);

type PersistedState = {
  workspacePath: string;
  openTabs: string[];
  activeFilePath: string | null;
  projectWidth: number;
  aiWidth: number;
  showProject: boolean;
  showAI: boolean;
  showBottom: boolean;
  treeFontSize: number;
};

const clamp = (value: number, min: number, max: number) => Math.min(max, Math.max(min, value));
const detectLineEnding = (text: string): 'LF' | 'CRLF' => (text.includes('\r\n') ? 'CRLF' : 'LF');
const pathDirname = (path: string) => {
  const normalized = path.replace(/[\\/]+$/, '');
  const idx = Math.max(normalized.lastIndexOf('/'), normalized.lastIndexOf('\\'));
  return idx > 0 ? normalized.slice(0, idx) : normalized;
};
const pathBasename = (path: string) => {
  const normalized = path.replace(/[\\/]+$/, '');
  const idx = Math.max(normalized.lastIndexOf('/'), normalized.lastIndexOf('\\'));
  return idx >= 0 ? normalized.slice(idx + 1) : normalized;
};
const joinPath = (dir: string, name: string) => {
  const sep = dir.includes('\\') ? '\\' : '/';
  return `${dir.replace(/[\\/]+$/, '')}${sep}${name}`;
};

/** Case-insensitive path compare (normalizes slashes, trims trailing separators). */
const pathsEqual = (a: string, b: string) => {
  const norm = (p: string) => p.replace(/[\\/]+$/, '').replace(/\\/g, '/').toLowerCase();
  return norm(a) === norm(b);
};

/** Match Rust `list_dir`: directories first, then localeCompare name (base-insensitive). */
const compareTreeNodes = (x: TreeNode, y: TreeNode) => {
  if (x.isDir !== y.isDir) return x.isDir ? -1 : 1;
  return x.name.localeCompare(y.name, undefined, { sensitivity: 'base' });
};

const insertChildSorted = (children: TreeNode[], node: TreeNode): TreeNode[] =>
  [...children.filter((c) => !pathsEqual(c.path, node.path)), node].sort(compareTreeNodes);

/** Insert `entry` as child of `parentDir` in the in-memory tree; returns false if parent not found. */
const mergeEntryUnderParent = (nodes: TreeNode[], parentDir: string, entry: TreeNode): TreeNode[] | null => {
  const out: TreeNode[] = [];
  let found = false;
  for (const node of nodes) {
    if (pathsEqual(node.path, parentDir)) {
      const base = node.children ?? [];
      out.push({
        ...node,
        children: insertChildSorted(base, entry),
        loaded: true,
      });
      found = true;
      continue;
    }
    if (node.children?.length) {
      const sub = mergeEntryUnderParent(node.children, parentDir, entry);
      if (sub) {
        out.push({ ...node, children: sub });
        found = true;
        continue;
      }
    }
    out.push(node);
  }
  return found ? out : null;
};

/** Update tree after create without full `reloadWorkspaceTree` (avoids explorer flash). */
const mergeNewTreeEntry = (parentDir: string, entry: TreeNode): boolean => {
  const root = workspacePath.value;
  if (!root) return false;
  if (pathsEqual(parentDir, root)) {
    treeNodes.value = insertChildSorted(treeNodes.value, entry);
    return true;
  }
  const next = mergeEntryUnderParent(treeNodes.value, parentDir, entry);
  if (!next) return false;
  treeNodes.value = next;
  return true;
};

/** Rebase `p` when it equals or extends `oldRoot` to use `newRoot` (same rules as tab path remap). */
const pathReplacePrefix = (p: string, oldRoot: string, newRoot: string): string => {
  const o = oldRoot.replace(/[\\/]+$/, '');
  const t = newRoot.replace(/[\\/]+$/, '');
  if (pathsEqual(p, o) || pathsEqual(p, oldRoot)) return t;
  const ol = o.toLowerCase();
  const pl = p.toLowerCase();
  if (pl.startsWith(`${ol}/`) || pl.startsWith(`${ol}\\`)) {
    const suffix = p.slice(o.length);
    const first = suffix[0];
    if (first === '/' || first === '\\') {
      const useSep = t.includes('\\') ? '\\' : '/';
      return `${t}${useSep}${suffix.slice(1)}`;
    }
  }
  return p;
};

const remapSubtreeRoots = (node: TreeNode, oldRoot: string, newRoot: string): TreeNode => {
  const mapOne = (x: TreeNode): TreeNode => {
    const np = pathReplacePrefix(x.path, oldRoot, newRoot);
    return {
      ...x,
      path: np,
      name: pathBasename(np),
      children: x.children?.map(mapOne),
    };
  };
  return mapOne(node);
};

const applyTreeRename = (oldPath: string, newPath: string): boolean => {
  const walk = (nodes: TreeNode[]): { list: TreeNode[]; hit: boolean } => {
    let hit = false;
    const list = nodes.map((n) => {
      if (pathsEqual(n.path, oldPath)) {
        hit = true;
        return remapSubtreeRoots(n, oldPath, newPath);
      }
      if (n.children?.length) {
        const { list: ch, hit: h } = walk(n.children);
        if (h) {
          hit = true;
          return { ...n, children: ch };
        }
      }
      return n;
    });
    return { list, hit };
  };
  const { list, hit } = walk(treeNodes.value);
  if (!hit) return false;
  treeNodes.value = list;
  return true;
};

const removeNodeFromTreeByPath = (targetPath: string): boolean => {
  const walk = (nodes: TreeNode[]): { list: TreeNode[]; hit: boolean } => {
    let hit = false;
    const list: TreeNode[] = [];
    for (const n of nodes) {
      if (pathsEqual(n.path, targetPath)) {
        hit = true;
        continue;
      }
      if (n.children?.length) {
        const { list: ch, hit: h } = walk(n.children);
        if (h) {
          hit = true;
          list.push({ ...n, children: ch });
        } else {
          list.push(n);
        }
        continue;
      }
      list.push(n);
    }
    return { list, hit };
  };
  const { list, hit } = walk(treeNodes.value);
  if (!hit) return false;
  treeNodes.value = list;
  return true;
};

const extractNodeByPath = (nodes: TreeNode[], targetPath: string): { next: TreeNode[]; extracted: TreeNode | null } => {
  let extracted: TreeNode | null = null;
  const next: TreeNode[] = [];
  for (const n of nodes) {
    if (pathsEqual(n.path, targetPath)) {
      extracted = n;
      continue;
    }
    if (n.children?.length) {
      const { next: ch, extracted: e } = extractNodeByPath(n.children, targetPath);
      if (e) {
        extracted = e;
        next.push({ ...n, children: ch });
        continue;
      }
    }
    next.push(n);
  }
  return { next, extracted };
};

const pruneExpandedForDeletedPath = (targetPath: string) => {
  const tl = targetPath.replace(/[\\/]+$/, '').toLowerCase();
  const next = new Set<string>();
  for (const p of expandedPaths.value) {
    const pl = p.replace(/[\\/]+$/, '').toLowerCase();
    if (pl === tl || pl.startsWith(`${tl}/`) || pl.startsWith(`${tl}\\`)) continue;
    next.add(p);
  }
  expandedPaths.value = next;
};

const pruneBookmarksUnderPath = (targetPath: string) => {
  const tl = targetPath.replace(/[\\/]+$/, '').toLowerCase();
  const next = new Set<string>();
  for (const b of treeBookmarks.value) {
    const bl = b.toLowerCase();
    if (bl === tl || bl.startsWith(`${tl}/`) || bl.startsWith(`${tl}\\`)) continue;
    next.add(b);
  }
  treeBookmarks.value = next;
  localStorage.setItem('ide.treeBookmarks', JSON.stringify(Array.from(treeBookmarks.value)));
};

const remapBookmarkPathsForRename = (oldPath: string, newPath: string) => {
  const oldLower = oldPath.toLowerCase();
  const remap = (p: string) => {
    const lp = p.toLowerCase();
    if (lp === oldLower) return newPath;
    if (lp.startsWith(`${oldLower}/`) || lp.startsWith(`${oldLower}\\`)) {
      return `${newPath}${p.slice(oldPath.length)}`;
    }
    return p;
  };
  treeBookmarks.value = new Set(Array.from(treeBookmarks.value).map(remap));
  localStorage.setItem('ide.treeBookmarks', JSON.stringify(Array.from(treeBookmarks.value)));
};

const isRasterImagePath = (path: string) => {
  const lower = path.toLowerCase();
  return (
    lower.endsWith('.png')
    || lower.endsWith('.jpg')
    || lower.endsWith('.jpeg')
    || lower.endsWith('.gif')
    || lower.endsWith('.webp')
    || lower.endsWith('.bmp')
    || lower.endsWith('.ico')
  );
};

const activeTab = computed(() => openTabs.value.find((tab) => tab.path === activeFilePath.value) ?? null);
const activeFileName = computed(() => activeTab.value?.name ?? 'Welcome');
const activeFileContent = computed(() => activeTab.value?.content ?? '');
const isDirty = computed(() => !!activeTab.value && activeTab.value.content !== activeTab.value.originalContent);
const statusFilePath = computed(() => activeTab.value?.path ?? (workspacePath.value || 'No file'));
const indentLabel = computed(() => (indentInsertSpaces.value ? `Spaces: ${indentTabSize.value}` : `Tab Size: ${indentTabSize.value}`));
const expandedPathList = computed(() => Array.from(expandedPaths.value));
const allFiles = computed(() => {
  const files: Array<{ name: string; path: string }> = [];
  const walk = (nodes: TreeNode[]) => {
    for (const node of nodes) {
      if (node.isDir) {
        if (node.children?.length) walk(node.children);
      } else {
        files.push({ name: node.name, path: node.path });
      }
    }
  };
  walk(treeNodes.value);
  return files;
});
const searchKeyword = ref('');
const searchResults = ref<Array<{ name: string; path: string }>>([]);
const searchSelectedIndex = ref(0);
let searchTimer: number | null = null;
const runWorkspaceSearch = async () => {
  if (!workspacePath.value) {
    searchResults.value = [];
    return;
  }
  const key = searchKeyword.value.trim();
  if (!key) {
    searchResults.value = allFiles.value.slice(0, 50);
    searchSelectedIndex.value = searchResults.value.length ? 0 : -1;
    return;
  }
  try {
    const rows = await invoke<TreeNode[]>('search_files_by_name', {
      path: workspacePath.value,
      keyword: key,
      limit: 300,
    });
    searchResults.value = rows.map((r) => ({ name: r.name, path: r.path }));
    searchSelectedIndex.value = searchResults.value.length ? 0 : -1;
  } catch (error) {
    appendLog(`Search failed: ${String(error)}`);
    searchResults.value = [];
    searchSelectedIndex.value = -1;
  }
};

const escapeHtml = (text: string) => text
  .replaceAll('&', '&amp;')
  .replaceAll('<', '&lt;')
  .replaceAll('>', '&gt;')
  .replaceAll('"', '&quot;')
  .replaceAll("'", '&#39;');

const highlightSearchText = (text: string) => {
  const key = searchKeyword.value.trim();
  if (!key) return escapeHtml(text);
  const lower = text.toLowerCase();
  const needle = key.toLowerCase();
  let i = 0;
  let out = '';
  while (i < text.length) {
    const hit = lower.indexOf(needle, i);
    if (hit < 0) {
      out += escapeHtml(text.slice(i));
      break;
    }
    out += escapeHtml(text.slice(i, hit));
    out += `<mark class="search-hit">${escapeHtml(text.slice(hit, hit + key.length))}</mark>`;
    i = hit + key.length;
  }
  return out;
};

const openSearchResult = async (path: string, index?: number) => {
  if (typeof index === 'number') searchSelectedIndex.value = index;
  await openFile(path);
};

const onSearchInputKeydown = (event: KeyboardEvent) => {
  if (!searchResults.value.length) return;
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    searchSelectedIndex.value = Math.min(searchResults.value.length - 1, searchSelectedIndex.value + 1);
    return;
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault();
    searchSelectedIndex.value = Math.max(0, searchSelectedIndex.value - 1);
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    const row = searchResults.value[Math.max(0, searchSelectedIndex.value)];
    if (row) void openSearchResult(row.path, searchSelectedIndex.value);
  }
};

const tabsByPaths = (paths: string[]) =>
  paths
    .map((path) => openTabs.value.find((tab) => tab.path === path) ?? null)
    .filter((tab): tab is OpenTab => !!tab);

const primaryTabs = computed(() => tabsByPaths(primaryTabPaths.value));
const secondaryTabs = computed(() => tabsByPaths(secondaryTabPaths.value));
const primaryFilePath = computed(() => activePrimaryPath.value);
const secondaryFilePath = computed(() => activeSecondaryPath.value);
const primaryFileName = computed(() => primaryTabs.value.find((t) => t.path === activePrimaryPath.value)?.name ?? 'Welcome');
const secondaryFileName = computed(() => secondaryTabs.value.find((t) => t.path === activeSecondaryPath.value)?.name ?? 'Welcome');
const primaryContent = computed(() => primaryTabs.value.find((t) => t.path === activePrimaryPath.value)?.content ?? '');
const secondaryContent = computed(() => secondaryTabs.value.find((t) => t.path === activeSecondaryPath.value)?.content ?? '');
const primaryDirty = computed(() => {
  const tab = primaryTabs.value.find((t) => t.path === activePrimaryPath.value);
  return !!tab && tab.content !== tab.originalContent;
});
const secondaryDirty = computed(() => {
  const tab = secondaryTabs.value.find((t) => t.path === activeSecondaryPath.value);
  return !!tab && tab.content !== tab.originalContent;
});

const startProjectResize = (event: PointerEvent) => {
  event.preventDefault();
  const startX = event.clientX;
  const startWidth = projectWidth.value;

  const onMove = (moveEvent: PointerEvent) => {
    const delta = moveEvent.clientX - startX;
    projectWidth.value = clamp(startWidth + delta, 180, 520);
  };

  const onUp = () => {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
  };

  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp, { once: true });
};

const startAiResize = (event: PointerEvent) => {
  event.preventDefault();
  const startX = event.clientX;
  const startWidth = aiWidth.value;

  const onMove = (moveEvent: PointerEvent) => {
    const delta = startX - moveEvent.clientX;
    aiWidth.value = clamp(startWidth + delta, 260, 560);
  };

  const onUp = () => {
    window.removeEventListener('pointermove', onMove);
    window.removeEventListener('pointerup', onUp);
  };

  window.addEventListener('pointermove', onMove);
  window.addEventListener('pointerup', onUp, { once: true });
};

const updateNodeByPath = (nodes: TreeNode[], targetPath: string, updater: (node: TreeNode) => TreeNode): TreeNode[] => {
  return nodes.map((node) => {
    if (node.path === targetPath) {
      return updater(node);
    }
    if (!node.children?.length) return node;
    return {
      ...node,
      children: updateNodeByPath(node.children, targetPath, updater),
    };
  });
};

const loadChildren = async (path: string) => {
  const children = (await invoke<TreeNode[]>('list_dir', { path })).map((node) => ({
    ...node,
    loaded: !node.isDir,
  }));
  treeNodes.value = updateNodeByPath(treeNodes.value, path, (node) => ({
    ...node,
    children,
    loaded: true,
  }));
  // Do not refresh git decorations on every folder expand; it creates unnecessary git churn.
  // Decorations are refreshed on workspace changes / file ops / explicit git actions.
};

const toggleFolder = async (path: string) => {
  treeSelectedPath.value = path;
  if (expandedPaths.value.has(path)) {
    expandedPaths.value.delete(path);
    expandedPaths.value = new Set(expandedPaths.value);
    return;
  }

  expandedPaths.value.add(path);
  expandedPaths.value = new Set(expandedPaths.value);
  const target = path;
  const needsLoad = (() => {
    const stack = [...treeNodes.value];
    while (stack.length) {
      const node = stack.pop()!;
      if (node.path === target) return node.isDir && !node.loaded;
      if (node.children?.length) stack.push(...node.children);
    }
    return false;
  })();
  if (needsLoad) {
    try {
      await loadChildren(path);
    } catch (error) {
      appendLog(`Load folder failed: ${String(error)}`);
    }
  }
};

const reloadWorkspaceTree = async () => {
  if (!workspacePath.value) return;
  const nodes = await invoke<TreeNode[]>('list_dir', { path: workspacePath.value });
  treeNodes.value = nodes.map((node) => ({ ...node, loaded: !node.isDir }));
  const expandedList = Array.from(expandedPaths.value).filter((p) => p !== workspacePath.value);
  for (const dirPath of expandedList) {
    try {
      const children = (await invoke<TreeNode[]>('list_dir', { path: dirPath })).map((node) => ({
        ...node,
        loaded: !node.isDir,
      }));
      treeNodes.value = updateNodeByPath(treeNodes.value, dirPath, (node) => ({
        ...node,
        children,
        loaded: true,
      }));
    } catch {
      // Ignore stale expanded entries (renamed/deleted paths) during reload.
    }
  }
  scheduleGitTreeDecorations();
};

const closeTabsUnderPath = (targetPath: string) => {
  const t = targetPath.toLowerCase();
  const isParent = (p: string) => {
    const lp = p.toLowerCase();
    return lp === t || lp.startsWith(`${t}/`) || lp.startsWith(`${t}\\`);
  };
  const removed = new Set(openTabs.value.filter((tab) => isParent(tab.path)).map((tab) => tab.path));
  if (!removed.size) return;
  openTabs.value = openTabs.value.filter((tab) => !removed.has(tab.path));
  primaryTabPaths.value = primaryTabPaths.value.filter((p) => !removed.has(p));
  secondaryTabPaths.value = secondaryTabPaths.value.filter((p) => !removed.has(p));
  if (activeFilePath.value && removed.has(activeFilePath.value)) {
    activeFilePath.value = primaryTabPaths.value.at(-1) ?? secondaryTabPaths.value.at(-1) ?? null;
  }
};

const renameTabsForPath = (oldPath: string, newPath: string) => {
  const oldLower = oldPath.toLowerCase();
  const remap = (p: string) => {
    const lp = p.toLowerCase();
    if (lp === oldLower) return newPath;
    if (lp.startsWith(`${oldLower}/`) || lp.startsWith(`${oldLower}\\`)) {
      return `${newPath}${p.slice(oldPath.length)}`;
    }
    return p;
  };
  openTabs.value = openTabs.value.map((tab) => {
    const mapped = remap(tab.path);
    return mapped === tab.path ? tab : { ...tab, path: mapped, name: pathBasename(mapped) };
  });
  primaryTabPaths.value = primaryTabPaths.value.map(remap);
  secondaryTabPaths.value = secondaryTabPaths.value.map(remap);
  if (activeFilePath.value) activeFilePath.value = remap(activeFilePath.value);
};

const remapExpandedPathsForRename = (oldPath: string, newPath: string) => {
  const oldLower = oldPath.toLowerCase();
  const mapped = new Set<string>();
  for (const p of expandedPaths.value) {
    const lp = p.toLowerCase();
    if (lp === oldLower) {
      mapped.add(newPath);
      continue;
    }
    if (lp.startsWith(`${oldLower}/`) || lp.startsWith(`${oldLower}\\`)) {
      mapped.add(`${newPath}${p.slice(oldPath.length)}`);
      continue;
    }
    mapped.add(p);
  }
  expandedPaths.value = mapped;
};

const handleTreeAction = async (payload: { action: string; path: string; isDir: boolean }) => {
  const { action, path, isDir } = payload;
  const parentDir = isDir ? path : pathDirname(path);
  try {
    if (action === 'new-file') {
      const name = (await openInputDialog({
        title: 'New File',
        message: 'Enter file name',
        defaultValue: 'new-file.txt',
        confirmText: 'Create',
      }))?.trim();
      if (!name) return;
      const createdPath = joinPath(parentDir, name);
      await invoke('create_file', { path: createdPath });
      const fileNode: TreeNode = { name, path: createdPath, isDir: false, loaded: true };
      if (!mergeNewTreeEntry(parentDir, fileNode)) {
        await loadChildren(parentDir);
      }
      scheduleGitTreeDecorations();
      expandedPaths.value.add(parentDir);
      expandedPaths.value = new Set(expandedPaths.value);
      treeSelectedPath.value = createdPath;
      treeRevealPath.value = createdPath;
      await openFile(createdPath);
      pushNotice('success', `Created file: ${name}`);
      return;
    }
    if (action === 'new-folder') {
      const name = (await openInputDialog({
        title: 'New Folder',
        message: 'Enter folder name',
        defaultValue: 'new-folder',
        confirmText: 'Create',
      }))?.trim();
      if (!name) return;
      const createdPath = joinPath(parentDir, name);
      await invoke('create_folder', { path: createdPath });
      const folderNode: TreeNode = { name, path: createdPath, isDir: true, loaded: false };
      if (!mergeNewTreeEntry(parentDir, folderNode)) {
        await loadChildren(parentDir);
      }
      scheduleGitTreeDecorations();
      expandedPaths.value.add(parentDir);
      expandedPaths.value = new Set(expandedPaths.value);
      treeSelectedPath.value = createdPath;
      treeRevealPath.value = createdPath;
      pushNotice('success', `Created folder: ${name}`);
      return;
    }
    if (action === 'rename') {
      const nextName = (await openInputDialog({
        title: 'Rename',
        message: 'Enter new name',
        defaultValue: pathBasename(path),
        confirmText: 'Rename',
      }))?.trim();
      if (!nextName || nextName === pathBasename(path)) return;
      const nextPath = joinPath(pathDirname(path), nextName);
      await invoke('rename_path', { oldPath: path, newPath: nextPath });
      renameTabsForPath(path, nextPath);
      remapExpandedPathsForRename(path, nextPath);
      remapBookmarkPathsForRename(path, nextPath);
      if (!applyTreeRename(path, nextPath)) {
        await reloadWorkspaceTree();
      }
      scheduleGitTreeDecorations();
      treeSelectedPath.value = nextPath;
      treeRevealPath.value = nextPath;
      if (!isDir) await openFile(nextPath);
      pushNotice('success', `Renamed to: ${nextName}`);
      return;
    }
    if (action === 'change-type') {
      if (isDir) {
        pushNotice('warning', 'Change file type only supports files.');
        return;
      }
      const base = pathBasename(path);
      const idx = base.lastIndexOf('.');
      const currentExt = idx >= 0 ? base.slice(idx + 1) : '';
      const nextExt = (await openInputDialog({
        title: 'Change File Type',
        message: 'Enter new extension (without dot)',
        defaultValue: currentExt,
        confirmText: 'Change',
      }))?.trim().replace(/^\./, '');
      if (!nextExt) return;
      const stem = idx >= 0 ? base.slice(0, idx) : base;
      const nextPath = joinPath(pathDirname(path), `${stem}.${nextExt}`);
      await invoke('rename_path', { oldPath: path, newPath: nextPath });
      renameTabsForPath(path, nextPath);
      remapExpandedPathsForRename(path, nextPath);
      remapBookmarkPathsForRename(path, nextPath);
      if (!applyTreeRename(path, nextPath)) {
        await reloadWorkspaceTree();
      }
      scheduleGitTreeDecorations();
      treeSelectedPath.value = nextPath;
      treeRevealPath.value = nextPath;
      await openFile(nextPath);
      pushNotice('success', `Changed type to .${nextExt}`);
      return;
    }
    if (action === 'open-file-dir') {
      const target = isDir ? path : pathDirname(path);
      await invoke('open_path_in_file_manager', { path: target });
      return;
    }
    if (action === 'open-folder-dir') {
      const target = isDir ? path : pathDirname(path);
      await invoke('open_path_in_file_manager', { path: target });
      return;
    }
    if (action === 'copy') {
      treeClipboard.value = { mode: 'copy', path, isDir };
      pushNotice('info', `Copied: ${pathBasename(path)}`);
      return;
    }
    if (action === 'cut') {
      treeClipboard.value = { mode: 'cut', path, isDir };
      pushNotice('info', `Cut: ${pathBasename(path)}`);
      return;
    }
    if (action === 'paste') {
      if (!treeClipboard.value) {
        pushNotice('warning', 'Clipboard is empty.');
        return;
      }
      const dstBase = isDir ? path : pathDirname(path);
      const srcBaseName = pathBasename(treeClipboard.value.path);
      const targetPath = joinPath(dstBase, srcBaseName);
      const exists = await invoke<boolean>('path_exists', { path: targetPath });
      if (exists) {
        pushNotice('warning', 'Target already exists.');
        return;
      }
      const srcPath = treeClipboard.value.path;
      const srcIsDir = treeClipboard.value.isDir;
      const pastedWasFile = !srcIsDir;
      const isCut = treeClipboard.value.mode === 'cut';
      if (!isCut) {
        await invoke('copy_path', { src: srcPath, dst: targetPath });
        const entry: TreeNode = {
          name: pathBasename(targetPath),
          path: targetPath,
          isDir: srcIsDir,
          loaded: !srcIsDir,
        };
        if (!mergeNewTreeEntry(dstBase, entry)) {
          await loadChildren(dstBase);
        }
      } else {
        await invoke('move_path', { src: srcPath, dst: targetPath });
        renameTabsForPath(srcPath, targetPath);
        remapExpandedPathsForRename(srcPath, targetPath);
        remapBookmarkPathsForRename(srcPath, targetPath);
        treeClipboard.value = null;
        const { next: treeWithout, extracted } = extractNodeByPath(treeNodes.value, srcPath);
        if (extracted) {
          treeNodes.value = treeWithout;
          const moved = remapSubtreeRoots(extracted, srcPath, targetPath);
          if (!mergeNewTreeEntry(dstBase, moved)) {
            await loadChildren(dstBase);
            await loadChildren(pathDirname(srcPath));
          }
        } else {
          await loadChildren(dstBase);
          await loadChildren(pathDirname(srcPath));
        }
      }
      expandedPaths.value.add(dstBase);
      expandedPaths.value = new Set(expandedPaths.value);
      scheduleGitTreeDecorations();
      treeSelectedPath.value = targetPath;
      treeRevealPath.value = targetPath;
      if (pastedWasFile) await openFile(targetPath);
      pushNotice('success', 'Pasted.');
      return;
    }
    if (action === 'copy-path') {
      try {
        await navigator.clipboard.writeText(path);
        pushNotice('success', 'Path copied.');
      } catch {
        pushNotice('warning', 'Clipboard write failed.');
      }
      return;
    }
    if (action === 'format') {
      if (isDir) {
        pushNotice('warning', 'Format code only supports files.');
        return;
      }
      if (activeFilePath.value !== path) await openFile(path);
      await nextTick();
      await Promise.resolve(activeEditorRef.value?.formatDocument?.(indentTabSize.value, indentInsertSpaces.value));
      await saveActiveFile(true);
      pushNotice('success', 'Formatted current file.');
      return;
    }
    if (action === 'bookmark') {
      if (treeBookmarks.value.has(path)) {
        treeBookmarks.value.delete(path);
      } else {
        treeBookmarks.value.add(path);
      }
      treeBookmarks.value = new Set(treeBookmarks.value);
      localStorage.setItem('ide.treeBookmarks', JSON.stringify(Array.from(treeBookmarks.value)));
      pushNotice('success', treeBookmarks.value.has(path) ? 'Bookmark added.' : 'Bookmark removed.');
      return;
    }
    if (action === 'git-diff') {
      if (isDir || !ensureGitReady('view diff')) return;
      await openTreeGitDiff(path, 'unstaged');
      return;
    }
    if (action === 'git-diff-staged') {
      if (isDir || !ensureGitReady('view diff')) return;
      await openTreeGitDiff(path, 'staged');
      return;
    }
    if (action === 'git-diff-head') {
      if (isDir || !ensureGitReady('view diff')) return;
      await openTreeGitDiff(path, 'head');
      return;
    }
    if (action === 'git-stage') {
      if (isDir || !ensureGitReady('stage file')) return;
      await invoke<string>('git_exec', { path: workspacePath.value, args: ['add', '--', path] });
      scheduleGitTreeDecorations();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      pushNotice('success', 'File staged.');
      return;
    }
    if (action === 'git-unstage') {
      if (isDir || !ensureGitReady('unstage file')) return;
      await invoke<string>('git_exec', { path: workspacePath.value, args: ['restore', '--staged', '--', path] });
      scheduleGitTreeDecorations();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      pushNotice('success', 'File unstaged.');
      return;
    }
    if (action === 'git-discard') {
      if (isDir || !ensureGitReady('roll back changes')) return;
      const kind = gitTreeDecorations.value[path];
      if (kind === 'ignored') {
        pushNotice('warning', 'Ignored files cannot be rolled back from this menu.');
        return;
      }
      const ok = await openConfirmDialog({
        title: 'Rollback',
        message:
          kind === 'untracked'
            ? `Delete untracked file "${pathBasename(path)}"? This cannot be undone.`
            : `Roll back all changes to "${pathBasename(path)}" (staged and unstaged)? This cannot be undone.`,
        confirmText: 'Rollback',
      });
      if (!ok) return;
      if (kind === 'untracked') {
        await invoke<string>('git_exec', { path: workspacePath.value, args: ['clean', '-f', '--', path] });
        closeTabsUnderPath(path);
        pruneBookmarksUnderPath(path);
        pruneExpandedForDeletedPath(path);
        if (!removeNodeFromTreeByPath(path)) {
          await reloadWorkspaceTree();
        }
      } else {
        await invoke<string>('git_exec', {
          path: workspacePath.value,
          args: ['restore', '--staged', '--worktree', '--', path],
        });
        await reloadOpenTabFromDisk(path);
      }
      scheduleGitTreeDecorations();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      pushNotice('success', 'Rollback completed.');
      return;
    }
    if (action === 'delete') {
      const ok = await openConfirmDialog({
        title: 'Delete',
        message: `Delete "${pathBasename(path)}"? This cannot be undone.`,
        confirmText: 'Delete',
      });
      if (!ok) return;
      await invoke('delete_path', { path });
      closeTabsUnderPath(path);
      if (treeSelectedPath.value) {
        const sel = treeSelectedPath.value.toLowerCase();
        const target = path.toLowerCase();
        if (sel === target || sel.startsWith(`${target}/`) || sel.startsWith(`${target}\\`)) {
          treeSelectedPath.value = null;
        }
      }
      pruneBookmarksUnderPath(path);
      pruneExpandedForDeletedPath(path);
      if (!removeNodeFromTreeByPath(path)) {
        await reloadWorkspaceTree();
      }
      scheduleGitTreeDecorations();
      pushNotice('success', 'Deleted.');
    }
  } catch (error) {
    appendLog(`Tree action failed (${action}): ${String(error)}`);
    pushNotice('error', `Tree action failed: ${action}`, 5200, String(error));
  }
};

const appendLog = (message: string) => {
  terminalLogs.value = [...terminalLogs.value.slice(-120), message];
};

const toggleWebviewDevtools = () => {
  void invoke('plugin:webview|internal_toggle_devtools').catch(() => {});
};

const openWorkspaceAt = async (selected: string) => {
  const nodes = await invoke<TreeNode[]>('list_dir', { path: selected });
  workspacePath.value = selected;
  treeNodes.value = nodes.map((node) => ({ ...node, loaded: !node.isDir }));
  expandedPaths.value = new Set([selected]);
  showProject.value = true;
  appendLog(`Opened folder: ${selected}`);
};

const openFolder = async () => {
  try {
    const selected = await open({ directory: true, multiple: false });
    if (!selected || typeof selected !== 'string') return;
    await openWorkspaceAt(selected);
  } catch (error) {
    appendLog(`Open folder failed: ${String(error)}`);
  }
};

type CliOpenEntry = { path: string; isDirectory: boolean };

const parentDirFromPath = (absPath: string) => {
  const m = absPath.match(/^(.+)[/\\][^/\\]+$/);
  return m ? m[1] : absPath;
};

const applyCliLaunchEntries = async (entries: CliOpenEntry[]) => {
  if (!entries.length) return;
  const dirs = entries.filter((e) => e.isDirectory);
  const files = entries.filter((e) => !e.isDirectory);
  try {
    if (dirs.length > 0) {
      await openWorkspaceAt(dirs[0].path);
    } else if (files.length > 0) {
      await openWorkspaceAt(parentDirFromPath(files[0].path));
    }
    for (const f of files) {
      await openFile(f.path);
    }
    for (const d of dirs.slice(1)) {
      appendLog(`Skipped extra folder (only first folder is opened as workspace): ${d.path}`);
    }
  } catch (error) {
    appendLog(`CLI open failed: ${String(error)}`);
    pushNotice('warning', 'Could not open path from terminal command.', 5200, String(error));
  }
};

const installCliToPath = async () => {
  try {
    const msg = await invoke<string>('install_cli_in_path');
    appendLog(msg.split('\n').join(' '));
    pushNotice('success', 'CLI launcher installed', 0, msg);
  } catch (error) {
    const text = String(error);
    appendLog(`Install CLI failed: ${text}`);
    pushNotice('error', 'Install CLI failed', 6200, text);
  }
};

const openFileDialog = async () => {
  try {
    const selected = await open({ directory: false, multiple: false });
    if (!selected || typeof selected !== 'string') return;
    await openFile(selected);
  } catch (error) {
    appendLog(`Open file failed: ${String(error)}`);
  }
};

watch(
  workspacePath,
  (path) => {
    (window as any).__IDE_WORKSPACE_PATH__ = path;
  },
  { immediate: true },
);

watch(
  [activeFilePath, activeFileContent],
  ([path, content]) => {
    (window as any).__IDE_ACTIVE_FILE_PATH__ = path;
    (window as any).__IDE_ACTIVE_FILE_CONTENT__ = content || '';
  },
  { immediate: true },
);

const scheduleGitTreeDecorations = () => {
  if (!workspacePath.value || !isGitRepo.value) {
    if (gitTreeDecorationsTimer != null) {
      clearTimeout(gitTreeDecorationsTimer);
      gitTreeDecorationsTimer = null;
    }
    gitTreeDecorations.value = {};
    return;
  }
  if (gitTreeDecorationsTimer != null) clearTimeout(gitTreeDecorationsTimer);
  gitTreeDecorationsTimer = window.setTimeout(() => {
    gitTreeDecorationsTimer = null;
    void fetchGitTreeDecorations();
  }, 900);
};

const fetchGitTreeDecorations = async () => {
  if (!workspacePath.value || !isGitRepo.value) {
    gitTreeDecorations.value = {};
    return;
  }
  // Coalesce bursts of triggers and avoid overlapping IPC calls.
  // This is intentionally conservative to keep `git` process spawns low.
  if ((fetchGitTreeDecorations as any)._inFlight) return;
  const now = Date.now();
  const lastAt = (fetchGitTreeDecorations as any)._lastAt ?? 0;
  const minInterval = 1200;
  if (now - lastAt < minInterval) {
    scheduleGitTreeDecorations();
    return;
  }
  (fetchGitTreeDecorations as any)._inFlight = true;
  try {
    const map = await invoke<Record<string, string>>('git_tree_decorations', {
      path: workspacePath.value,
    });
    gitTreeDecorations.value = map && typeof map === 'object' ? map : {};
  } catch {
    gitTreeDecorations.value = {};
  } finally {
    (fetchGitTreeDecorations as any)._lastAt = Date.now();
    (fetchGitTreeDecorations as any)._inFlight = false;
  }
};

const refreshGitRepoState = async () => {
  if (!workspacePath.value) {
    isGitRepo.value = false;
    return;
  }
  try {
    const state = await invoke<{ is_repo: boolean }>('git_overview', {
      path: workspacePath.value,
      limit: 1,
    });
    isGitRepo.value = !!state?.is_repo;
  } catch {
    isGitRepo.value = false;
  }
  if (isGitRepo.value) scheduleGitTreeDecorations();
};

watch(workspacePath, () => {
  void refreshGitRepoState();
}, { immediate: true });

watch([workspacePath, isGitRepo], () => {
  if (!workspacePath.value || !isGitRepo.value) {
    if (gitTreeDecorationsTimer != null) {
      clearTimeout(gitTreeDecorationsTimer);
      gitTreeDecorationsTimer = null;
    }
    gitTreeDecorations.value = {};
    return;
  }
  scheduleGitTreeDecorations();
});

const ensureGitReady = (actionLabel: string) => {
  if (!workspacePath.value) {
    const msg = `Cannot ${actionLabel}: no workspace opened.`;
    appendLog(msg);
    pushNotice('warning', 'Please open a folder first.');
    return false;
  }
  if (!isGitRepo.value) {
    const msg = `Cannot ${actionLabel}: current workspace is not a git repository.`;
    appendLog(msg);
    pushNotice('warning', 'Current folder is not a Git repository.');
    return false;
  }
  return true;
};

const restoreSession = async () => {
  try {
    const state = await invoke<any>('load_app_state');
    if (!state) return;

    if (state.workspacePath && typeof state.workspacePath === 'string') {
      workspacePath.value = state.workspacePath;
      const nodes = await invoke<TreeNode[]>('list_dir', { path: state.workspacePath });
      treeNodes.value = nodes.map((node) => ({ ...node, loaded: !node.isDir }));
      expandedPaths.value = new Set([state.workspacePath]);
    }

    if (typeof state.projectWidth === 'number') projectWidth.value = clamp(state.projectWidth, 180, 520);
    if (typeof state.aiWidth === 'number') aiWidth.value = clamp(state.aiWidth, 260, 560);
    if (typeof state.showProject === 'boolean') showProject.value = state.showProject;
    if (typeof state.showAI === 'boolean') showAI.value = state.showAI;
    // UX choice: terminal/git panel is user-invoked, do not auto-open on launch.
    showBottom.value = false;
    if (typeof state.treeFontSize === 'number') treeFontSize.value = clamp(state.treeFontSize, 10, 20);

    if (Array.isArray(state.openTabs)) {
      for (const p of state.openTabs) {
        if (typeof p === 'string' && p.length) {
          await openFile(p);
        }
      }
    }
    if (typeof state.activeFilePath === 'string') {
      activeFilePath.value = state.activeFilePath;
      activePrimaryPath.value = state.activeFilePath;
    }
    appendLog('Session restored.');
  } catch (error) {
    appendLog(`Restore session failed: ${String(error)}`);
  }
};

const persistStateNow = async () => {
  const payload = {
    workspacePath: workspacePath.value,
    openTabs: openTabs.value.map((t) => t.path),
    activeFilePath: activeFilePath.value,
    projectWidth: projectWidth.value,
    aiWidth: aiWidth.value,
    showProject: showProject.value,
    showAI: showAI.value,
    showBottom: showBottom.value,
    treeFontSize: treeFontSize.value,
  } satisfies PersistedState;

  try {
    await invoke('save_app_state', {
      state: {
        workspacePath: payload.workspacePath,
        openTabs: payload.openTabs,
        activeFilePath: payload.activeFilePath,
        projectWidth: payload.projectWidth,
        aiWidth: payload.aiWidth,
        showProject: payload.showProject,
        showAI: payload.showAI,
        showBottom: payload.showBottom,
        treeFontSize: payload.treeFontSize,
      },
    });
  } catch (error) {
    appendLog(`Persist state failed: ${String(error)}`);
  }
};

const schedulePersist = () => {
  if (persistTimer) window.clearTimeout(persistTimer);
  persistTimer = window.setTimeout(() => {
    void persistStateNow();
  }, 800);
};

const ensurePathInGroup = (group: 'primary' | 'secondary', path: string) => {
  const target = group === 'primary' ? primaryTabPaths : secondaryTabPaths;
  if (!target.value.includes(path)) {
    target.value = [...target.value, path];
  }
};

const activatePathInGroup = (group: 'primary' | 'secondary', path: string) => {
  ensurePathInGroup(group, path);
  if (group === 'primary') {
    activePrimaryPath.value = path;
  } else {
    activeSecondaryPath.value = path;
  }
  activeEditorGroup.value = group;
  activeFilePath.value = path;
  const tab = openTabs.value.find((t) => t.path === path);
  if (tab?.encoding) encoding.value = tab.encoding;
  if (tab?.lineEnding) lineEnding.value = tab.lineEnding;
};

const openFile = async (path: string) => {
  treeSelectedPath.value = path;
  const existing = openTabs.value.find((tab) => tab.path === path);
  if (existing) {
    if (existing.encoding) encoding.value = existing.encoding;
    activatePathInGroup(activeEditorGroup.value, path);
    return;
  }
  try {
    const fileEncoding = 'UTF-8';
    const isImage = isRasterImagePath(path);
    const content = isImage
      ? (await invoke<ImageDataUrlPayload>('read_image_data_url', { path })).dataUrl
      : await invoke<string>('read_file_with_encoding', { path, encoding: fileEncoding });
    const name = path.split(/[/\\]/).pop() || 'Untitled';
    openTabs.value.push({
      path,
      name,
      content,
      originalContent: content,
      encoding: fileEncoding,
      lineEnding: isImage ? 'LF' : detectLineEnding(content),
      isBinaryImage: isImage,
    });
    encoding.value = fileEncoding;
    lineEnding.value = isImage ? 'LF' : detectLineEnding(content);
    activatePathInGroup(activeEditorGroup.value, path);
    appendLog(`Opened file: ${path}`);
  } catch (error) {
    appendLog(`Open file failed: ${String(error)}`);
  }
};

const findTreeNodeByPath = (path: string) => {
  const stack = [...treeNodes.value];
  while (stack.length) {
    const node = stack.pop()!;
    if (node.path === path) return node;
    if (node.children?.length) stack.push(...node.children);
  }
  return null;
};

const saveActiveFile = async (silent = false) => {
  if (!activeTab.value) return;
  if (!isDirty.value) return;
  try {
    await invoke('write_file_with_encoding', {
      path: activeTab.value.path,
      content: activeTab.value.content,
      encoding: activeTab.value.encoding || 'UTF-8',
    });
    openTabs.value = openTabs.value.map((tab) =>
      tab.path === activeTab.value?.path
        ? { ...tab, originalContent: tab.content }
        : tab,
    );
    if (!silent) appendLog(`Saved file: ${activeTab.value.path}`);
    scheduleGitTreeDecorations();
  } catch (error) {
    appendLog(`Save file failed: ${String(error)}`);
  }
};

const updateActiveContent = (content: string) => {
  if (!activeFilePath.value) return;
  openTabs.value = openTabs.value.map((tab) =>
    tab.path === activeFilePath.value ? { ...tab, content } : tab,
  );
};

const updateContentForGroup = (group: 'primary' | 'secondary', content: string) => {
  const path = group === 'secondary' ? activeSecondaryPath.value : activePrimaryPath.value;
  if (!path) return;
  activeEditorGroup.value = group;
  activeFilePath.value = path;
  openTabs.value = openTabs.value.map((tab) =>
    tab.path === path ? { ...tab, content } : tab,
  );
};

const activateTab = (path: string, group: 'primary' | 'secondary' = activeEditorGroup.value) => {
  activatePathInGroup(group, path);
};

const reorderTabsInGroup = (payload: {
  fromPath: string;
  insertIdx: number;
  group: 'primary' | 'secondary';
}) => {
  const target = payload.group === 'primary' ? primaryTabPaths : secondaryTabPaths;
  const paths = [...target.value];
  const fromIdx = paths.indexOf(payload.fromPath);
  if (fromIdx < 0) return;
  const [moved] = paths.splice(fromIdx, 1);
  const maxPos = paths.length;
  const idx = Math.max(0, Math.min(payload.insertIdx, maxPos));
  paths.splice(idx, 0, moved);
  target.value = paths;
  schedulePersist();
};

const focusEditorGroup = (group: 'primary' | 'secondary') => {
  activeEditorGroup.value = group;
  activeFilePath.value = group === 'secondary'
    ? (activeSecondaryPath.value ?? activePrimaryPath.value)
    : (activePrimaryPath.value ?? activeSecondaryPath.value);
};

const normalizeSplitState = () => {
  // Cursor-like behavior:
  // if primary group becomes empty while secondary still has tabs,
  // promote secondary to primary and collapse split view.
  if (primaryTabPaths.value.length === 0 && secondaryTabPaths.value.length > 0) {
    primaryTabPaths.value = [...secondaryTabPaths.value];
    activePrimaryPath.value = activeSecondaryPath.value ?? primaryTabPaths.value[0] ?? null;
    secondaryTabPaths.value = [];
    activeSecondaryPath.value = null;
    splitRightEnabled.value = false;
    activeEditorGroup.value = 'primary';
    activeFilePath.value = activePrimaryPath.value;
    return;
  }

  if (splitRightEnabled.value && secondaryTabPaths.value.length === 0) {
    splitRightEnabled.value = false;
    activeSecondaryPath.value = null;
    activeEditorGroup.value = 'primary';
  }
  activeFilePath.value = activeEditorGroup.value === 'secondary'
    ? (activeSecondaryPath.value ?? activePrimaryPath.value)
    : (activePrimaryPath.value ?? activeSecondaryPath.value);
};

const moveTabToNextGroup = (path: string) => {
  const inSecondary = secondaryTabPaths.value.includes(path);
  const inPrimary = primaryTabPaths.value.includes(path);
  if (!inPrimary && !inSecondary) return;

  if (!splitRightEnabled.value) splitRightEnabled.value = true;

  if (inPrimary) {
    primaryTabPaths.value = primaryTabPaths.value.filter((p) => p !== path);
    ensurePathInGroup('secondary', path);
    activeSecondaryPath.value = path;
    if (activePrimaryPath.value === path) {
      activePrimaryPath.value = primaryTabPaths.value[0] ?? null;
    }
    activeEditorGroup.value = 'secondary';
  } else {
    secondaryTabPaths.value = secondaryTabPaths.value.filter((p) => p !== path);
    ensurePathInGroup('primary', path);
    activePrimaryPath.value = path;
    if (activeSecondaryPath.value === path) {
      activeSecondaryPath.value = secondaryTabPaths.value[0] ?? null;
    }
    activeEditorGroup.value = 'primary';
  }
  normalizeSplitState();
};

const closeActiveGroup = () => {
  if (activeEditorGroup.value === 'secondary' && splitRightEnabled.value) {
    const remove = new Set(secondaryTabPaths.value);
    openTabs.value = openTabs.value.filter((t) => !remove.has(t.path));
    secondaryTabPaths.value = [];
    activeSecondaryPath.value = null;
    normalizeSplitState();
    return;
  }

  // Closing primary group keeps app usable by promoting secondary group if any.
  if (splitRightEnabled.value && secondaryTabPaths.value.length > 0) {
    const remove = new Set(primaryTabPaths.value);
    openTabs.value = openTabs.value.filter((t) => !remove.has(t.path));
    primaryTabPaths.value = [...secondaryTabPaths.value];
    activePrimaryPath.value = activeSecondaryPath.value ?? primaryTabPaths.value[0] ?? null;
    secondaryTabPaths.value = [];
    activeSecondaryPath.value = null;
    splitRightEnabled.value = false;
    activeEditorGroup.value = 'primary';
    activeFilePath.value = activePrimaryPath.value;
    return;
  }

  closeAllTabs();
};

const closeTab = (path: string, group: 'primary' | 'secondary' | 'both' = 'both') => {
  if (!openTabs.value.some((tab) => tab.path === path)) return;
  const primaryIdx = primaryTabPaths.value.indexOf(path);
  const secondaryIdx = secondaryTabPaths.value.indexOf(path);
  const primaryWasActive = activePrimaryPath.value === path;
  const secondaryWasActive = activeSecondaryPath.value === path;

  if ((group === 'primary' || group === 'both') && primaryIdx >= 0) {
    const nextPrimary = primaryTabPaths.value[primaryIdx + 1] ?? primaryTabPaths.value[primaryIdx - 1] ?? null;
    primaryTabPaths.value = primaryTabPaths.value.filter((p) => p !== path);
    if (primaryWasActive) activePrimaryPath.value = nextPrimary;
  }
  if ((group === 'secondary' || group === 'both') && secondaryIdx >= 0) {
    const nextSecondary = secondaryTabPaths.value[secondaryIdx + 1] ?? secondaryTabPaths.value[secondaryIdx - 1] ?? null;
    secondaryTabPaths.value = secondaryTabPaths.value.filter((p) => p !== path);
    if (secondaryWasActive) activeSecondaryPath.value = nextSecondary;
  }

  // Only remove model when no editor group references this path anymore.
  const stillUsed = primaryTabPaths.value.includes(path) || secondaryTabPaths.value.includes(path);
  if (!stillUsed) {
    openTabs.value = openTabs.value.filter((tab) => tab.path !== path);
  }
  normalizeSplitState();
};

const closeCurrentTab = () => {
  const current = activeEditorGroup.value === 'secondary' ? activeSecondaryPath.value : activePrimaryPath.value;
  if (!current) return;
  closeTab(current, activeEditorGroup.value);
};

const closeOtherTabs = (path: string) => {
  if (activeEditorGroup.value === 'secondary' && splitRightEnabled.value) {
    secondaryTabPaths.value = secondaryTabPaths.value.filter((p) => p === path);
    activeSecondaryPath.value = secondaryTabPaths.value[0] ?? null;
  } else {
    primaryTabPaths.value = primaryTabPaths.value.filter((p) => p === path);
    activePrimaryPath.value = primaryTabPaths.value[0] ?? null;
  }
  const keep = new Set([...primaryTabPaths.value, ...secondaryTabPaths.value]);
  openTabs.value = openTabs.value.filter((t) => keep.has(t.path));
  normalizeSplitState();
};

const closeAllTabs = () => {
  openTabs.value = [];
  primaryTabPaths.value = [];
  secondaryTabPaths.value = [];
  activePrimaryPath.value = null;
  activeSecondaryPath.value = null;
  splitRightEnabled.value = false;
  activeEditorGroup.value = 'primary';
  activeFilePath.value = null;
};

const closeTabsToRight = (path: string) => {
  const source = activeEditorGroup.value === 'secondary' && splitRightEnabled.value
    ? secondaryTabPaths.value
    : primaryTabPaths.value;
  const idx = source.findIndex((p) => p === path);
  if (idx < 0) return;
  const next = source.slice(0, idx + 1);
  if (activeEditorGroup.value === 'secondary' && splitRightEnabled.value) {
    secondaryTabPaths.value = next;
    if (!activeSecondaryPath.value || !next.includes(activeSecondaryPath.value)) activeSecondaryPath.value = path;
  } else {
    primaryTabPaths.value = next;
    if (!activePrimaryPath.value || !next.includes(activePrimaryPath.value)) activePrimaryPath.value = path;
  }
  const keep = new Set([...primaryTabPaths.value, ...secondaryTabPaths.value]);
  openTabs.value = openTabs.value.filter((t) => keep.has(t.path));
  normalizeSplitState();
};

const closeSavedTabs = () => {
  const unsaved = new Set(openTabs.value.filter((t) => t.content !== t.originalContent).map((t) => t.path));
  openTabs.value = openTabs.value.filter((t) => unsaved.has(t.path));
  primaryTabPaths.value = primaryTabPaths.value.filter((p) => unsaved.has(p));
  secondaryTabPaths.value = secondaryTabPaths.value.filter((p) => unsaved.has(p));
  if (activePrimaryPath.value && !unsaved.has(activePrimaryPath.value)) activePrimaryPath.value = primaryTabPaths.value[0] ?? null;
  if (activeSecondaryPath.value && !unsaved.has(activeSecondaryPath.value)) activeSecondaryPath.value = secondaryTabPaths.value[0] ?? null;
  normalizeSplitState();
};

const toWorkspaceRelativePath = (path: string) => {
  if (!workspacePath.value) return path;
  const wp = workspacePath.value.replaceAll('\\', '/').replace(/\/+$/, '');
  const p = path.replaceAll('\\', '/');
  if (!p.toLowerCase().startsWith(`${wp.toLowerCase()}/`)) return path;
  return p.slice(wp.length + 1);
};

const handleTabAction = async (payload: {
  action:
    | 'close'
    | 'close-others'
    | 'close-all'
    | 'close-saved'
    | 'close-right'
    | 'split-right'
    | 'move-to-next-group'
    | 'close-group'
    | 'copy-path'
    | 'copy-real-path'
    | 'reveal-in-folder';
  path: string;
}) => {
  if (secondaryTabPaths.value.includes(payload.path)) {
    activeEditorGroup.value = 'secondary';
    activeSecondaryPath.value = payload.path;
    activeFilePath.value = payload.path;
  } else if (primaryTabPaths.value.includes(payload.path)) {
    activeEditorGroup.value = 'primary';
    activePrimaryPath.value = payload.path;
    activeFilePath.value = payload.path;
  }
  if (payload.action === 'close') {
    closeTab(payload.path, activeEditorGroup.value);
    return;
  }
  if (payload.action === 'close-others') {
    closeOtherTabs(payload.path);
    return;
  }
  if (payload.action === 'close-all') {
    closeAllTabs();
    return;
  }
  if (payload.action === 'close-right') {
    closeTabsToRight(payload.path);
    return;
  }
  if (payload.action === 'close-saved') {
    closeSavedTabs();
    return;
  }
  if (payload.action === 'split-right') {
    splitRightEnabled.value = true;
    ensurePathInGroup('secondary', payload.path);
    activatePathInGroup('secondary', payload.path);
    return;
  }
  if (payload.action === 'move-to-next-group') {
    moveTabToNextGroup(payload.path);
    return;
  }
  if (payload.action === 'close-group') {
    closeActiveGroup();
    return;
  }
  if (payload.action === 'copy-path') {
    await navigator.clipboard.writeText(toWorkspaceRelativePath(payload.path));
    appendLog(`Copied path: ${toWorkspaceRelativePath(payload.path)}`);
    return;
  }
  if (payload.action === 'copy-real-path') {
    await navigator.clipboard.writeText(payload.path);
    appendLog(`Copied full path: ${payload.path}`);
    return;
  }
  if (payload.action === 'reveal-in-folder') {
    try {
      await invoke('reveal_in_folder', { path: payload.path });
    } catch (error) {
      appendLog(`Reveal file failed: ${String(error)}`);
    }
  }
};

const onCloseTabFromGroup = (payload: { path: string; group: 'primary' | 'secondary' }) => {
  closeTab(payload.path, payload.group);
};

const handleMenuAction = async (action: string) => {
  if (action === 'open-file') {
    await openFileDialog();
    return;
  }
  if (action === 'open-folder') {
    await openFolder();
    return;
  }
  if (action === 'save-file') {
    await saveActiveFile();
    return;
  }
  if (action === 'save-all') {
    // Current implementation is single-file editing; treat as save active
    await saveActiveFile();
    return;
  }
  if (action === 'open-settings') {
    showSettings.value = true;
    return;
  }
  if (action === 'view-toggle-explorer') {
    showProject.value = !showProject.value;
    return;
  }
  if (action === 'view-format-code') {
    await Promise.resolve(activeEditorRef.value?.formatDocument?.(indentTabSize.value, indentInsertSpaces.value));
    await saveActiveFile(true);
    return;
  }
  if (action === 'view-toggle-ai') {
    showAI.value = !showAI.value;
    return;
  }
  if (action === 'view-toggle-terminal') {
    showBottom.value = !showBottom.value;
    return;
  }
  if (action === 'go-to-file') {
    showToolPanel('search');
    return;
  }
  if (action === 'edit-undo') {
    runEditAction('undo');
    return;
  }
  if (action === 'edit-redo') {
    runEditAction('redo');
    return;
  }
  if (action === 'edit-cut') {
    runEditAction('cut');
    return;
  }
  if (action === 'edit-copy') {
    runEditAction('copy');
    return;
  }
  if (action === 'edit-paste') {
    runEditAction('paste');
    return;
  }
  if (action === 'edit-find') {
    runEditAction('find');
    return;
  }
  if (action === 'selection-select-all') {
    runEditAction('selectAll');
    return;
  }
  if (action === 'go-next-tab') {
    if (!openTabs.value.length || !activeFilePath.value) return;
    const idx = openTabs.value.findIndex((tab) => tab.path === activeFilePath.value);
    const next = openTabs.value[(idx + 1) % openTabs.value.length];
    activeFilePath.value = next.path;
    return;
  }
  if (action === 'go-prev-tab') {
    if (!openTabs.value.length || !activeFilePath.value) return;
    const idx = openTabs.value.findIndex((tab) => tab.path === activeFilePath.value);
    const prev = openTabs.value[(idx - 1 + openTabs.value.length) % openTabs.value.length];
    activeFilePath.value = prev.path;
    return;
  }
  if (action === 'run-project') {
    appendLog('Run Project triggered from menu');
    showBottom.value = true;
    return;
  }
  if (action === 'run-debug') {
    appendLog('Start Debugging triggered from menu');
    showBottom.value = true;
    return;
  }
  if (action === 'terminal-toggle') {
    bottomTool.value = 'terminal';
    showBottom.value = !showBottom.value;
    if (showBottom.value) {
      await Promise.resolve();
      terminalRef.value?.focusActive?.();
    }
    return;
  }
  if (action === 'terminal-clear') {
    bottomTool.value = 'terminal';
    terminalRef.value?.clearActive?.();
    return;
  }
  if (action === 'terminal-new') {
    bottomTool.value = 'terminal';
    showBottom.value = true;
    await Promise.resolve();
    terminalRef.value?.createTerminal?.();
    return;
  }
  if (action === 'terminal-kill-active') {
    bottomTool.value = 'terminal';
    await Promise.resolve();
    terminalRef.value?.killActive?.();
    return;
  }
  if (action === 'terminal-focus') {
    bottomTool.value = 'terminal';
    showBottom.value = true;
    await Promise.resolve();
    terminalRef.value?.focusActive?.();
    return;
  }
  if (action === 'terminal-rename') {
    bottomTool.value = 'terminal';
    showBottom.value = true;
    await Promise.resolve();
    terminalRef.value?.renameActive?.();
    return;
  }
  if (action === 'git-open') {
    bottomTool.value = 'git';
    showBottom.value = true;
    await nextTick();
    gitPanelRef.value?.setSection('head');
    await Promise.resolve(gitPanelRef.value?.refresh?.());
    return;
  }
  if (action === 'git-refresh') {
    bottomTool.value = 'git';
    showBottom.value = true;
    await nextTick();
    await Promise.resolve(gitPanelRef.value?.refresh?.());
    scheduleGitTreeDecorations();
    return;
  }
  if (action === 'git-log-head') {
    bottomTool.value = 'git';
    showBottom.value = true;
    await nextTick();
    gitPanelRef.value?.setSection('head');
    await Promise.resolve(gitPanelRef.value?.refresh?.());
    return;
  }
  if (action === 'git-show-local') {
    bottomTool.value = 'git';
    showBottom.value = true;
    await nextTick();
    gitPanelRef.value?.setSection('local');
    await Promise.resolve(gitPanelRef.value?.refresh?.());
    return;
  }
  if (action === 'git-show-remote') {
    bottomTool.value = 'git';
    showBottom.value = true;
    await nextTick();
    gitPanelRef.value?.setSection('remote');
    await Promise.resolve(gitPanelRef.value?.refresh?.());
    return;
  }
  if (action === 'git-show-tags') {
    bottomTool.value = 'git';
    showBottom.value = true;
    await nextTick();
    gitPanelRef.value?.setSection('tags');
    await Promise.resolve(gitPanelRef.value?.refresh?.());
    return;
  }
  if (action === 'git-fetch') {
    if (!workspacePath.value) return;
    try {
      const out = await invoke<string>('git_fetch', { path: workspacePath.value });
      appendLog(out.trim() || 'Git fetch completed.');
      pushNotice('success', 'Git fetch completed.', 2600);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
    } catch (e) {
      appendLog(`Git fetch failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-pull') {
    if (!workspacePath.value) return;
    try {
      const out = await invoke<string>('git_pull', { path: workspacePath.value });
      appendLog(out.trim() || 'Git pull completed.');
      pushNotice('success', 'Git pull completed.', 2600);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Git pull failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-push') {
    if (!ensureGitReady('push')) return;
    try {
      const out = await invoke<string>('git_push', { path: workspacePath.value });
      appendLog(out.trim() || 'Git push completed.');
      pushNotice('success', 'Git push completed.', 2600);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Git push failed: ${String(e)}`);
      pushNotice('error', 'Git push failed.', 5200, String(e));
    }
    return;
  }
  if (action === 'git-commit') {
    await openGitCommitDialog();
    return;
  }
  if (action === 'git-stash-save') {
    if (!workspacePath.value) return;
    const message = ((await openInputDialog({
      title: 'Stash Changes',
      message: 'Stash message (optional)',
      defaultValue: 'WIP',
      confirmText: 'Stash',
    })) || 'WIP').trim() || 'WIP';
    try {
      const out = await invoke<string>('git_stash_save', { path: workspacePath.value, message });
      appendLog(out.trim() || 'Stashed.');
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Stash failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-stash-pop') {
    if (!workspacePath.value) return;
    try {
      const out = await invoke<string>('git_stash_pop', { path: workspacePath.value });
      appendLog(out.trim() || 'Unstashed.');
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Unstash failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-stash-list') {
    if (!workspacePath.value) return;
    try {
      const list = await invoke<string[]>('git_stash_list', { path: workspacePath.value });
      appendLog(list.length ? `Stashes:\n${list.join('\n')}` : 'No stashes.');
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
    } catch (e) {
      appendLog(`List stashes failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-branches') {
    if (!workspacePath.value) return;
    try {
      const listRaw = await invoke<string>('git_exec', {
        path: workspacePath.value,
        args: ['branch', '--format=%(refname:short)'],
      });
      const branches = listRaw
        .split('\n')
        .map((s) => s.trim())
        .filter((s) => !!s);
      if (!branches.length) {
        appendLog('No local branches found.');
      } else {
        const selected = await openSelectDialog({
          title: 'Checkout Branch',
          message: 'Select branch to checkout',
          options: branches.map((b) => ({ label: b, value: b })),
          defaultValue: branches[0] ?? '',
          confirmText: 'Checkout',
        });
        if (selected && branches.includes(selected.trim())) {
          const out = await invoke<string>('git_exec', {
            path: workspacePath.value,
            args: ['checkout', selected.trim()],
          });
          appendLog(out.trim() || `Checked out: ${selected.trim()}`);
          scheduleGitTreeDecorations();
        }
      }
    } catch (e) {
      appendLog(`Load branches failed: ${String(e)}`);
    }
    bottomTool.value = 'git';
    showBottom.value = true;
    await nextTick();
    gitPanelRef.value?.setSection('local');
    await Promise.resolve(gitPanelRef.value?.refresh?.());
    return;
  }
  if (action === 'git-status') {
    if (!workspacePath.value) return;
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['status', '--porcelain=v1', '-b'] });
      appendLog(out.trim() ? `git status:\n${out.trim()}` : 'Working tree clean.');
    } catch (e) {
      appendLog(`git status failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-diff') {
    if (!workspacePath.value) return;
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['diff'] });
      appendLog(out.trim() ? `git diff:\n${out.trim()}` : 'No local changes.');
    } catch (e) {
      appendLog(`git diff failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-checkout') {
    if (!workspacePath.value) return;
    const target = await openInputDialog({
      title: 'Checkout Branch',
      message: 'Checkout branch (name)',
      defaultValue: '',
      confirmText: 'Checkout',
    });
    if (!target) return;
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['checkout', target.trim()] });
      appendLog(out.trim() || `Checked out: ${target}`);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Checkout failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-branch-create') {
    if (!workspacePath.value) return;
    const name = await openInputDialog({
      title: 'Create Branch',
      message: 'New branch name',
      defaultValue: '',
      confirmText: 'Next',
    });
    if (!name) return;
    const checkout = await openConfirmDialog({
      title: 'Create Branch',
      message: 'Checkout new branch now?',
      confirmText: 'Checkout',
      cancelText: 'Create Only',
    });
    try {
      const args = checkout ? ['checkout', '-b', name.trim()] : ['branch', name.trim()];
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args });
      appendLog(out.trim() || (checkout ? `Created & checked out: ${name}` : `Created branch: ${name}`));
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Create branch failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-merge') {
    if (!workspacePath.value) return;
    const from = await openInputDialog({
      title: 'Merge Branch',
      message: 'Merge branch into current (name)',
      defaultValue: '',
      confirmText: 'Merge',
    });
    if (!from) return;
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['merge', '--no-edit', from.trim()] });
      appendLog(out.trim() || `Merged: ${from}`);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Merge failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-rebase') {
    if (!workspacePath.value) return;
    const onto = await openInputDialog({
      title: 'Rebase',
      message: 'Rebase current branch onto (branch name)',
      defaultValue: '',
      confirmText: 'Rebase',
    });
    if (!onto) return;
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['rebase', onto.trim()] });
      appendLog(out.trim() || `Rebased onto: ${onto}`);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Rebase failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-cherry-pick') {
    if (!workspacePath.value) return;
    const sha = await openInputDialog({
      title: 'Cherry-pick',
      message: 'Commit SHA',
      defaultValue: '',
      confirmText: 'Cherry-pick',
    });
    if (!sha) return;
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['cherry-pick', sha.trim()] });
      appendLog(out.trim() || `Cherry-picked: ${sha}`);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Cherry-pick failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-revert') {
    if (!workspacePath.value) return;
    const sha = await openInputDialog({
      title: 'Revert Commit',
      message: 'Commit SHA',
      defaultValue: '',
      confirmText: 'Revert',
    });
    if (!sha) return;
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['revert', '--no-edit', sha.trim()] });
      appendLog(out.trim() || `Reverted: ${sha}`);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Revert failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'git-reset') {
    if (!workspacePath.value) return;
    const modeInput = await openSelectDialog({
      title: 'Reset',
      message: 'Select reset mode',
      options: [
        { label: 'soft', value: 'soft' },
        { label: 'mixed', value: 'mixed' },
        { label: 'hard', value: 'hard' },
      ],
      defaultValue: 'mixed',
      confirmText: 'Next',
    });
    const mode = (modeInput || 'mixed').trim().toLowerCase();
    if (!['soft', 'mixed', 'hard'].includes(mode)) return;
    const target = await openInputDialog({
      title: 'Reset',
      message: 'Reset target (e.g. HEAD~1, <sha>)',
      defaultValue: 'HEAD~1',
      confirmText: 'Reset',
    });
    if (!target) return;
    if (mode === 'hard') {
      const ok = await openConfirmDialog({
        title: 'Hard Reset',
        message: 'This will discard local changes. Continue?',
        confirmText: 'Continue',
      });
      if (!ok) return;
    }
    try {
      const out = await invoke<string>('git_exec', { path: workspacePath.value, args: ['reset', `--${mode}`, target.trim()] });
      appendLog(out.trim() || `Reset --${mode} ${target}`);
      bottomTool.value = 'git';
      showBottom.value = true;
      await nextTick();
      await Promise.resolve(gitPanelRef.value?.refresh?.());
      scheduleGitTreeDecorations();
    } catch (e) {
      appendLog(`Reset failed: ${String(e)}`);
    }
    return;
  }
  if (action === 'help-about') {
    showHelpAbout.value = true;
    return;
  }
  if (action === 'help-toggle-devtools') {
    toggleWebviewDevtools();
    return;
  }
  if (action === 'help-shortcuts') {
    showHelpShortcuts.value = true;
    return;
  }
  if (action === 'help-install-cli') {
    await installCliToPath();
    return;
  }
  appendLog(`Menu action: ${action}`);
};

const openGitCommitDialog = async () => {
  if (!workspacePath.value) return;
  showGitCommitDialog.value = true;
  gitCommitError.value = '';
  gitCommitLoading.value = false;
  if (!gitCommitMessage.value.trim()) {
    gitCommitMessage.value = '';
  }
  try {
    const out = await invoke<string>('git_exec', {
      path: workspacePath.value,
      args: ['status', '--porcelain=v1'],
    });
    const previousSelection = new Map(gitCommitChanges.value.map((c) => [c.path, c.selected]));
    gitCommitChanges.value = out
      .split('\n')
      .map((line) => line.trimEnd())
      .filter((line) => !!line)
      .map((line) => {
        const code = line.slice(0, 2);
        const rawPath = line.slice(3).trim();
        const path = rawPath.includes(' -> ') ? rawPath.split(' -> ').pop()!.trim() : rawPath;
        return {
          path,
          code,
          selected: previousSelection.get(path) ?? true,
          untracked: code === '??',
        } satisfies GitChangeItem;
      });
    scheduleGitTreeDecorations();
  } catch (e) {
    gitCommitChanges.value = [];
    gitCommitError.value = String(e);
  }
};

const closeGitCommitDialog = () => {
  showGitCommitDialog.value = false;
  gitCommitError.value = '';
};

/** Run after the commit dialog is hidden so overlay removal and Git/tree updates are not in the same frame. */
const queuePostCommitRefresh = () => {
  requestAnimationFrame(() => {
    void Promise.resolve(gitPanelRef.value?.refresh?.());
    scheduleGitTreeDecorations();
  });
};

const submitGitCommit = async () => {
  if (!workspacePath.value) return;
  const message = gitCommitMessage.value.trim();
  if (!message) {
    gitCommitError.value = 'Commit message is required.';
    return;
  }
  gitCommitLoading.value = true;
  gitCommitError.value = '';
  try {
    const selected = gitCommitChanges.value.filter((c) => c.selected).map((c) => c.path);
    if (!selected.length && !gitCommitAmend.value) {
      gitCommitError.value = 'Please select at least one changed file.';
      return;
    }
    if (selected.length) {
      await invoke<string>('git_exec', {
        path: workspacePath.value,
        args: ['add', '--', ...selected],
      });
    }
    const args = gitCommitAmend.value
      ? ['commit', '--amend', '-m', message]
      : ['commit', '-m', message, ...(selected.length ? ['--', ...selected] : [])];
    const out = await invoke<string>('git_exec', {
      path: workspacePath.value,
      args,
    });
    appendLog(out.trim() || 'Commit created.');
    pushNotice('success', 'Commit created.', 2600);
    closeGitCommitDialog();
    gitCommitMessage.value = '';
    gitCommitAmend.value = false;
    await nextTick();
    queuePostCommitRefresh();
  } catch (e) {
    gitCommitError.value = String(e);
  } finally {
    gitCommitLoading.value = false;
  }
};

const submitGitCommitAndPush = async () => {
  await submitGitCommit();
  if (gitCommitError.value) return;
  if (!ensureGitReady('push')) return;
  try {
    const out = await invoke<string>('git_push', { path: workspacePath.value });
    appendLog(out.trim() || 'Git push completed.');
    pushNotice('success', 'Git push completed.', 2600);
    await nextTick();
    queuePostCommitRefresh();
  } catch (e) {
    appendLog(`Git push failed: ${String(e)}`);
    pushNotice('error', 'Git push failed.', 5200, String(e));
  }
};

const toggleGitCommitChange = (path: string) => {
  gitCommitChanges.value = gitCommitChanges.value.map((c) =>
    c.path === path ? { ...c, selected: !c.selected } : c,
  );
};

const previewGitCommitDiff = async (path: string, fromDiffNav = false) => {
  if (!workspacePath.value) return;
  if (!fromDiffNav) {
    gitDiffNavFilesOverride.value = null;
  }
  gitCommitSelectedDiffPath.value = path;
  gitCommitDiffLoading.value = true;
  gitCommitDiffText.value = '';
  showGitDiffDialog.value = true;
  try {
    const out = await invoke<string>('git_exec', {
      path: workspacePath.value,
      args: ['diff', '--', path],
    });
    if (out.trim()) {
      gitCommitDiffText.value = out;
    } else {
      const staged = await invoke<string>('git_exec', {
        path: workspacePath.value,
        args: ['diff', '--cached', '--', path],
      });
      gitCommitDiffText.value = staged.trim() ? staged : 'No diff content available.';
    }
  } catch (e) {
    gitCommitDiffText.value = `Load diff failed: ${String(e)}`;
  } finally {
    gitCommitDiffLoading.value = false;
  }
};

const closeGitDiffDialog = () => {
  showGitDiffDialog.value = false;
  gitDiffNavFilesOverride.value = null;
};

type TreeGitDiffMode = 'unstaged' | 'staged' | 'head';

const openTreeGitDiff = async (path: string, mode: TreeGitDiffMode) => {
  if (!workspacePath.value || !ensureGitReady('view diff')) return;
  gitDiffNavFilesOverride.value = [path];
  gitCommitSelectedDiffPath.value = path;
  gitCommitDiffLoading.value = true;
  gitCommitDiffText.value = '';
  showGitDiffDialog.value = true;
  try {
    const args =
      mode === 'staged'
        ? ['diff', '--cached', '--', path]
        : mode === 'head'
          ? ['diff', 'HEAD', '--', path]
          : ['diff', '--', path];
    let out = await invoke<string>('git_exec', {
      path: workspacePath.value,
      args,
    });
    if (mode === 'unstaged' && !out.trim()) {
      out = await invoke<string>('git_exec', {
        path: workspacePath.value,
        args: ['diff', '--cached', '--', path],
      });
    }
    gitCommitDiffText.value = out.trim() ? out : 'No diff content available.';
  } catch (e) {
    gitCommitDiffText.value = `Load diff failed: ${String(e)}`;
  } finally {
    gitCommitDiffLoading.value = false;
  }
};

const reloadOpenTabFromDisk = async (path: string) => {
  const tab = openTabs.value.find((t) => t.path === path);
  if (!tab || tab.isBinaryImage) return;
  try {
    const content = await invoke<string>('read_file_with_encoding', {
      path,
      encoding: tab.encoding || 'UTF-8',
    });
    openTabs.value = openTabs.value.map((t) =>
      t.path === path
        ? { ...t, content, originalContent: content, lineEnding: detectLineEnding(content) }
        : t,
    );
  } catch {
    // File may have been removed; leave tab as-is.
  }
};

const previewPrevGitDiff = async () => {
  const idx = gitDiffDialogCurrentIndex.value;
  const files = gitDiffDialogFileList.value;
  if (idx <= 0) return;
  await previewGitCommitDiff(files[idx - 1], true);
};

const previewNextGitDiff = async () => {
  const idx = gitDiffDialogCurrentIndex.value;
  const files = gitDiffDialogFileList.value;
  if (idx < 0 || idx >= files.length - 1) return;
  await previewGitCommitDiff(files[idx + 1], true);
};

const selectAllGitCommitChanges = () => {
  gitCommitChanges.value = gitCommitChanges.value.map((c) => ({ ...c, selected: true }));
};

const clearGitCommitChangesSelection = () => {
  gitCommitChanges.value = gitCommitChanges.value.map((c) => ({ ...c, selected: false }));
};

const rollbackSelectedGitChanges = async () => {
  if (!workspacePath.value) return;
  const selected = gitCommitChanges.value.filter((c) => c.selected);
  if (!selected.length) return;
  const ok = await openConfirmDialog({
    title: 'Rollback Changes',
    message: 'Rollback selected changes? This cannot be undone.',
    confirmText: 'Rollback',
  });
  if (!ok) return;
  try {
    const tracked = selected.filter((c) => !c.untracked).map((c) => c.path);
    const untracked = selected.filter((c) => c.untracked).map((c) => c.path);
    if (tracked.length) {
      await invoke<string>('git_exec', {
        path: workspacePath.value,
        args: ['restore', '--staged', '--worktree', '--', ...tracked],
      });
    }
    if (untracked.length) {
      await invoke<string>('git_exec', {
        path: workspacePath.value,
        args: ['clean', '-f', '--', ...untracked],
      });
    }
    scheduleGitTreeDecorations();
    await openGitCommitDialog();
  } catch (e) {
    gitCommitError.value = String(e);
  }
};

const selectLeftTool = (tool: LeftTool) => {
  leftTool.value = tool;
  if (tool === 'search') {
    void runWorkspaceSearch();
  }
  if (tool === 'source') {
    bottomTool.value = 'git';
    showBottom.value = true;
  }
};

const openGitFromSidebar = async () => {
  if (bottomTool.value === 'git' && showBottom.value) {
    showBottom.value = false;
    return;
  }
  bottomTool.value = 'git';
  showBottom.value = true;
};

const openTerminalFromSidebar = async () => {
  if (bottomTool.value === 'terminal' && showBottom.value) {
    showBottom.value = false;
    return;
  }
  bottomTool.value = 'terminal';
  showBottom.value = true;
};

const showToolPanel = (tool: LeftTool) => {
  leftTool.value = tool;
  showProject.value = true;
  if (tool === 'search') {
    void runWorkspaceSearch();
  }
};

const activeEditorRef = computed(() =>
  activeEditorGroup.value === 'secondary' ? secondaryEditorRef.value : primaryEditorRef.value,
);

const runEditAction = async (action: 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'find' | 'selectAll' | 'format') => {
  await Promise.resolve(activeEditorRef.value?.runEditorAction(action));
};

const onEditorCursorChange = (pos: { line: number; col: number }) => {
  cursorLine.value = pos.line;
  cursorCol.value = pos.col;
};

const onEditorIndentChange = (indent: { tabSize: number; insertSpaces: boolean }) => {
  indentTabSize.value = indent.tabSize;
  indentInsertSpaces.value = indent.insertSpaces;
};

const performEncodingConversion = async (nextEncoding: string) => {
  if (!activeTab.value) return;
  const current = activeTab.value;
  if (current.isBinaryImage) {
    pushNotice('info', 'Binary image files do not support text encoding conversion.');
    return;
  }
  if (current.encoding === nextEncoding) return;
  try {
    await invoke('write_file_with_encoding', {
      path: current.path,
      content: current.content,
      encoding: nextEncoding,
    });
    const reloaded = await invoke<string>('read_file_with_encoding', {
      path: current.path,
      encoding: nextEncoding,
    });
    openTabs.value = openTabs.value.map((tab) => (tab.path === current.path
      ? {
        ...tab,
        content: reloaded,
        originalContent: reloaded,
        encoding: nextEncoding,
        lineEnding: detectLineEnding(reloaded),
      }
      : tab));
    encoding.value = nextEncoding;
    lineEnding.value = detectLineEnding(reloaded);
    appendLog(`Converted ${current.path} to ${nextEncoding}.`);
    pushNotice('success', `Encoding changed to ${nextEncoding}.`);
    scheduleGitTreeDecorations();
  } catch (error) {
    appendLog(`Encoding convert failed: ${String(error)}`);
    pushNotice('error', 'Encoding conversion failed.', 5200, String(error));
  }
};

const statusChangeEncoding = async (nextEncoding: string) => {
  if (!activeTab.value) {
    encoding.value = nextEncoding;
    appendLog(`Encoding switched to ${nextEncoding}.`);
    pushNotice('info', `Encoding switched to ${nextEncoding}.`);
    return;
  }
  const current = activeTab.value;
  if (current.encoding === nextEncoding) return;
  await performEncodingConversion(nextEncoding);
};

const statusChangeLineEnding = (next: 'LF' | 'CRLF') => {
  const current = activeTab.value;
  if (!current) {
    lineEnding.value = next;
    pushNotice('info', `Line ending switched to ${next}.`);
    return;
  }
  if (current.lineEnding === next) return;
  if (current.isBinaryImage) {
    pushNotice('info', 'Binary image files do not support line-ending conversion.');
    return;
  }
  activeEditorRef.value?.setLineEnding(next);
  lineEnding.value = next;
  openTabs.value = openTabs.value.map((tab) => (tab.path === current.path
    ? { ...tab, lineEnding: next }
    : tab));
  appendLog(`Line ending changed to ${next}: ${current.path}`);
  pushNotice('success', `Line ending changed to ${next}.`);
};

const statusGoToPosition = () => {
  appendLog(`Cursor: Line ${cursorLine.value}, Col ${cursorCol.value}`);
};

const statusChangeIndent = () => {
  settingsInitialSection.value = 'editor';
  showSettings.value = true;
};

const applyIndentFromSettings = () => {
  const tabSize = Math.max(1, Math.min(16, indentTabSize.value));
  activeEditorRef.value?.setIndentation?.(tabSize, indentInsertSpaces.value);
};

const statusToggleAiOnline = () => {
  void refreshAiConnectionStatus();
};

const refreshAiConnectionStatus = async () => {
  try {
    aiOnline.value = await invoke<boolean>('aihub_connection_status');
  } catch {
    aiOnline.value = false;
  }
};

const statusOpenNotifications = () => {
  if (notices.value.length === 0) {
    pushNotice('info', 'No notifications.');
    return;
  }
  notices.value = [];
  notificationsCount.value = 0;
};

const addSelectionToAiChat = () => {
  const selection = activeEditorRef.value?.getSelectionContext?.();
  if (!selection) {
    pushNotice('warning', 'No code selection to add.');
    return;
  }
  showAI.value = true;
  window.dispatchEvent(new CustomEvent('ai-add-code-ref', { detail: selection }));
  pushNotice('success', `Added ${selection.fileName}:${selection.startLine}-${selection.endLine} to AI chat.`, 1800);
};

const isInputLikeTarget = (target: EventTarget | null) => {
  const element = target as HTMLElement | null;
  if (!element) return false;
  const tag = element.tagName;
  if (tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT') return true;
  return element.isContentEditable;
};

const isInsideMonaco = (target: EventTarget | null) => {
  const element = target as HTMLElement | null;
  if (!element) return false;
  return !!element.closest('.monaco-editor');
};

const hasTextSelection = () => {
  const selection = window.getSelection();
  if (!selection) return false;
  return !selection.isCollapsed && !!selection.toString().trim();
};
const isInsideProjectTree = (target: EventTarget | null) => {
  const el = target as HTMLElement | null;
  return !!el?.closest('.project-tree');
};

const onWindowKeydown = (event: KeyboardEvent) => {
  if (matchesShortcut(event, 'Ctrl+Shift+D')) {
    event.preventDefault();
    event.stopPropagation();
    toggleWebviewDevtools();
    return;
  }
  if (isInputLikeTarget(event.target)) {
    const allowInInput = globalKeybindings.some((binding) =>
      (binding.command === 'menu.go-next-tab' || binding.command === 'menu.go-prev-tab')
      && matchesShortcut(event, binding.shortcut),
    );
    if (!allowInInput) return;
  }

  const inExplorerTree = showProject.value
    && leftTool.value === 'explorer'
    && !!treeSelectedPath.value
    && isInsideProjectTree(event.target);

  if (inExplorerTree && treeSelectedPath.value) {
    const selectedPath = treeSelectedPath.value;
    const selectedNode = findTreeNodeByPath(selectedPath);
    const isDir = !!selectedNode?.isDir;
    const targetAction = (action: string) => {
      void handleTreeAction({ action, path: selectedPath, isDir });
    };
    for (const binding of treeKeybindings) {
      if (!matchesShortcut(event, binding.shortcut)) continue;
      event.preventDefault();
      if (binding.command === 'tree.new-file') targetAction('new-file');
      else if (binding.command === 'tree.rename') targetAction('rename');
      else if (binding.command === 'tree.delete') targetAction('delete');
      else if (binding.command === 'tree.copy') targetAction('copy');
      else if (binding.command === 'tree.cut') targetAction('cut');
      else if (binding.command === 'tree.paste') targetAction('paste');
      else if (binding.command === 'tree.copy-path') targetAction('copy-path');
      else if (binding.command === 'tree.format') targetAction('format');
      else if (binding.command === 'tree.bookmark') targetAction('bookmark');
      return;
    }
  }
  for (const binding of globalKeybindings) {
    if (!matchesShortcut(event, binding.shortcut)) continue;
    if (binding.command === 'edit.copy' || binding.command === 'edit.cut') {
      if (!isInsideMonaco(event.target) && hasTextSelection()) return;
    }
    event.preventDefault();
    if (binding.command === 'menu.git-open') {
      void handleMenuAction('git-open');
      return;
    }
    if (binding.command === 'menu.go-to-file') {
      void handleMenuAction('go-to-file');
      return;
    }
    if (binding.command === 'menu.go-next-tab') {
      void handleMenuAction('go-next-tab');
      return;
    }
    if (binding.command === 'menu.go-prev-tab') {
      void handleMenuAction('go-prev-tab');
      return;
    }
    if (binding.command === 'menu.terminal-toggle') {
      void handleMenuAction('terminal-toggle');
      return;
    }
    if (binding.command === 'menu.git-push') {
      void handleMenuAction('git-push');
      return;
    }
    if (binding.command === 'menu.git-commit') {
      void handleMenuAction('git-commit');
      return;
    }
    if (binding.command === 'menu.git-pull') {
      void handleMenuAction('git-pull');
      return;
    }
    if (binding.command === 'terminal.new') {
      showBottom.value = true;
      void Promise.resolve().then(() => terminalRef.value?.createTerminal?.());
      return;
    }
    if (binding.command === 'terminal.toggle') {
      showBottom.value = !showBottom.value;
      if (showBottom.value) void Promise.resolve().then(() => terminalRef.value?.focusActive?.());
      return;
    }
    if (binding.command === 'tab.move-next-group') {
      if (activeFilePath.value) moveTabToNextGroup(activeFilePath.value);
      return;
    }
    if (binding.command === 'tab.close-group') {
      closeActiveGroup();
      return;
    }
    if (binding.command === 'tab.split-right') {
      if (activeFilePath.value) void handleTabAction({ action: 'split-right', path: activeFilePath.value });
      return;
    }
    if (binding.command === 'tab.close-all') {
      closeAllTabs();
      return;
    }
    if (binding.command === 'tab.close-others') {
      if (activeFilePath.value) closeOtherTabs(activeFilePath.value);
      return;
    }
    if (binding.command === 'tab.close-current') {
      closeCurrentTab();
      return;
    }
    if (binding.command === 'tab.close-saved') {
      closeSavedTabs();
      return;
    }
    if (binding.command === 'menu.view-format-code') {
      void handleMenuAction('view-format-code');
      return;
    }
    if (binding.command === 'ai.add-selection') {
      addSelectionToAiChat();
      return;
    }
    if (binding.command === 'app.save') {
      void saveActiveFile();
      return;
    }
    if (binding.command === 'app.open-file') {
      void openFileDialog();
      return;
    }
    if (binding.command === 'edit.find') return void runEditAction('find');
    if (binding.command === 'edit.undo') return void runEditAction('undo');
    if (binding.command === 'edit.redo') return void runEditAction('redo');
    if (binding.command === 'edit.cut') return void runEditAction('cut');
    if (binding.command === 'edit.copy') return void runEditAction('copy');
    if (binding.command === 'edit.paste') return void runEditAction('paste');
    if (binding.command === 'edit.select-all') return void runEditAction('selectAll');
    if (binding.command === 'settings.open') {
      showSettings.value = true;
      return;
    }
  }
};

const onAppWindowFocus = () => {
  if (workspacePath.value && isGitRepo.value) scheduleGitTreeDecorations();
};

onMounted(() => {
  window.addEventListener('keydown', onWindowKeydown, true);
  window.addEventListener('focus', onAppWindowFocus);
  const savedTheme = localStorage.getItem('ide.theme') as 'dark' | 'light' | 'monaco' | null;
  const savedFontSize = Number(localStorage.getItem('ide.editorFontSize') || 13);
  const savedTreeFontSize = Number(localStorage.getItem('ide.treeFontSize') || 12);
  const savedDelay = Number(localStorage.getItem('ide.autoSaveDelay') || 700);
  if (savedTheme && ['dark', 'light', 'monaco'].includes(savedTheme)) theme.value = savedTheme;
  editorFontSize.value = clamp(savedFontSize, 11, 24);
  treeFontSize.value = clamp(savedTreeFontSize, 10, 20);
  autoSaveDelay.value = clamp(savedDelay, 200, 5000);
  void (async () => {
    try {
      const raw = localStorage.getItem('ide.treeBookmarks');
      if (raw) {
        const list = JSON.parse(raw);
        if (Array.isArray(list)) treeBookmarks.value = new Set(list.filter((v) => typeof v === 'string'));
      }
    } catch {
      treeBookmarks.value = new Set();
    }
    await restoreSession();
    try {
      const pending = await invoke<CliOpenEntry[] | null>('take_cli_launch_paths');
      if (pending && pending.length > 0) {
        await applyCliLaunchEntries(pending);
      }
    } catch {
      // Not running inside Tauri (e.g. plain web dev).
    }
  })();

  // Memory indicator (best-effort; may be unavailable in WebView2)
  const updateMemory = () => {
    const anyPerf = performance as any;
    const mem = anyPerf?.memory;
    if (!mem || typeof mem.usedJSHeapSize !== 'number' || typeof mem.jsHeapSizeLimit !== 'number') {
      memoryText.value = '—';
      memoryPercent.value = 0;
      return;
    }
    const used = mem.usedJSHeapSize / (1024 * 1024);
    const limit = mem.jsHeapSizeLimit / (1024 * 1024);
    memoryText.value = `${Math.round(used)} / ${Math.round(limit)} MB`;
    memoryPercent.value = limit > 0 ? (used / limit) * 100 : 0;
  };
  updateMemory();
  memoryInterval = window.setInterval(updateMemory, 2000);
  void refreshAiConnectionStatus();
  aiStatusInterval = window.setInterval(() => {
    void refreshAiConnectionStatus();
  }, 15000);
});

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onWindowKeydown, true);
  window.removeEventListener('focus', onAppWindowFocus);
  if (gitTreeDecorationsTimer != null) {
    clearTimeout(gitTreeDecorationsTimer);
    gitTreeDecorationsTimer = null;
  }
  if (autoSaveTimer) window.clearTimeout(autoSaveTimer);
  if (searchTimer) window.clearTimeout(searchTimer);
  if (persistTimer) window.clearTimeout(persistTimer);
  if (memoryInterval) window.clearInterval(memoryInterval);
  if (aiStatusInterval) window.clearInterval(aiStatusInterval);
  void persistStateNow();
});

watch(
  () => [
    workspacePath.value,
    openTabs.value.map((t) => t.path).join('|'),
    activeFilePath.value,
    projectWidth.value,
    aiWidth.value,
    showProject.value,
    showAI.value,
    showBottom.value,
  ],
  () => {
    schedulePersist();
  },
);

watch(
  () => [activeTab.value?.path, activeTab.value?.content] as const,
  ([path]) => {
    if (!path || !isDirty.value) return;
    if (autoSaveTimer) window.clearTimeout(autoSaveTimer);
    autoSaveTimer = window.setTimeout(() => {
      void saveActiveFile(true);
    }, autoSaveDelay.value);
  },
);

watch(
  () => [searchKeyword.value, workspacePath.value, leftTool.value] as const,
  () => {
    if (leftTool.value !== 'search') return;
    if (searchTimer) window.clearTimeout(searchTimer);
    searchTimer = window.setTimeout(() => {
      void runWorkspaceSearch();
    }, 150);
  },
  { immediate: true },
);

watch(
  () => searchResults.value.length,
  (len) => {
    if (len <= 0) {
      searchSelectedIndex.value = -1;
      return;
    }
    if (searchSelectedIndex.value < 0) searchSelectedIndex.value = 0;
    if (searchSelectedIndex.value >= len) searchSelectedIndex.value = len - 1;
  },
);

watch(theme, (value) => {
  document.documentElement.setAttribute('data-theme', value);
  localStorage.setItem('ide.theme', value);
}, { immediate: true });

watch(editorFontSize, (value) => {
  const next = clamp(value, 11, 24);
  if (next !== value) editorFontSize.value = next;
  localStorage.setItem('ide.editorFontSize', String(next));
});

watch(treeFontSize, (value) => {
  const next = clamp(value, 10, 20);
  if (next !== value) treeFontSize.value = next;
  localStorage.setItem('ide.treeFontSize', String(next));
});

watch(autoSaveDelay, (value) => {
  const next = clamp(value, 200, 5000);
  if (next !== value) autoSaveDelay.value = next;
  localStorage.setItem('ide.autoSaveDelay', String(next));
});

watch(treeRevealPath, (path) => {
  if (!path) return;
  window.setTimeout(() => {
    if (treeRevealPath.value === path) treeRevealPath.value = null;
  }, 1800);
});
</script>

<template>
  <div
    class="ide-shell"
    :style="{
      '--ide-project-width': showProject ? `${projectWidth}px` : '0px',
      '--ide-project-splitter-width': showProject ? '4px' : '0px',
      '--ide-toolwindow-width': showAI ? `${aiWidth}px` : '0px',
      '--ide-toolwindow-splitter-width': showAI ? '4px' : '0px',
      '--ide-tree-font-size': `${treeFontSize}px`,
    }"
  >
    <TitleBar
      :has-workspace="!!workspacePath"
      :is-git-repo="isGitRepo"
      @menu-action="handleMenuAction"
    />

    <div class="ide-body">
      <div class="ide-body-main">
        <LeftSideBar
          v-model:active="showProject"
          :left-tool="leftTool"
          @select-left-tool="selectLeftTool"
          @open-git="openGitFromSidebar"
          @open-terminal="openTerminalFromSidebar"
        />

        <div class="ide-workspace">
        <ProjectTree
          v-if="showProject && leftTool === 'explorer'"
          :root-path="workspacePath"
          :nodes="treeNodes"
          :expanded-paths="expandedPathList"
          :selected-path="treeSelectedPath || activeFilePath"
          :bookmarks="Array.from(treeBookmarks)"
          :reveal-path="treeRevealPath"
          :git-enabled="isGitRepo"
          :git-decorations="gitTreeDecorations"
          @toggle-folder="toggleFolder"
          @open-file="openFile"
          @tree-action="handleTreeAction"
        />
        <section v-else-if="showProject" class="ide-left-panel">
          <header class="ide-panel-header">
            <span class="ide-panel-title">{{ leftTool }}</span>
          </header>

          <div class="ide-panel-content ide-scrollbar">
            <template v-if="leftTool === 'search'">
              <input
                v-model="searchKeyword"
                class="ide-search-input"
                type="text"
                placeholder="Search files by name..."
                @keydown="onSearchInputKeydown"
              >
              <div v-if="searchKeyword.trim() && searchResults.length === 0" class="ide-list-item">
                <div>No results</div>
                <div class="muted">Try another keyword</div>
              </div>
              <div
                v-for="(item, idx) in searchResults"
                :key="item.path"
                class="ide-list-item"
                :class="{ active: idx === searchSelectedIndex }"
                @mouseenter="searchSelectedIndex = idx"
                @click="openSearchResult(item.path, idx)"
              >
                <div v-html="highlightSearchText(item.name)" />
                <div class="muted" v-html="highlightSearchText(item.path)" />
              </div>
            </template>

            <template v-else-if="leftTool === 'source'">
              <div class="ide-list-item">
                <div>Opened Files</div>
                <div class="muted">{{ openTabs.length }}</div>
              </div>
              <div class="ide-list-item">
                <div>Unsaved Changes</div>
                <div class="muted">{{ openTabs.filter(t => t.content !== t.originalContent).length }}</div>
              </div>
            </template>

            <template v-else-if="leftTool === 'run'">
              <button class="ide-btn ide-btn--primary" type="button" @click="appendLog('Run started...')">Run Project</button>
              <button class="ide-btn" type="button" @click="appendLog('Debug started...')">Start Debug</button>
            </template>

            <template v-else-if="leftTool === 'extensions'">
              <div class="ide-list-item"><div>Vue Language Features</div><div class="muted">Enabled</div></div>
              <div class="ide-list-item"><div>TypeScript Tools</div><div class="muted">Enabled</div></div>
              <div class="ide-list-item"><div>Tauri Support</div><div class="muted">Enabled</div></div>
            </template>
          </div>
        </section>

        <div
          v-if="showProject"
          class="ide-splitter ide-splitter--vertical ide-splitter-project"
          title="Resize Project"
          @pointerdown="startProjectResize"
        />

        <section class="ide-center">
          <main class="ide-center__main">
            <div class="editor-groups" :class="{ split: splitRightEnabled }">
              <section class="editor-group">
                <EditorMain
                ref="primaryEditorRef"
                group-id="primary"
                :file-path="primaryFilePath"
                :tabs="primaryTabs"
                :active-path="primaryFilePath"
                :file-name="primaryFileName"
                :content="primaryContent"
                :dirty="primaryDirty"
                :editor-font-size="editorFontSize"
                :editor-theme="theme"
                :indent-tab-size="indentTabSize"
                :indent-insert-spaces="indentInsertSpaces"
                @focus-editor="focusEditorGroup('primary')"
                @update:content="updateContentForGroup('primary', $event)"
                @activate-tab="activateTab($event, 'primary')"
                @reorder-tabs="reorderTabsInGroup"
                @close-tab="onCloseTabFromGroup"
                @tab-action="handleTabAction"
                @cursor-change="onEditorCursorChange"
                @indent-change="onEditorIndentChange"
                />
              </section>
              <div v-if="splitRightEnabled" class="editor-group-splitter" />
              <section v-if="splitRightEnabled" class="editor-group">
                <EditorMain
                ref="secondaryEditorRef"
                group-id="secondary"
                :file-path="secondaryFilePath"
                :tabs="secondaryTabs"
                :active-path="secondaryFilePath"
                :file-name="secondaryFileName"
                :content="secondaryContent"
                :dirty="secondaryDirty"
                :editor-font-size="editorFontSize"
                :editor-theme="theme"
                :indent-tab-size="indentTabSize"
                :indent-insert-spaces="indentInsertSpaces"
                @focus-editor="focusEditorGroup('secondary')"
                @update:content="updateContentForGroup('secondary', $event)"
                @activate-tab="activateTab($event, 'secondary')"
                @reorder-tabs="reorderTabsInGroup"
                @close-tab="onCloseTabFromGroup"
                @tab-action="handleTabAction"
                @cursor-change="onEditorCursorChange"
                @indent-change="onEditorIndentChange"
                />
              </section>
            </div>
          </main>

          <TerminalPanel
            v-if="bottomTool === 'terminal'"
            class="ide-bottom-wide"
            ref="terminalRef"
            v-model:open="showBottom"
            :tree-font-size="treeFontSize"
            :theme="theme"
          />
          <GitPanel
            v-else
            class="ide-bottom-wide"
            ref="gitPanelRef"
            v-model:open="showBottom"
            :workspace-path="workspacePath"
            :tree-font-size="treeFontSize"
            @worktree-changed="scheduleGitTreeDecorations"
          />
        </section>
        </div>

        <RightSideBar v-model:active="showAI" />
      </div>

      <div
        v-if="showAI"
        class="ide-splitter ide-splitter--vertical ide-splitter-toolwindow ide-splitter-toolwindow-overlay"
        title="Resize Tool Window"
        @pointerdown="startAiResize"
      />
      <ToolWindow
        :open="showAI"
        title="AI Assistant"
        position="right"
        @close="showAI = false"
      />
    </div>

    <StatusBar
      :branch="gitBranch"
      :file-path="statusFilePath"
      :encoding="encoding"
      :line-ending="lineEnding"
      :line="cursorLine"
      :col="cursorCol"
      :indent-label="indentLabel"
      :memory-text="memoryText"
      :memory-percent="memoryPercent"
      :ai-online="aiOnline"
      :notifications-count="notificationsCount"
      @change-encoding="statusChangeEncoding"
      @change-line-ending="statusChangeLineEnding"
      @click-position="statusGoToPosition"
      @click-indent="statusChangeIndent"
      @toggle-ai="statusToggleAiOnline"
      @open-notifications="statusOpenNotifications"
    />
    <SettingsDialog
      :open="showSettings"
      :initial-section="settingsInitialSection"
      :theme="theme"
      :editor-font-size="editorFontSize"
      :tree-font-size="treeFontSize"
      :auto-save-delay="autoSaveDelay"
      :indent-tab-size="indentTabSize"
      :indent-insert-spaces="indentInsertSpaces"
      @close="showSettings = false"
      @update:theme="theme = $event"
      @update:editor-font-size="editorFontSize = $event"
      @update:tree-font-size="treeFontSize = $event"
      @update:auto-save-delay="autoSaveDelay = $event"
      @update:indent-tab-size="indentTabSize = $event; applyIndentFromSettings()"
      @update:indent-insert-spaces="indentInsertSpaces = $event; applyIndentFromSettings()"
      @install-cli="installCliToPath"
    />
    <GitCommitDialog
      :open="showGitCommitDialog"
      :message="gitCommitMessage"
      :loading="gitCommitLoading"
      :changes="gitCommitChanges"
      :error-text="gitCommitError"
      :amend="gitCommitAmend"
      @close="closeGitCommitDialog"
      @update:message="gitCommitMessage = $event"
      @update:amend="gitCommitAmend = $event"
      @toggle-change="toggleGitCommitChange"
      @select-all="selectAllGitCommitChanges"
      @select-none="clearGitCommitChangesSelection"
      @open-diff="previewGitCommitDiff"
      @refresh="openGitCommitDialog"
      @rollback-selected="rollbackSelectedGitChanges"
      @confirm="submitGitCommit"
      @confirm-and-push="submitGitCommitAndPush"
    />
    <GitDiffDialog
      :open="showGitDiffDialog"
      :path="gitCommitSelectedDiffPath"
      :loading="gitCommitDiffLoading"
      :diff-text="gitCommitDiffText"
      :files="gitDiffDialogFileList"
      :current-index="gitDiffDialogCurrentIndex"
      @close="closeGitDiffDialog"
      @prev="previewPrevGitDiff"
      @next="previewNextGitDiff"
    />
    <HelpAboutDialog :open="showHelpAbout" @close="showHelpAbout = false" />
    <HelpShortcutsDialog :open="showHelpShortcuts" :shortcuts="shortcutItems" @close="showHelpShortcuts = false" />
    <UiPromptDialog
      :open="promptDialogOpen"
      :mode="promptDialogMode"
      :title="promptDialogTitle"
      :message="promptDialogMessage"
      :model-value="promptDialogValue"
      :options="promptDialogOptions"
      :placeholder="promptDialogPlaceholder"
      :confirm-text="promptDialogConfirmText"
      :cancel-text="promptDialogCancelText"
      @update:open="promptDialogOpen = $event"
      @update:model-value="promptDialogValue = $event"
      @confirm="onPromptDialogConfirm"
      @cancel="onPromptDialogCancel"
    />
    <div class="notice-stack">
      <div
        v-for="n in notices"
        :key="n.id"
        class="notice-item"
        :class="`is-${n.level}`"
        @click="toggleNotice(n.id)"
      >
        <div class="notice-row">
          <span class="notice-icon">
            <span v-if="n.level === 'success'">✔</span>
            <span v-else-if="n.level === 'warning'">!</span>
            <span v-else-if="n.level === 'error'">✖</span>
            <span v-else>i</span>
          </span>
          <span class="notice-text">{{ n.message }}</span>
          <button class="notice-close" @click.stop="dismissNotice(n.id)">×</button>
        </div>
        <div v-if="n.detail && n.expanded" class="notice-detail">{{ n.detail }}</div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.notice-stack {
  position: fixed;
  top: calc(var(--ide-titlebar-height) + 8px);
  right: 12px;
  z-index: 160;
  display: flex;
  flex-direction: column;
  gap: 8px;
}
.notice-item {
  min-width: 260px;
  max-width: 520px;
  padding: 9px 12px;
  border-radius: 6px;
  font-size: 12px;
  color: var(--ide-text);
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-elevated);
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.28);
  cursor: pointer;
  animation: notice-in 180ms ease-out;
}
.notice-row {
  display: grid;
  grid-template-columns: 16px minmax(0, 1fr) 18px;
  gap: 8px;
  align-items: center;
}
.notice-icon {
  font-size: 12px;
  line-height: 1;
  color: var(--ide-text-muted);
}
.notice-text {
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.notice-close {
  width: 18px;
  height: 18px;
  border-radius: 4px;
  color: var(--ide-text-muted);
  cursor: pointer;
}
.notice-close:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}
.notice-detail {
  margin-top: 8px;
  padding-top: 8px;
  border-top: 1px solid color-mix(in srgb, var(--ide-border) 55%, transparent);
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--ide-text-muted);
  font-size: 11px;
  line-height: 1.45;
}
.notice-item.is-success {
  border-color: color-mix(in srgb, #2e7d32 45%, var(--ide-border));
}
.notice-item.is-warning {
  border-color: color-mix(in srgb, #ffb300 45%, var(--ide-border));
}
.notice-item.is-error {
  border-color: color-mix(in srgb, #d32f2f 50%, var(--ide-border));
}
.ide-list-item.active {
  background: color-mix(in srgb, var(--ide-accent) 20%, transparent);
}
:deep(mark.search-hit) {
  background: color-mix(in srgb, #ffd54f 58%, transparent);
  color: inherit;
  padding: 0 1px;
}
@keyframes notice-in {
  from {
    opacity: 0;
    transform: translateY(-6px);
  }
  to {
    opacity: 1;
    transform: translateY(0);
  }
}
</style>