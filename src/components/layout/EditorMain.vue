<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';
import * as monaco from 'monaco-editor';
import { emmetHTML, emmetCSS, emmetJSX } from 'emmet-monaco-es';
import { openPath } from '@tauri-apps/plugin-opener';
import { FontAwesomeIcon } from '../../icons/fontawesome';
import { renderMarkdownPreview } from '../../utils/markdownPreview';
// 导入 Monaco 自带的 worker
import editorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import jsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import cssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker';
import htmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker';
import tsWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker';

// 设置全局 Worker 加载逻辑
self.MonacoEnvironment = {
  getWorker(_, label) {
    if (label === 'json') {
      return new jsonWorker();
    }
    if (label === 'css' || label === 'scss' || label === 'less') {
      return new cssWorker();
    }
    if (label === 'html' || label === 'handlebars' || label === 'razor') {
      return new htmlWorker();
    }
    if (label === 'typescript' || label === 'javascript') {
      return new tsWorker();
    }
    return new editorWorker();
  }
};

const editorContainer = ref<HTMLElement | null>(null);
const tabsContainerRef = ref<HTMLElement | null>(null);
let editor: monaco.editor.IStandaloneCodeEditor | null = null;
let suppressChangeEvent = false;
let languageDefaultsApplied = false;
let tabsResizeObserver: ResizeObserver | null = null;
let editorResizeObserver: ResizeObserver | null = null;

const props = withDefaults(defineProps<{
  tabs: Array<{
    path: string;
    name: string;
    content: string;
    originalContent: string;
  }>;
  activePath: string | null;
  filePath: string | null;
  fileName: string;
  content: string;
  dirty: boolean;
  editorFontSize: number;
  editorTheme: 'dark' | 'light' | 'monaco';
  indentTabSize?: number;
  indentInsertSpaces?: boolean;
  groupId?: 'primary' | 'secondary';
}>(), {
  indentTabSize: 2,
  indentInsertSpaces: true,
  groupId: 'primary',
});

const emit = defineEmits<{
  (e: 'update:content', value: string): void;
  (e: 'activate-tab', path: string): void;
  (e: 'close-tab', payload: { path: string; group: 'primary' | 'secondary' }): void;
  (e: 'tab-action', payload: {
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
  }): void;
  (e: 'cursor-change', value: { line: number; col: number }): void;
  (e: 'indent-change', value: { tabSize: number; insertSpaces: boolean }): void;
  (e: 'focus-editor'): void;
  (e: 'reorder-tabs', payload: { fromPath: string; insertIdx: number; group: 'primary' | 'secondary' }): void;
}>();

const tabMenu = ref<{ open: boolean; x: number; y: number; path: string | null }>({
  open: false,
  x: 0,
  y: 0,
  path: null,
});
const tabMenuRef = ref<HTMLElement | null>(null);
const hiddenTabsMenuOpen = ref(false);
const hiddenTabPaths = ref<string[]>([]);
const hiddenTabsCount = ref(0);
const hasTabsOverflow = ref(false);
const hiddenTabsMenuPos = ref({ x: 0, y: 0 });
const markdownPreviewOpen = ref(false);
const markdownPreviewPaneRef = ref<HTMLElement | null>(null);
let markdownSyncLock: 'editor' | 'preview' | null = null;

const extensionOf = (name: string) => name.split('.').pop()?.toLowerCase() ?? '';
const activeTabEntry = computed(() => props.tabs.find((t) => t.path === props.activePath) ?? null);
const isMarkdownFile = computed(() => {
  const fromPath = (props.activePath ?? props.filePath ?? '').replace(/\\/g, '/').split('/').pop() ?? '';
  const fromTabName = activeTabEntry.value?.name ?? '';
  const base = fromTabName || fromPath || props.fileName;
  const ext = extensionOf(base);
  return ext === 'md' || ext === 'markdown';
});
const isHtmlFile = computed(() => {
  const base = (props.fileName || props.filePath || '').split(/[\\/]/).pop() ?? '';
  const ext = extensionOf(base);
  return ext === 'html' || ext === 'htm';
});

const tabDragSourcePath = ref<string | null>(null);
const onTabDragStart = (e: DragEvent, path: string) => {
  const t = e.target as HTMLElement | null;
  if (t?.closest('.tab-close')) {
    e.preventDefault();
    return;
  }
  tabDragSourcePath.value = path;
  e.dataTransfer?.setData('text/plain', path);
  e.dataTransfer!.effectAllowed = 'move';
};
const onTabDragEnd = () => {
  tabDragSourcePath.value = null;
};
const onTabDragOver = (e: DragEvent) => {
  e.preventDefault();
  if (e.dataTransfer) e.dataTransfer.dropEffect = 'move';
};
/** Insert index in the path array *after* removing fromPath; avoids insertBeforePath === moved tab (indexOf -1). */
const computeTabReorderInsertIdx = (fromPath: string, tabIndex: number, before: boolean) => {
  const n = props.tabs.length;
  const fromIdx = props.tabs.findIndex((t) => t.path === fromPath);
  if (fromIdx < 0) return null;
  const rawInsert = before ? tabIndex : tabIndex + 1;
  const ri = Math.max(0, Math.min(rawInsert, n));
  return fromIdx < ri ? ri - 1 : ri;
};

const onTabDrop = (e: DragEvent, tab: { path: string }, tabIndex: number) => {
  e.preventDefault();
  e.stopPropagation();
  const fromPath = e.dataTransfer?.getData('text/plain');
  if (!fromPath) return;
  const el = e.currentTarget as HTMLElement;
  const rect = el.getBoundingClientRect();
  const before = e.clientX < rect.left + rect.width / 2;
  if (fromPath === tab.path && before) return;
  const insertIdx = computeTabReorderInsertIdx(fromPath, tabIndex, before);
  if (insertIdx === null) return;
  emit('reorder-tabs', { fromPath, insertIdx, group: props.groupId });
};
const onTabsScrollDrop = (e: DragEvent) => {
  if (e.target !== e.currentTarget) return;
  e.preventDefault();
  const fromPath = e.dataTransfer?.getData('text/plain');
  if (!fromPath) return;
  const n = props.tabs.length;
  const fromIdx = props.tabs.findIndex((t) => t.path === fromPath);
  if (fromIdx < 0) return;
  const rawInsert = n;
  const insertIdx = fromIdx < rawInsert ? rawInsert - 1 : rawInsert;
  emit('reorder-tabs', { fromPath, insertIdx, group: props.groupId });
};
const isRasterImageFile = computed(() => {
  const ext = extensionOf(props.fileName);
  return ['png', 'jpg', 'jpeg', 'gif', 'webp', 'bmp', 'ico'].includes(ext);
});
const isSvgFile = computed(() => extensionOf(props.fileName) === 'svg');
const shouldShowImagePreview = computed(() =>
  isRasterImageFile.value && !isSvgFile.value && props.content.startsWith('data:image/'),
);

const markdownPreviewHtml = computed(() => renderMarkdownPreview(props.content));

const largeFileMetrics = computed(() => {
  const text = props.content ?? '';
  // Heuristics: Monaco performance cliffs are usually caused by very large models
  // (many lines and/or very long single-line files).
  const bytes = typeof TextEncoder !== 'undefined' ? new TextEncoder().encode(text).length : text.length;
  const lines = (text.match(/\n/g)?.length ?? 0) + 1;
  const maxLineLength = text
    .split('\n')
    .reduce((m, l) => (l.length > m ? l.length : m), 0);
  return { bytes, lines, maxLineLength };
});

const isLargeFile = computed(() => {
  const { bytes, lines, maxLineLength } = largeFileMetrics.value;
  return bytes >= 1_000_000 || lines >= 20_000 || maxLineLength >= 20_000;
});

const applyLargeFileOptions = () => {
  if (!editor) return;
  const large = isLargeFile.value;

  editor.updateOptions({
    // Monaco built-in fast-paths.
    largeFileOptimizations: true,

    // Expensive UI features that scale poorly with big models.
    minimap: { enabled: !large },
    folding: !large,
    codeLens: !large,
    wordBasedSuggestions: !large,
    occurrencesHighlight: !large,
    selectionHighlight: !large,
    renderValidationDecorations: large ? 'off' : 'editable',
    scrollBeyondLastLine: !large,
    smoothScrolling: !large,
    cursorSmoothCaretAnimation: large ? 'off' : 'on',

    // Reduce layout / paint churn.
    renderWhitespace: large ? 'none' : 'selection',
    renderControlCharacters: !large,
    bracketPairColorization: !large,
    guides: {
      indentation: !large,
      bracketPairs: !large,
      bracketPairsHorizontal: !large,
      highlightActiveIndentation: !large,
    },
  });
};

const openHtmlInBrowser = async () => {
  if (!isHtmlFile.value) return;
  const path = props.filePath;
  if (!path) return;
  try {
    await openPath(path);
  } catch (err) {
    // In dev (pure Vite) or when opener scope denies the path, this may fail.
    console.error('Failed to open HTML in external browser via openPath', path, err);
  }
};

const onTabContextMenu = (event: MouseEvent, path: string) => {
  event.preventDefault();
  tabMenu.value = {
    open: true,
    x: event.clientX,
    y: event.clientY,
    path,
  };
  nextTick(() => {
    const menu = tabMenuRef.value;
    if (!menu) return;
    const rect = menu.getBoundingClientRect();
    const margin = 8;
    const maxX = window.innerWidth - rect.width - margin;
    const maxY = window.innerHeight - rect.height - margin;
    tabMenu.value = {
      ...tabMenu.value,
      x: Math.max(margin, Math.min(tabMenu.value.x, maxX)),
      y: Math.max(margin, Math.min(tabMenu.value.y, maxY)),
    };
  });
};

const closeTabMenu = () => {
  tabMenu.value.open = false;
  hiddenTabsMenuOpen.value = false;
};

const onGlobalPointerDown = (event: PointerEvent) => {
  const target = event.target as HTMLElement | null;
  if (!target) return;
  if (target.closest('.hidden-tabs-trigger') || target.closest('.hidden-tabs-menu')) return;
  closeTabMenu();
};

const estimateTabWidth = (tab: { name: string; content: string; originalContent: string }) => {
  const dirtySuffix = tab.content !== tab.originalContent ? 2 : 0;
  const len = tab.name.length + dirtySuffix;
  // Keep this estimate close to real rendered width; conservative values here
  // caused tabs to be hidden too early (e.g. only ~5 visible).
  return Math.min(180, Math.max(76, len * 6 + 46));
};

const hiddenTabs = computed(() => {
  const hiddenSet = new Set(hiddenTabPaths.value);
  return props.tabs.filter((tab) => hiddenSet.has(tab.path));
});

const toggleHiddenTabsMenu = (event?: MouseEvent) => {
  if (hiddenTabsCount.value <= 0 && !hasTabsOverflow.value) return;
  const menuWidth = 240;
  if (event?.currentTarget) {
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    hiddenTabsMenuPos.value = {
      x: Math.max(8, Math.min(window.innerWidth - menuWidth - 8, rect.right - menuWidth + 4)),
      y: rect.bottom - 1,
    };
  }
  hiddenTabsMenuOpen.value = !hiddenTabsMenuOpen.value;
};

const recomputeHiddenTabs = () => {
  const container = tabsContainerRef.value;
  if (!container) {
    hiddenTabPaths.value = [];
    hiddenTabsCount.value = 0;
    return;
  }
  const width = container.clientWidth;
  if (width <= 0) {
    hiddenTabPaths.value = [];
    hiddenTabsCount.value = 0;
    hasTabsOverflow.value = false;
    return;
  }
  hasTabsOverflow.value = container.scrollWidth > width + 1;

  const tabElements = Array.from(container.querySelectorAll<HTMLElement>('.tab[data-path]'));
  const collect = (cutoff: number) => {
    const result: string[] = [];
    for (const el of tabElements) {
      const path = el.dataset.path;
      if (!path) continue;
      const tabRight = el.offsetLeft + el.offsetWidth;
      if (tabRight > cutoff) result.push(path);
    }
    return result;
  };

  let hidden = collect(width);
  // Fallback: if there is obvious overflow but no hidden rows detected,
  // run once more on next frame to avoid transient layout timing issues.
  if (!hidden.length && container.scrollWidth > width + 1) {
    requestAnimationFrame(() => {
      const retry = collect(tabsContainerRef.value?.clientWidth ?? width);
      if (retry.length) {
        hiddenTabPaths.value = retry;
        hiddenTabsCount.value = retry.length;
      }
    });
  }
  hiddenTabPaths.value = hidden;
  hiddenTabsCount.value = hidden.length;
};

const onWindowResize = () => {
  hiddenTabsMenuOpen.value = false;
  requestAnimationFrame(() => {
    recomputeHiddenTabs();
    editor?.layout();
  });
};

const syncPreviewScrollFromEditor = () => {
  if (!editor || !markdownPreviewPaneRef.value) return;
  if (!(isMarkdownFile.value && markdownPreviewOpen.value && !shouldShowImagePreview.value)) return;
  if (markdownSyncLock === 'preview') return;
  const scrollTop = editor.getScrollTop();
  const scrollHeight = editor.getScrollHeight();
  const layout = editor.getLayoutInfo();
  const maxEditor = Math.max(1, scrollHeight - layout.height);
  const ratio = Math.max(0, Math.min(1, scrollTop / maxEditor));
  const pane = markdownPreviewPaneRef.value;
  const maxPreview = Math.max(0, pane.scrollHeight - pane.clientHeight);
  markdownSyncLock = 'editor';
  pane.scrollTop = ratio * maxPreview;
  window.setTimeout(() => {
    markdownSyncLock = null;
  }, 0);
};

const onMarkdownPreviewScroll = () => {
  if (!editor || !markdownPreviewPaneRef.value) return;
  if (markdownSyncLock === 'editor') return;
  const pane = markdownPreviewPaneRef.value;
  const maxPreview = Math.max(1, pane.scrollHeight - pane.clientHeight);
  const ratio = Math.max(0, Math.min(1, pane.scrollTop / maxPreview));
  const scrollHeight = editor.getScrollHeight();
  const layout = editor.getLayoutInfo();
  const maxEditor = Math.max(0, scrollHeight - layout.height);
  markdownSyncLock = 'preview';
  editor.setScrollTop(ratio * maxEditor);
  window.setTimeout(() => {
    markdownSyncLock = null;
  }, 0);
};

const onTabMenuAction = (
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
    | 'reveal-in-folder',
) => {
  if (!tabMenu.value.path) return;
  emit('tab-action', { action, path: tabMenu.value.path });
  closeTabMenu();
};

const languageByExt = (name: string) => {
  const lowerName = name.toLowerCase();
  if (
    lowerName === '.env' ||
    lowerName.endsWith('.env') ||
    lowerName.startsWith('.env.') ||
    lowerName.includes('.env.')
  ) {
    return 'shell';
  }
  const ext = name.split('.').pop()?.toLowerCase();
  if (!ext) return 'plaintext';
  if (ext === 'env') return 'shell';
  if (ext === 'ts' || ext === 'tsx') return 'typescript';
  if (ext === 'js' || ext === 'jsx') return 'javascript';
  if (ext === 'vue') return 'html';
  if (ext === 'json') return 'json';
  if (ext === 'yaml' || ext === 'yml') return 'yaml';
  if (ext === 'py') return 'python';
  if (ext === 'php' || ext === 'phtml') return 'php';
  if (ext === 'java') return 'java';
  if (ext === 'kt' || ext === 'kts') return 'kotlin';
  if (ext === 'go') return 'go';
  if (ext === 'c') return 'c';
  if (ext === 'h') return 'cpp';
  if (ext === 'cpp' || ext === 'cc' || ext === 'cxx' || ext === 'hpp' || ext === 'hh' || ext === 'hxx') return 'cpp';
  if (ext === 'cs') return 'csharp';
  if (ext === 'swift') return 'swift';
  if (ext === 'rb') return 'ruby';
  if (ext === 'sh' || ext === 'bash' || ext === 'zsh' || ext === 'fish') return 'shell';
  if (ext === 'xml') return 'xml';
  if (ext === 'sql') return 'sql';
  if (ext === 'toml') return 'ini';
  if (ext === 'ini' || ext === 'cfg' || ext === 'conf') return 'ini';
  if (ext === 'dockerfile') return 'dockerfile';
  if (ext === 'css' || ext === 'scss' || ext === 'less') return 'css';
  if (ext === 'html' || ext === 'htm') return 'html';
  if (ext === 'md') return 'markdown';
  if (ext === 'rs') return 'rust';
  if (ext === 'txt' || ext === 'log') return 'plaintext';
  return 'plaintext';
};

const monacoTheme = (theme: 'dark' | 'light' | 'monaco') => {
  if (theme === 'light') return 'vs';
  if (theme === 'monaco') return 'vs-dark';
  return 'vs-dark';
};

const applyLanguageDefaults = () => {
  if (languageDefaultsApplied) return;
  languageDefaultsApplied = true;

  const tsDefaults = monaco.languages.typescript.typescriptDefaults as any;
  const jsDefaults = monaco.languages.typescript.javascriptDefaults as any;
  const jsonDefaults = (monaco.languages as any).json?.jsonDefaults as any;
  const htmlDefaults = (monaco.languages as any).html?.htmlDefaults as any;
  const cssDefaults = (monaco.languages as any).css?.cssDefaults as any;

  // In a lightweight IDE shell without full project type information,
  // Monaco's built-in validators can produce many false-positive red markers.
  const safeCall = (fn: () => void) => {
    try {
      fn();
    } catch {
      // Ignore Monaco API differences between builds.
    }
  };

  if (typeof tsDefaults?.setDiagnosticsOptions === 'function') {
    safeCall(() => tsDefaults.setDiagnosticsOptions({
      noSemanticValidation: true,
      noSyntaxValidation: true,
    }));
  }
  if (typeof jsDefaults?.setDiagnosticsOptions === 'function') {
    safeCall(() => jsDefaults.setDiagnosticsOptions({
      noSemanticValidation: true,
      noSyntaxValidation: true,
    }));
  }

  if (typeof jsonDefaults?.setDiagnosticsOptions === 'function') {
    safeCall(() => jsonDefaults.setDiagnosticsOptions({
      validate: false,
      allowComments: true,
      trailingCommas: 'ignore',
      schemaValidation: 'ignore',
      schemaRequest: 'ignore',
    }));
  }

  if (typeof htmlDefaults?.setOptions === 'function') {
    safeCall(() => htmlDefaults.setOptions({
      // WebStorm-like: keep HTML inspections enabled for single files.
      // (We still auto-disable heavy decorations in large-file mode.)
      validate: true,
    }));
  }

  if (typeof cssDefaults?.setDiagnosticsOptions === 'function') {
    safeCall(() => cssDefaults.setDiagnosticsOptions({
      validate: false,
      lint: {
        compatibleVendorPrefixes: 'ignore',
        vendorPrefix: 'ignore',
        duplicateProperties: 'ignore',
        emptyRules: 'ignore',
        importStatement: 'ignore',
        boxModel: 'ignore',
        universalSelector: 'ignore',
        zeroUnits: 'ignore',
        fontFaceProperties: 'ignore',
        hexColorLength: 'ignore',
        argumentsInColorFunction: 'ignore',
        unknownProperties: 'ignore',
        important: 'ignore',
        float: 'ignore',
        idSelector: 'ignore',
      },
    }));
  }

  // Enable Emmet abbreviations (html:5, ul>li*3, etc.) in Monaco.
  safeCall(() => {
    emmetHTML(monaco, ['html']);
    emmetCSS(monaco, ['css', 'scss', 'less']);
    emmetJSX(monaco, ['javascript', 'typescript', 'javascriptreact', 'typescriptreact']);
  });
};

const applyIndentation = (tabSize: number, insertSpaces: boolean) => {
  if (!editor) return;
  const normalizedTabSize = Math.max(1, Math.min(16, tabSize));
  editor.updateOptions({ tabSize: normalizedTabSize, insertSpaces });
  const model = editor.getModel();
  if (!model) return;
  model.updateOptions({
    tabSize: normalizedTabSize,
    insertSpaces,
  });
  const opts = model.getOptions();
  emit('indent-change', { tabSize: opts.tabSize, insertSpaces: opts.insertSpaces });
};

const runEditorAction = async (action: 'undo' | 'redo' | 'cut' | 'copy' | 'paste' | 'find' | 'selectAll' | 'format') => {
  if (!editor) return;
  editor.focus();
  if (action === 'format') {
    // Force editor + model indentation options right before formatting.
    applyIndentation(props.indentTabSize, props.indentInsertSpaces);
    await editor.getAction('editor.action.formatDocument').run();
    return;
  }
  const model = editor.getModel();
  const selection = editor.getSelection();
  const readClipboardText = async () => {
    if (!navigator.clipboard?.readText) return '';
    try {
      return await navigator.clipboard.readText();
    } catch {
      return '';
    }
  };
  const writeClipboardText = async (text: string) => {
    if (!navigator.clipboard?.writeText) return false;
    try {
      await navigator.clipboard.writeText(text);
      return true;
    } catch {
      return false;
    }
  };
  if ((action === 'copy' || action === 'cut') && model && selection && !selection.isEmpty()) {
    const selectedText = model.getValueInRange(selection);
    const copied = await writeClipboardText(selectedText);
    if (action === 'cut' && copied) {
      editor.executeEdits('manual-cut', [{ range: selection, text: '' }]);
    }
    if (copied) return;
  }
  if (action === 'paste' && model && selection) {
    const text = await readClipboardText();
    if (text) {
      editor.executeEdits('manual-paste', [{ range: selection, text }]);
      return;
    }
  }
  const actionMap: Record<typeof action, string> = {
    undo: 'undo',
    redo: 'redo',
    cut: 'editor.action.clipboardCutAction',
    copy: 'editor.action.clipboardCopyAction',
    paste: 'editor.action.clipboardPasteAction',
    find: 'actions.find',
    selectAll: 'editor.action.selectAll',
    format: 'editor.action.formatDocument',
  };
  editor.trigger('menu', actionMap[action], null);
};

const setIndentation = (tabSize: number, insertSpaces: boolean) => {
  applyIndentation(tabSize, insertSpaces);
};

const setLineEnding = (lineEnding: 'LF' | 'CRLF') => {
  if (!editor) return;
  const model = editor.getModel();
  if (!model) return;
  const next = lineEnding === 'CRLF'
    ? monaco.editor.EndOfLineSequence.CRLF
    : monaco.editor.EndOfLineSequence.LF;
  model.pushEOL(next);
};

const getLineEnding = () => {
  if (!editor) return 'LF' as const;
  const model = editor.getModel();
  if (!model) return 'LF' as const;
  return model.getEOL() === '\r\n' ? 'CRLF' as const : 'LF' as const;
};

const formatDocument = async (tabSize: number, insertSpaces: boolean) => {
  if (!editor) return;
  applyIndentation(tabSize, insertSpaces);
  editor.focus();
  await editor.getAction('editor.action.formatDocument').run();
};

const getIndentation = () => {
  if (!editor) return { tabSize: 2, insertSpaces: true };
  const model = editor.getModel();
  if (!model) return { tabSize: 2, insertSpaces: true };
  const opts = model.getOptions();
  return { tabSize: opts.tabSize, insertSpaces: opts.insertSpaces };
};

const getSelectionContext = () => {
  if (!editor || !props.filePath) return null;
  const model = editor.getModel();
  const selection = editor.getSelection();
  if (!model || !selection || selection.isEmpty()) return null;
  const snippet = model.getValueInRange(selection).trim();
  if (!snippet) return null;
  return {
    path: props.filePath,
    fileName: props.fileName,
    startLine: selection.startLineNumber,
    endLine: selection.endLineNumber,
    snippet,
  };
};

const syncEditorFromProps = () => {
  if (!editor) return;
  if (shouldShowImagePreview.value) {
    editor.updateOptions({ readOnly: true, fontSize: props.editorFontSize });
    monaco.editor.setTheme(monacoTheme(props.editorTheme));
    return;
  }
  applyLargeFileOptions();
  const model = editor.getModel();
  if (editor.getValue() !== props.content) {
    suppressChangeEvent = true;
    editor.setValue(props.content);
    suppressChangeEvent = false;
  }
  editor.updateOptions({ readOnly: !props.filePath });
  editor.updateOptions({ fontSize: props.editorFontSize });
  monaco.editor.setTheme(monacoTheme(props.editorTheme));
  if (model) {
    monaco.editor.setModelLanguage(model, languageByExt(props.fileName));
    model.updateOptions({
      tabSize: Math.max(1, Math.min(16, props.indentTabSize)),
      insertSpaces: props.indentInsertSpaces,
    });
    const opts = model.getOptions();
    emit('indent-change', { tabSize: opts.tabSize, insertSpaces: opts.insertSpaces });
  }
};

onMounted(() => {
  try {
    applyLanguageDefaults();
  } catch {
    // Do not block editor rendering if defaults fail.
  }
  if (editorContainer.value) {
    editor = monaco.editor.create(editorContainer.value, {
      value: props.content || '',
      language: 'plaintext',
      theme: monacoTheme(props.editorTheme),
      automaticLayout: true,
      largeFileOptimizations: true,
      readOnly: true,
      fontFamily: 'JetBrains Mono, Menlo, Monaco, Courier New, monospace',
      fontSize: props.editorFontSize,
      minimap: { enabled: true },
      renderLineHighlight: 'all',
    });

    editor.onDidChangeModelContent(() => {
      if (suppressChangeEvent) return;
      emit('update:content', editor?.getValue() ?? '');
    });

    editor.onDidChangeCursorPosition((e) => {
      emit('cursor-change', { line: e.position.lineNumber, col: e.position.column });
    });
    editor.onDidFocusEditorText(() => {
      emit('focus-editor');
    });
    editor.onDidScrollChange(() => {
      syncPreviewScrollFromEditor();
    });

    // Default: WebStorm-ish (2 spaces), and inform parent once.
    setIndentation(2, true);
    // Important for split editors: initial props may already contain content.
    applyLargeFileOptions();
    syncEditorFromProps();
    if (editorContainer.value && typeof ResizeObserver !== 'undefined') {
      editorResizeObserver = new ResizeObserver(() => {
        requestAnimationFrame(() => editor?.layout());
      });
      editorResizeObserver.observe(editorContainer.value);
    }
  }
});

watch(
  () => [
    props.content,
    props.fileName,
    props.filePath,
    props.editorFontSize,
    props.editorTheme,
    props.indentTabSize,
    props.indentInsertSpaces,
  ] as const,
  () => {
    syncEditorFromProps();
  },
  { immediate: true },
);

watch(
  () => props.fileName,
  () => {
    markdownPreviewOpen.value = false;
  },
);

watch(
  () => [shouldShowImagePreview.value, markdownPreviewOpen.value] as const,
  () => {
    requestAnimationFrame(() => {
      editor?.layout();
      syncPreviewScrollFromEditor();
    });
  },
);

watch(
  () => [props.tabs.length, props.activePath, props.tabs.map((t) => `${t.path}:${t.name.length}:${t.content !== t.originalContent ? 1 : 0}`).join('|')] as const,
  () => {
    nextTick(() => {
      recomputeHiddenTabs();
    });
  },
  { immediate: true },
);

onBeforeUnmount(() => {
  window.removeEventListener('pointerdown', onGlobalPointerDown);
  window.removeEventListener('keydown', onWindowKeydownForMenu);
  window.removeEventListener('resize', onWindowResize);
  editorResizeObserver?.disconnect();
  editorResizeObserver = null;
  editor?.dispose();
  editor = null;
  tabsResizeObserver?.disconnect();
  tabsResizeObserver = null;
});

const onWindowKeydownForMenu = (event: KeyboardEvent) => {
  if (event.key === 'Escape') {
    closeTabMenu();
  }
};

onMounted(() => {
  window.addEventListener('pointerdown', onGlobalPointerDown);
  window.addEventListener('keydown', onWindowKeydownForMenu);
  window.addEventListener('resize', onWindowResize);
  requestAnimationFrame(() => {
    recomputeHiddenTabs();
  });
  if (tabsContainerRef.value && typeof ResizeObserver !== 'undefined') {
    tabsResizeObserver = new ResizeObserver(() => {
      recomputeHiddenTabs();
    });
    tabsResizeObserver.observe(tabsContainerRef.value);
  }
});

defineExpose({
  runEditorAction,
  setIndentation,
  getIndentation,
  setLineEnding,
  getLineEnding,
  formatDocument,
  getSelectionContext,
});
</script>

<template>
  <div class="editor-main" :style="{ '--md-preview-font-size': `${props.editorFontSize}px` }" @pointerdown="emit('focus-editor')">
    <div class="tabs-bar">
      <div
        ref="tabsContainerRef"
        class="tabs-scroll"
        @dragover.prevent="onTabDragOver"
        @drop="onTabsScrollDrop"
      >
        <div
          v-for="(tab, tabIndex) in tabs"
          :key="tab.path"
          class="tab"
          :class="{ active: tab.path === activePath, 'tab--dragging': tabDragSourcePath === tab.path }"
          :data-path="tab.path"
          draggable="true"
          @dragstart="onTabDragStart($event, tab.path)"
          @dragend="onTabDragEnd"
          @dragover.prevent="onTabDragOver"
          @drop.prevent.stop="onTabDrop($event, tab, tabIndex)"
          @click="emit('activate-tab', tab.path)"
          @contextmenu="onTabContextMenu($event, tab.path)"
        >
          <span class="tab-name">{{ tab.name }}<span v-if="tab.content !== tab.originalContent"> *</span></span>
          <button
            type="button"
            class="tab-close"
            draggable="false"
            @click.stop="emit('close-tab', { path: tab.path, group: props.groupId })"
          >
            x
          </button>
        </div>
        <div v-if="tabs.length === 0" class="tab active">{{ fileName }}<span v-if="dirty"> *</span></div>
      </div>
      <div
        v-if="hiddenTabsCount > 0 || hasTabsOverflow"
        class="tabs-actions"
      >
        <button
          type="button"
          class="hidden-tabs-trigger"
          :class="{ 'is-open': hiddenTabsMenuOpen }"
          :title="`Hidden tabs (${hiddenTabsCount})`"
          @pointerdown.stop
          @click.stop="toggleHiddenTabsMenu($event)"
        >
          <span class="hidden-tabs-icon">
            <FontAwesomeIcon :icon="['fas', hiddenTabsMenuOpen ? 'chevron-up' : 'chevron-down']" />
          </span>
          <span class="hidden-tabs-count">{{ hiddenTabsCount }}</span>
        </button>
      </div>
      <transition name="hidden-tabs-fade">
        <div
          v-if="hiddenTabsMenuOpen && hiddenTabs.length > 0"
          class="hidden-tabs-menu"
          :style="{ left: `${hiddenTabsMenuPos.x}px`, top: `${hiddenTabsMenuPos.y}px` }"
          @pointerdown.stop
        >
          <div
            v-for="tab in hiddenTabs"
            :key="`hidden-${tab.path}`"
            class="hidden-tab-item"
            :class="{ active: tab.path === activePath }"
            :title="tab.name"
            @click="emit('activate-tab', tab.path); hiddenTabsMenuOpen = false"
          >
            <span class="hidden-tab-name">{{ tab.name }}<span v-if="tab.content !== tab.originalContent"> *</span></span>
            <button class="hidden-tab-close" @click.stop="emit('close-tab', { path: tab.path, group: props.groupId })">
              <FontAwesomeIcon :icon="['fas', 'xmark']" />
            </button>
          </div>
        </div>
      </transition>
    </div>
    <div
      v-if="tabMenu.open && tabMenu.path"
      ref="tabMenuRef"
      class="tab-menu"
      :style="{ left: `${tabMenu.x}px`, top: `${tabMenu.y}px` }"
      @pointerdown.stop
    >
      <button class="tab-menu-item" @click="onTabMenuAction('close')"><span>Close</span><span class="shortcut">Ctrl+W</span></button>
      <button class="tab-menu-item" @click="onTabMenuAction('close-others')"><span>Close Others</span><span class="shortcut">Ctrl+Alt+W</span></button>
      <button class="tab-menu-item" @click="onTabMenuAction('close-right')"><span>Close to the Right</span></button>
      <button class="tab-menu-item" @click="onTabMenuAction('close-all')"><span>Close All</span><span class="shortcut">Ctrl+Shift+W</span></button>
      <button class="tab-menu-item" @click="onTabMenuAction('close-saved')"><span>Close Saved</span><span class="shortcut">Ctrl+Alt+S</span></button>
      <div class="tab-menu-sep" />
      <button class="tab-menu-item" @click="onTabMenuAction('split-right')"><span>Split Right</span><span class="shortcut">Ctrl+\</span></button>
      <button class="tab-menu-item" @click="onTabMenuAction('move-to-next-group')"><span>Move to Next Group</span><span class="shortcut">Ctrl+Alt+\</span></button>
      <button class="tab-menu-item" @click="onTabMenuAction('close-group')"><span>Close Group</span><span class="shortcut">Ctrl+Shift+\</span></button>
      <div class="tab-menu-sep" />
      <button class="tab-menu-item" @click="onTabMenuAction('copy-path')">Copy Path</button>
      <button class="tab-menu-item" @click="onTabMenuAction('copy-real-path')">Copy Full Path</button>
      <button class="tab-menu-item" @click="onTabMenuAction('reveal-in-folder')">Reveal in File Manager</button>
    </div>
    <div class="editor-workbench">
      <div class="editor-surface">
        <div
          v-if="isMarkdownFile && !shouldShowImagePreview"
          class="md-editor-mode-switch"
          @pointerdown.stop
        >
          <button
            type="button"
            class="md-mode-btn"
            :class="{ active: !markdownPreviewOpen }"
            @click.stop="markdownPreviewOpen = false"
          >
            Markdown
          </button>
          <button
            type="button"
            class="md-mode-btn"
            :class="{ active: markdownPreviewOpen }"
            @click.stop="markdownPreviewOpen = true"
          >
            Preview
          </button>
        </div>
        <button
          v-if="isHtmlFile && !shouldShowImagePreview"
          type="button"
          class="html-open-btn"
          title="Open in browser"
          @pointerdown.stop
          @click.stop="openHtmlInBrowser"
        >
          <FontAwesomeIcon :icon="['fas', 'globe']" />
        </button>
        <div
          ref="editorContainer"
          class="monaco-instance"
          :class="{
            'monaco-instance--hidden': shouldShowImagePreview || (isMarkdownFile && markdownPreviewOpen),
          }"
        />
        <div
          v-if="isMarkdownFile && markdownPreviewOpen && !shouldShowImagePreview"
          ref="markdownPreviewPaneRef"
          class="markdown-preview-pane markdown-preview-pane--full ide-scrollbar"
          v-html="markdownPreviewHtml"
          @scroll="onMarkdownPreviewScroll"
        />
        <div v-if="shouldShowImagePreview" class="image-preview-overlay">
          <div class="image-preview-wrap ide-scrollbar">
            <img class="image-preview" :src="content" :alt="fileName" draggable="false">
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* 保持之前的样式 */
.editor-main {
  flex: 1;
  min-width: 0;
  min-height: 0;
  max-width: 100%;
  display: flex;
  flex-direction: column;
  background-color: var(--ide-bg-editor);
}
.editor-workbench {
  flex: 1;
  min-height: 0;
  min-width: 0;
  display: flex;
  flex-direction: column;
  position: relative;
}
/* In-flow Monaco establishes height; overlays are position:absolute. All-absolute children would collapse this flex item to 0 height. */
.editor-surface {
  flex: 1;
  min-height: 0;
  min-width: 0;
  position: relative;
  display: flex;
  flex-direction: column;
}
.monaco-instance {
  flex: 1;
  min-height: 0;
  min-width: 0;
  position: relative;
  overflow: hidden;
}
.md-editor-mode-switch {
  position: absolute;
  top: 10px;
  right: 14px;
  z-index: 8;
  display: flex;
  align-items: stretch;
  border: 1px solid var(--ide-border);
  border-radius: 6px;
  overflow: hidden;
  background: color-mix(in srgb, var(--ide-bg-editor) 88%, var(--ide-bg-main) 12%);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.22);
  backdrop-filter: blur(6px);
}
.md-mode-btn {
  padding: 5px 12px;
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.02em;
  border: none;
  background: transparent;
  color: var(--ide-text-muted);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease;
}
.md-mode-btn + .md-mode-btn {
  border-left: 1px solid var(--ide-border);
}
.md-mode-btn:hover {
  color: var(--ide-text);
  background: color-mix(in srgb, var(--ide-hover) 55%, transparent);
}
.md-mode-btn.active {
  color: var(--ide-text);
  background: color-mix(in srgb, var(--ide-accent) 24%, var(--ide-bg-editor) 76%);
}
.html-open-btn {
  position: absolute;
  top: 10px;
  right: 14px;
  z-index: 8;
  width: 28px;
  height: 28px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  border-radius: 999px;
  border: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-editor) 88%, var(--ide-bg-main) 12%);
  color: var(--ide-text-muted);
  cursor: pointer;
  transition: background-color 0.12s ease, color 0.12s ease, box-shadow 0.12s ease;
}
.html-open-btn :deep(svg) {
  width: 13px;
  height: 13px;
}
.html-open-btn:hover {
  color: var(--ide-text);
  background: color-mix(in srgb, var(--ide-accent) 18%, var(--ide-bg-editor) 82%);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.28);
}
.monaco-instance--hidden {
  visibility: hidden;
  pointer-events: none;
}
.markdown-preview-pane {
  min-height: 0;
  overflow: auto;
  padding: 12px 14px;
  color: var(--ide-text);
  background: color-mix(in srgb, var(--ide-bg-editor) 92%, #000 8%);
  font-size: var(--md-preview-font-size, 13px);
  line-height: 1.5;
}
.markdown-preview-pane--full {
  position: absolute;
  inset: 0;
  z-index: 3;
}
.markdown-preview-pane :deep(h1),
.markdown-preview-pane :deep(h2),
.markdown-preview-pane :deep(h3),
.markdown-preview-pane :deep(h4),
.markdown-preview-pane :deep(h5),
.markdown-preview-pane :deep(h6) {
  margin: 0 0 8px;
  font-weight: 600;
}
.markdown-preview-pane :deep(h1) { font-size: calc(var(--md-preview-font-size, 13px) * 1.32); }
.markdown-preview-pane :deep(h2) { font-size: calc(var(--md-preview-font-size, 13px) * 1.2); }
.markdown-preview-pane :deep(h3) { font-size: calc(var(--md-preview-font-size, 13px) * 1.1); }
.markdown-preview-pane :deep(h4),
.markdown-preview-pane :deep(h5),
.markdown-preview-pane :deep(h6) { font-size: var(--md-preview-font-size, 13px); }
.markdown-preview-pane :deep(p) {
  margin: 0 0 8px;
}
.markdown-preview-pane :deep(p),
.markdown-preview-pane :deep(li) {
  line-height: 1.5;
}
.markdown-preview-pane :deep(code) {
  font-family: 'JetBrains Mono', Menlo, Monaco, 'Courier New', monospace;
  font-size: 0.92em;
  padding: 1px 4px;
  border: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-main) 94%, #000 6%);
}
.markdown-preview-pane :deep(.md-link) {
  color: var(--ide-accent);
  text-decoration: none;
}
.markdown-preview-pane :deep(.md-link:hover) {
  text-decoration: underline;
}
.markdown-preview-pane :deep(ul),
.markdown-preview-pane :deep(ol) {
  margin: 0 0 10px 0;
  padding-left: 1.5em;
  list-style-position: outside;
}
.markdown-preview-pane :deep(ul) {
  list-style-type: disc;
}
.markdown-preview-pane :deep(ol) {
  list-style-type: decimal;
}
.markdown-preview-pane :deep(li) {
  display: list-item;
  margin: 2px 0;
}
.markdown-preview-pane :deep(li)::marker {
  color: var(--ide-text-muted);
}
.markdown-preview-pane :deep(ul ul),
.markdown-preview-pane :deep(ul ol),
.markdown-preview-pane :deep(ol ul),
.markdown-preview-pane :deep(ol ol) {
  margin: 4px 0 6px;
}
.markdown-preview-pane :deep(blockquote) {
  margin: 0 0 10px;
  padding: 6px 12px;
  border-left: 3px solid color-mix(in srgb, var(--ide-accent) 55%, var(--ide-border) 45%);
  color: var(--ide-text-muted);
  background: color-mix(in srgb, var(--ide-bg-main) 88%, var(--ide-accent) 6%);
}
.markdown-preview-pane :deep(blockquote p:last-child) {
  margin-bottom: 0;
}
.markdown-preview-pane :deep(hr) {
  margin: 14px 0;
  border: none;
  border-top: 1px solid var(--ide-border);
}
.markdown-preview-pane :deep(del) {
  text-decoration: line-through;
  opacity: 0.85;
}
.markdown-preview-pane :deep(table) {
  width: 100%;
  margin: 0 0 12px;
  border-collapse: collapse;
  font-size: 0.96em;
}
.markdown-preview-pane :deep(th),
.markdown-preview-pane :deep(td) {
  border: 1px solid var(--ide-border);
  padding: 6px 10px;
  text-align: left;
  vertical-align: top;
}
.markdown-preview-pane :deep(th) {
  background: color-mix(in srgb, var(--ide-bg-main) 90%, #000 10%);
  font-weight: 600;
}
.markdown-preview-pane :deep(tr:nth-child(even) td) {
  background: color-mix(in srgb, var(--ide-bg-editor) 96%, #000 4%);
}
.markdown-preview-pane :deep(.md-img) {
  max-width: 100%;
  height: auto;
  margin: 6px 0 10px;
  border-radius: 4px;
  border: 1px solid var(--ide-border);
}
.markdown-preview-pane :deep(li input[type='checkbox']) {
  margin: 0 6px 0 0;
  vertical-align: middle;
  pointer-events: none;
  accent-color: var(--ide-accent);
}
.markdown-preview-pane :deep(.md-pre) {
  margin: 0 0 10px;
  border: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-main) 92%, #000 8%);
  padding: 8px;
  overflow: auto;
  font-size: 0.92em;
}
.markdown-preview-pane :deep(.md-pre code) {
  display: block;
  padding: 0;
  border: none;
  background: transparent;
  font-size: inherit;
  line-height: 1.45;
  white-space: pre;
  word-break: normal;
  overflow-wrap: normal;
}
.image-preview-overlay {
  position: absolute;
  inset: 0;
  z-index: 6;
  background: var(--ide-bg-editor);
}
.image-preview-wrap {
  width: 100%;
  height: 100%;
  overflow: auto;
  display: flex;
  align-items: center;
  justify-content: center;
  padding: 18px;
}
.image-preview {
  max-width: 100%;
  max-height: 100%;
  object-fit: contain;
  border: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-main) 95%, #000 5%);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
}
.tabs-bar {
  height: max(var(--ide-tabs-height), 36px);
  min-height: 36px;
  min-width: 0;
  background-color: var(--ide-bg-main);
  border-bottom: 1px solid var(--ide-border);
  display: flex;
  align-items: stretch;
  flex-shrink: 0;
  position: relative;
}
.tabs-scroll {
  flex: 1 1 0;
  min-width: 0;
  display: flex;
  align-items: center;
  overflow: hidden;
}
.tabs-actions {
  flex: 0 0 auto;
  display: flex;
  align-items: stretch;
  border-left: 1px solid var(--ide-border);
  background: var(--ide-bg-main);
  box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.03);
  z-index: 4;
  min-width: 0;
}
.tab {
  height: 100%;
  display: inline-flex;
  flex: 0 0 auto;
  align-items: center;
  gap: 6px;
  padding: 0 10px 0 13px;
  font-size: 12px;
  color: var(--ide-text-muted);
  border-right: 1px solid var(--ide-border);
  cursor: grab;
  user-select: none;
}
.tab:active {
  cursor: grabbing;
}
.tab .tab-close {
  cursor: pointer;
}
.tab.active {
  color: var(--ide-text);
  background: linear-gradient(to bottom, rgba(255, 255, 255, 0.03), rgba(0, 0, 0, 0.05));
}
.tab-name {
  max-width: 180px;
  overflow: hidden;
  text-overflow: ellipsis;
}
.tab-close {
  box-sizing: border-box;
  flex: 0 0 18px;
  width: 18px;
  height: 18px;
  padding: 0;
  border: none;
  border-radius: 3px;
  color: var(--ide-text-muted);
  cursor: pointer;
  opacity: 0;
  pointer-events: none;
  transition: opacity 0.12s ease;
}
.tab.active .tab-close,
.tab:hover .tab-close {
  opacity: 1;
  pointer-events: auto;
}
.tab-close:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}
.tab-close:focus-visible {
  opacity: 1;
  pointer-events: auto;
  outline: 1px solid color-mix(in srgb, var(--ide-accent) 55%, var(--ide-border));
  outline-offset: 1px;
}
.tab--dragging {
  opacity: 0.45;
}

.hidden-tabs-trigger {
  flex: 0 0 auto;
  min-width: 34px;
  width: 34px;
  border: none;
  background: color-mix(in srgb, var(--ide-bg-main) 82%, var(--ide-hover));
  color: var(--ide-text);
  cursor: pointer;
  font-size: 11px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 2px;
  box-shadow: inset 1px 0 0 rgba(255, 255, 255, 0.04);
}
.hidden-tabs-trigger:not(:first-child) {
  border-left: 1px solid var(--ide-border);
}
.hidden-tabs-trigger:hover {
  background: color-mix(in srgb, var(--ide-hover) 88%, var(--ide-bg-main));
}
.hidden-tabs-trigger.is-open {
  background: color-mix(in srgb, var(--ide-accent) 18%, var(--ide-bg-main));
}
.hidden-tabs-icon {
  line-height: 1;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  transition: transform 0.18s ease;
}
.hidden-tabs-trigger.is-open .hidden-tabs-icon {
  transform: translateY(-0.5px);
}
.hidden-tabs-count {
  min-width: 10px;
  text-align: center;
  font-size: 10px;
  color: var(--ide-text-muted);
}

.hidden-tabs-menu {
  position: fixed;
  width: 240px;
  max-height: 320px;
  overflow: auto;
  border: 1px solid var(--ide-border);
  border-radius: 8px;
  background: color-mix(in srgb, var(--ide-bg-main) 94%, #111 6%);
  box-shadow: 0 12px 28px rgba(0, 0, 0, 0.42), 0 2px 8px rgba(0, 0, 0, 0.25);
  backdrop-filter: blur(4px);
  z-index: 130;
  scrollbar-color: #5d6672 #242a33;
}
.hidden-tabs-menu::-webkit-scrollbar { width: 8px; height: 8px; }
.hidden-tabs-menu::-webkit-scrollbar-thumb { background: #5d6672; border-radius: 8px; }
.hidden-tabs-menu::-webkit-scrollbar-track { background: #242a33; }
.hidden-tab-item {
  height: 30px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  padding: 0 8px;
  font-size: 12px;
  color: var(--ide-text);
  cursor: pointer;
}
.hidden-tab-item:hover {
  background: var(--ide-hover);
}
.hidden-tab-item.active {
  background: color-mix(in srgb, var(--ide-accent) 20%, transparent);
}
.hidden-tab-name {
  flex: 1;
  min-width: 0;
  max-width: 100%;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.hidden-tab-close {
  flex: 0 0 auto;
  width: 18px;
  height: 18px;
  border-radius: 4px;
  color: var(--ide-text-muted);
  cursor: pointer;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  font-size: 10px;
}
.hidden-tab-close:hover {
  background: color-mix(in srgb, var(--ide-hover) 88%, transparent);
  color: var(--ide-text);
}

.hidden-tabs-fade-enter-active,
.hidden-tabs-fade-leave-active {
  transition: opacity 0.16s ease, transform 0.16s ease;
}
.hidden-tabs-fade-enter-from,
.hidden-tabs-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px) scale(0.985);
}

.tab-menu {
  position: fixed;
  min-width: 220px;
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  border-radius: 8px;
  padding: 6px 0;
  z-index: 120;
  box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
}

.tab-menu-item {
  width: 100%;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 12px;
  color: var(--ide-text);
  cursor: pointer;
  font-size: 12px;
}

.tab-menu-item:hover {
  background: var(--ide-hover);
}

.tab-menu-sep {
  height: 1px;
  margin: 6px 8px;
  background: var(--ide-border);
}

.shortcut {
  margin-left: 12px;
  color: var(--ide-text-muted);
  font-size: 11px;
}
</style>