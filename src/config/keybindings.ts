export type Keybinding = {
  shortcut: string;
  command: string;
};

export const treeKeybindings: Keybinding[] = [
  { shortcut: 'Alt+Insert', command: 'tree.new-file' },
  { shortcut: 'Shift+F6', command: 'tree.rename' },
  { shortcut: 'Delete', command: 'tree.delete' },
  { shortcut: 'Ctrl+C', command: 'tree.copy' },
  { shortcut: 'Ctrl+X', command: 'tree.cut' },
  { shortcut: 'Ctrl+V', command: 'tree.paste' },
  { shortcut: 'Ctrl+Shift+C', command: 'tree.copy-path' },
  { shortcut: 'Ctrl+Alt+L', command: 'tree.format' },
  { shortcut: 'F11', command: 'tree.bookmark' },
];

export const globalKeybindings: Keybinding[] = [
  { shortcut: 'Ctrl+Shift+N', command: 'menu.go-to-file' },
  { shortcut: 'Alt+ArrowRight', command: 'menu.go-next-tab' },
  { shortcut: 'Alt+ArrowLeft', command: 'menu.go-prev-tab' },
  { shortcut: 'Alt+9', command: 'menu.git-open' },
  { shortcut: 'Alt+F12', command: 'menu.terminal-toggle' },
  { shortcut: 'Ctrl+Shift+K', command: 'menu.git-push' },
  { shortcut: 'Ctrl+K', command: 'menu.git-commit' },
  { shortcut: 'Ctrl+T', command: 'menu.git-pull' },
  { shortcut: 'Ctrl+Shift+`', command: 'terminal.new' },
  { shortcut: 'Ctrl+`', command: 'terminal.toggle' },
  { shortcut: 'Ctrl+Alt+\\', command: 'tab.move-next-group' },
  { shortcut: 'Ctrl+Shift+\\', command: 'tab.close-group' },
  { shortcut: 'Ctrl+\\', command: 'tab.split-right' },
  { shortcut: 'Ctrl+Shift+W', command: 'tab.close-all' },
  { shortcut: 'Ctrl+Alt+W', command: 'tab.close-others' },
  { shortcut: 'Ctrl+W', command: 'tab.close-current' },
  { shortcut: 'Ctrl+Alt+S', command: 'tab.close-saved' },
  { shortcut: 'Ctrl+Alt+L', command: 'menu.view-format-code' },
  { shortcut: 'Ctrl+Shift+L', command: 'ai.add-selection' },
  { shortcut: 'Ctrl+S', command: 'app.save' },
  { shortcut: 'Ctrl+O', command: 'app.open-file' },
  { shortcut: 'Ctrl+F', command: 'edit.find' },
  { shortcut: 'Ctrl+Z', command: 'edit.undo' },
  { shortcut: 'Ctrl+Y', command: 'edit.redo' },
  { shortcut: 'Ctrl+X', command: 'edit.cut' },
  { shortcut: 'Ctrl+C', command: 'edit.copy' },
  { shortcut: 'Ctrl+V', command: 'edit.paste' },
  { shortcut: 'Ctrl+A', command: 'edit.select-all' },
  { shortcut: 'Ctrl+,', command: 'settings.open' },
];

const normalizeKeyToken = (token: string) => token.trim().toLowerCase();

const eventKeyName = (event: KeyboardEvent) => {
  if (event.key === ' ') return 'space';
  if (event.key.length === 1) return event.key.toLowerCase();
  return event.key.toLowerCase();
};

export const matchesShortcut = (event: KeyboardEvent, shortcut: string) => {
  const tokens = shortcut.split('+').map(normalizeKeyToken);
  const needsCtrl = tokens.includes('ctrl');
  const needsShift = tokens.includes('shift');
  const needsAlt = tokens.includes('alt');
  const needsMeta = tokens.includes('meta');
  const keyToken = tokens.find((t) => !['ctrl', 'shift', 'alt', 'meta'].includes(t));
  if (!keyToken) return false;

  const ctrlOrMeta = event.ctrlKey || event.metaKey;
  if (needsCtrl !== ctrlOrMeta) return false;
  if (needsShift !== event.shiftKey) return false;
  if (needsAlt !== event.altKey) return false;
  if (needsMeta !== event.metaKey) return false;

  return eventKeyName(event) === keyToken;
};
