import type { ShortcutHelpItem } from './menuConfig';

export const treeShortcutByAction: Record<string, string> = {
  'new-file': 'Alt+Insert',
  rename: 'Shift+F6',
  copy: 'Ctrl+C',
  cut: 'Ctrl+X',
  paste: 'Ctrl+V',
  'copy-path': 'Ctrl+Shift+C',
  format: 'Ctrl+Alt+L',
  bookmark: 'F11',
  delete: 'Delete',
};

export const getTreeShortcut = (action: string) => treeShortcutByAction[action] ?? '';

export const treeShortcutHelpItems: ShortcutHelpItem[] = [
  { category: 'Project Tree', key: 'Alt+Insert', action: 'New File', description: 'Create file at selected node' },
  { category: 'Project Tree', key: 'Shift+F6', action: 'Rename', description: 'Rename selected file/folder' },
  { category: 'Project Tree', key: 'Ctrl+C', action: 'Copy', description: 'Copy selected file/folder' },
  { category: 'Project Tree', key: 'Ctrl+X', action: 'Cut', description: 'Cut selected file/folder' },
  { category: 'Project Tree', key: 'Ctrl+V', action: 'Paste', description: 'Paste to selected folder' },
  { category: 'Project Tree', key: 'Ctrl+Shift+C', action: 'Copy Path', description: 'Copy selected path' },
  { category: 'Project Tree', key: 'Ctrl+Alt+L', action: 'Format Code', description: 'Format selected file' },
  { category: 'Project Tree', key: 'F11', action: 'Toggle Bookmark', description: 'Add/remove bookmark' },
  { category: 'Project Tree', key: 'Delete', action: 'Delete', description: 'Delete selected file/folder' },
];
