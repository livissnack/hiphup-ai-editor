export type MenuItem = {
  label: string;
  action?: string;
  separator?: boolean;
  shortcut?: string;
  children?: MenuItem[];
  header?: boolean;
};

export type FullMenuKey = 'file' | 'edit' | 'git' | 'view' | 'go' | 'terminal' | 'help';

export const menuOrder: Array<{ key: FullMenuKey; label: string }> = [
  { key: 'file', label: 'File' },
  { key: 'edit', label: 'Edit' },
  { key: 'git', label: 'Git' },
  { key: 'view', label: 'View' },
  { key: 'go', label: 'Go' },
  { key: 'terminal', label: 'Terminal' },
  { key: 'help', label: 'Help' },
];

const fileItems: MenuItem[] = [
  { label: 'Open File...', action: 'open-file', shortcut: 'Ctrl+O' },
  { label: 'Open Folder...', action: 'open-folder', shortcut: 'Ctrl+K Ctrl+O' },
  { separator: true, label: '-' },
  { label: 'Save', action: 'save-file', shortcut: 'Ctrl+S' },
  { label: 'Save All', action: 'save-all', shortcut: 'Ctrl+K S' },
  { separator: true, label: '-' },
  { label: 'Settings', action: 'open-settings', shortcut: 'Ctrl+,' },
];
const editItems: MenuItem[] = [
  { label: 'Undo', action: 'edit-undo', shortcut: 'Ctrl+Z' },
  { label: 'Redo', action: 'edit-redo', shortcut: 'Ctrl+Y' },
  { separator: true, label: '-' },
  { label: 'Cut', action: 'edit-cut', shortcut: 'Ctrl+X' },
  { label: 'Copy', action: 'edit-copy', shortcut: 'Ctrl+C' },
  { label: 'Paste', action: 'edit-paste', shortcut: 'Ctrl+V' },
  { separator: true, label: '-' },
  { label: 'Find', action: 'edit-find', shortcut: 'Ctrl+F' },
];
const gitItems: MenuItem[] = [
  { label: 'Tool Window: Git', action: 'git-open', shortcut: 'Alt+9' },
  { label: 'Commit…', action: 'git-commit', shortcut: 'Ctrl+K' },
  { label: 'Push…', action: 'git-push', shortcut: 'Ctrl+Shift+K' },
  { label: 'Update Project…', action: 'git-pull', shortcut: 'Ctrl+T' },
  { label: 'Fetch', action: 'git-fetch' },
  { label: 'Branches…', action: 'git-branches' },
  { separator: true, label: '-' },
  { label: 'Show Status', action: 'git-status' },
  { label: 'Show Diff', action: 'git-diff' },
  { separator: true, label: '-' },
  { label: 'Refresh', action: 'git-refresh', shortcut: 'Ctrl+R' },
  {
    label: 'More...',
    children: [
      { label: 'Branch', header: true },
      { label: 'Checkout…', action: 'git-checkout' },
      { label: 'Create Branch…', action: 'git-branch-create' },
      { label: 'Pull', action: 'git-pull' },
      { label: 'Merge…', action: 'git-merge' },
      { label: 'Rebase…', action: 'git-rebase' },
      { label: 'Cherry-pick…', action: 'git-cherry-pick' },
      { label: 'Revert…', action: 'git-revert' },
      { label: 'Reset…', action: 'git-reset' },
      { separator: true, label: '-' },
      { label: 'History', header: true },
      { label: 'Log (Head)', action: 'git-log-head' },
      { label: 'Local Branches', action: 'git-show-local' },
      { label: 'Remote Branches', action: 'git-show-remote' },
      { label: 'Tags', action: 'git-show-tags' },
      { separator: true, label: '-' },
      { label: 'Stash', header: true },
      { label: 'Stash Changes…', action: 'git-stash-save' },
      { label: 'Unstash Changes…', action: 'git-stash-pop' },
      { label: 'View Stashes…', action: 'git-stash-list' },
    ],
  },
];
const viewItems: MenuItem[] = [
  { label: 'Reformat Code', action: 'view-format-code', shortcut: 'Ctrl+Alt+L' },
  { separator: true, label: '-' },
  { label: 'Toggle Explorer', action: 'view-toggle-explorer' },
  { label: 'Toggle AI Panel', action: 'view-toggle-ai' },
  { label: 'Toggle Terminal', action: 'view-toggle-terminal' },
  { separator: true, label: '-' },
  { label: 'Open Settings', action: 'open-settings' },
];
const goItems: MenuItem[] = [
  { label: 'Go to File', action: 'go-to-file', shortcut: 'Ctrl+Shift+N' },
  { label: 'Next Tab', action: 'go-next-tab', shortcut: 'Alt+ArrowRight' },
  { label: 'Previous Tab', action: 'go-prev-tab', shortcut: 'Alt+ArrowLeft' },
];
const terminalItems: MenuItem[] = [
  { label: 'Toggle Terminal', action: 'terminal-toggle', shortcut: 'Ctrl+`' },
  { label: 'New Terminal', action: 'terminal-new', shortcut: 'Ctrl+Shift+`' },
  { label: 'Focus Terminal', action: 'terminal-focus' },
  { label: 'Rename Terminal', action: 'terminal-rename' },
  { label: 'Clear Terminal', action: 'terminal-clear' },
  { separator: true, label: '-' },
  { label: 'Kill Terminal', action: 'terminal-kill-active' },
];
const helpItems: MenuItem[] = [
  { label: 'About AI Editor', action: 'help-about' },
  { label: 'Keyboard Shortcuts', action: 'help-shortcuts' },
  { separator: true, label: '-' },
  { label: "Install 'ai-editor' CLI (PATH)", action: 'help-install-cli' },
];

export const menuMap: Record<FullMenuKey, MenuItem[]> = {
  file: fileItems,
  edit: editItems,
  git: gitItems,
  view: viewItems,
  go: goItems,
  terminal: terminalItems,
  help: helpItems,
};

export type ShortcutHelpItem = {
  category: string;
  key: string;
  action: string;
  description: string;
};

const describeAction = (label: string) => label.replace(/…/g, '').trim();

export const getMenuShortcutHelpItems = (): ShortcutHelpItem[] => {
  const out: ShortcutHelpItem[] = [];
  const walk = (category: string, items: MenuItem[]) => {
    for (const item of items) {
      if (item.separator || item.header) continue;
      if (item.shortcut) {
        out.push({
          category: category[0].toUpperCase() + category.slice(1),
          key: item.shortcut,
          action: describeAction(item.label),
          description: `${describeAction(item.label)} (${category})`,
        });
      }
      if (item.children?.length) walk(category, item.children);
    }
  };
  for (const [category, items] of Object.entries(menuMap) as Array<[FullMenuKey, MenuItem[]]>) {
    walk(category, items);
  }
  return out;
};
