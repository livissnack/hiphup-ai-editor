const isMacPlatform = () => {
  if (typeof navigator === 'undefined') return false;
  const uaData = (navigator as Navigator & { userAgentData?: { platform?: string } }).userAgentData;
  const platform = (uaData?.platform || navigator.platform || '').toLowerCase();
  return platform.includes('mac');
};

export const formatShortcutForDisplay = (shortcut: string) => {
  if (!shortcut) return '';
  const isMac = isMacPlatform();
  if (!isMac) return shortcut;

  return shortcut
    .split(' ')
    .map((part) => part
      .split('+')
      .map((token) => {
        const key = token.trim().toLowerCase();
        if (key === 'ctrl') return 'Cmd';
        if (key === 'alt') return 'Option';
        return token;
      })
      .join('+'))
    .join(' ');
};
