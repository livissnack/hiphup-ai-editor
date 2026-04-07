<script setup lang="ts">
import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import twemoji from 'twemoji';
import { computed, nextTick, onBeforeUnmount, onMounted, ref, watch } from 'vue';

const props = defineProps<{
  title: string;
  position: 'left' | 'right';
  open: boolean;
}>();

const emit = defineEmits(['close']);

type ChatRole = 'user' | 'assistant' | 'system';
type ChatMessage = { role: ChatRole; content: string; imageUrl?: string };
type PersistedChatMessage = { role: ChatRole; content: string };
type ChatSession = {
  id: string;
  title: string;
  model: string;
  messages: PersistedChatMessage[];
  pinned: boolean;
  pinOrder: number;
  draftInput: string;
  updatedAt: number;
};
type PersistedChatState = {
  activeSessionId: string;
  sessions: ChatSession[];
};
type ExportedChatState = {
  version: 1;
  exportedAt: string;
  activeSessionId: string;
  sessions: ChatSession[];
};
type ModelOptionItem = { id: string; isFree: boolean; supportsImage: boolean };
type PendingCodeRef = {
  id: string;
  path: string;
  fileName: string;
  startLine: number;
  endLine: number;
  snippet: string;
};
type ChatResult = { content: string; model: string; switched: boolean; fallbackFailures?: string[] };
type ChatStreamChunkEvent = { requestId: string; chunk: string };
type AddCodeRefDetail = {
  path: string;
  fileName?: string;
  startLine?: number;
  endLine?: number;
  snippet: string;
};
type FailedRequestSnapshot = {
  model: string;
  messages: any[];
};

const messages = ref<ChatMessage[]>([
  { role: 'assistant', content: 'Hi, I am your AI assistant. Ask me anything about the current project.' },
]);
const input = ref('');
const loading = ref(false);
const autoContext = ref(true);
const model = ref('gpt-4o-mini');
const apiBaseUrl = ref('https://aihubmix.com/v1');
const apiKey = ref('');
const showSettings = ref(false);
const chatScrollRef = ref<HTMLElement | null>(null);
const modelOptions = ref<ModelOptionItem[]>([]);
const modelLoading = ref(false);
const modelLoadError = ref('');
const pendingImageDataUrl = ref('');
const pendingImageName = ref('');
const pendingCodeRefs = ref<PendingCodeRef[]>([]);
const previewCodeRefId = ref('');
const failedRequest = ref<FailedRequestSnapshot | null>(null);
const streamingAssistantMessage = ref<ChatMessage | null>(null);
const fileInputRef = ref<HTMLInputElement | null>(null);
const importSessionsInputRef = ref<HTMLInputElement | null>(null);
const modelSearch = ref('');
const modelSearchInputRef = ref<HTMLInputElement | null>(null);
const modelPicker = ref<'settings' | 'inline' | null>(null);
const showSessionMenu = ref(false);
const showMoreMenu = ref(false);
const sessionSearch = ref('');
const sessionSearchInputRef = ref<HTMLInputElement | null>(null);
const highlightedSessionId = ref('');
const modelPickerPlacement = ref<{ settings: 'up' | 'down'; inline: 'up' | 'down' }>({
  settings: 'down',
  inline: 'up',
});
const aiConfigLoaded = ref(false);
const chatStateLoaded = ref(false);
const chatSessions = ref<ChatSession[]>([]);
const activeSessionId = ref('');
let applyingSession = false;
let saveAiConfigTimer: number | null = null;
let saveAiChatStateTimer: number | null = null;
const modelSupportsImage = computed(() => {
  const current = modelOptions.value.find((m) => m.id === model.value);
  return !!current?.supportsImage;
});
const filteredModelOptions = computed(() => {
  const keyword = modelSearch.value.trim().toLowerCase();
  if (!keyword) return modelOptions.value;
  return modelOptions.value.filter((m) => m.id.toLowerCase().includes(keyword));
});
const sortedSessions = computed(() => [...chatSessions.value].sort((a, b) => b.updatedAt - a.updatedAt));
const filteredSessions = computed(() => {
  const key = sessionSearch.value.trim().toLowerCase();
  if (!key) return sortedSessions.value;
  return sortedSessions.value.filter((s) => s.title.toLowerCase().includes(key));
});
const pinnedSessions = computed(() => filteredSessions.value
  .filter((s) => s.pinned)
  .sort((a, b) => a.pinOrder - b.pinOrder || b.updatedAt - a.updatedAt));
const recentSessions = computed(() => filteredSessions.value.filter((s) => !s.pinned));
const flattenedVisibleSessions = computed(() => [...pinnedSessions.value, ...recentSessions.value]);
const sessionNavList = computed(() => [...chatSessions.value].sort((a, b) => b.updatedAt - a.updatedAt));
const activeSessionTitle = computed(() => {
  const active = chatSessions.value.find((s) => s.id === activeSessionId.value);
  return active?.title || 'Chat';
});

const makeSessionTitle = (list: PersistedChatMessage[]) => {
  const userMsg = list.find((m) => m.role === 'user' && m.content.trim());
  if (!userMsg) return 'New Chat';
  // Use code-point slicing so emoji is not cut into invalid surrogate pairs.
  return Array.from(userMsg.content.trim()).slice(0, 48).join('');
};

const createSession = (seed?: Partial<ChatSession>): ChatSession => ({
  id: seed?.id || `chat-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
  title: seed?.title || 'New Chat',
  model: seed?.model || model.value || 'gpt-4o-mini',
  pinned: !!seed?.pinned,
  pinOrder: Number(seed?.pinOrder || 0),
  messages: (seed?.messages?.length ? seed.messages : [{ role: 'assistant', content: 'New chat started. What should we work on?' }])
    .map((m) => ({ role: m.role, content: m.content })),
  draftInput: seed?.draftInput || '',
  updatedAt: Number(seed?.updatedAt || Date.now()),
});

const applySessionToView = (session: ChatSession) => {
  applyingSession = true;
  model.value = session.model || 'gpt-4o-mini';
  messages.value = session.messages.length
    ? session.messages.map((m) => ({ role: m.role, content: m.content }))
    : [{ role: 'assistant', content: 'New chat started. What should we work on?' }];
  input.value = session.draftInput || '';
  pendingImageDataUrl.value = '';
  pendingImageName.value = '';
  pendingCodeRefs.value = [];
  previewCodeRefId.value = '';
  failedRequest.value = null;
  window.setTimeout(() => {
    applyingSession = false;
  }, 0);
};

const syncActiveSessionFromView = () => {
  if (applyingSession) return;
  const idx = chatSessions.value.findIndex((s) => s.id === activeSessionId.value);
  if (idx < 0) return;
  const cleanMessages = messages.value
    .filter((m) => ['user', 'assistant', 'system'].includes(m.role))
    .map((m) => ({ role: m.role, content: String(m.content || '') }))
    .filter((m) => m.content.trim())
    .slice(-80);
  const nextTitle = makeSessionTitle(cleanMessages);
  chatSessions.value[idx] = {
    ...chatSessions.value[idx],
    model: model.value.trim() || 'gpt-4o-mini',
    messages: cleanMessages,
    draftInput: input.value,
    title: nextTitle || chatSessions.value[idx].title,
    updatedAt: Date.now(),
  };
};

const loadAiConfig = async () => {
  try {
    const config = await invoke<{ baseUrl: string; apiKey: string; model: string }>('load_ai_config');
    if (config?.baseUrl) apiBaseUrl.value = config.baseUrl;
    if (typeof config?.apiKey === 'string') apiKey.value = config.apiKey;
    if (typeof config?.model === 'string' && config.model.trim()) model.value = config.model.trim();
  } catch {
    // Keep defaults if backend state is not available.
  } finally {
    aiConfigLoaded.value = true;
  }
};

const normalizePersistedMessage = (value: any): PersistedChatMessage | null => {
  const role = String(value?.role || '').trim() as ChatRole;
  const content = String(value?.content ?? '').trim();
  if (!['user', 'assistant', 'system'].includes(role)) return null;
  if (!content) return null;
  return { role, content };
};

const loadAiChatState = async () => {
  try {
    const state = await invoke<PersistedChatState | null>('load_ai_chat_state');
    if (state && Array.isArray(state.sessions) && state.sessions.length) {
      const sessions = state.sessions
        .map((item) => {
          const safeMessages = Array.isArray(item?.messages)
            ? item.messages
                .map((m: any) => normalizePersistedMessage(m))
                .filter((m: PersistedChatMessage | null): m is PersistedChatMessage => !!m)
                .slice(-80)
            : [];
          if (!safeMessages.length) return null;
          return createSession({
            id: String(item?.id || ''),
            title: String(item?.title || ''),
            model: String(item?.model || ''),
            pinned: !!item?.pinned,
            pinOrder: Number(item?.pinOrder || item?.pin_order || 0),
            draftInput: String(item?.draftInput || ''),
            updatedAt: Number(item?.updatedAt || 0),
            messages: safeMessages,
          });
        })
        .filter((s): s is ChatSession => !!s);
      if (sessions.length) {
        chatSessions.value = sessions;
        const active = sessions.find((s) => s.id === state.activeSessionId) || sessions[0];
        activeSessionId.value = active.id;
        applySessionToView(active);
      }
    }
    if (!chatSessions.value.length) {
      const session = createSession();
      chatSessions.value = [session];
      activeSessionId.value = session.id;
      applySessionToView(session);
    }
  } catch {
    if (!chatSessions.value.length) {
      const session = createSession();
      chatSessions.value = [session];
      activeSessionId.value = session.id;
      applySessionToView(session);
    }
  } finally {
    chatStateLoaded.value = true;
  }
};

const scheduleSaveAiChatState = () => {
  if (!chatStateLoaded.value) return;
  syncActiveSessionFromView();
  if (saveAiChatStateTimer !== null) {
    window.clearTimeout(saveAiChatStateTimer);
  }
  saveAiChatStateTimer = window.setTimeout(() => {
    const payload: PersistedChatState = {
      activeSessionId: activeSessionId.value,
      sessions: chatSessions.value.slice(-20).map((s) => ({
        ...s,
        messages: s.messages.slice(-80),
      })),
    };
    void invoke('save_ai_chat_state', { state: payload });
    saveAiChatStateTimer = null;
  }, 240);
};

const scheduleSaveAiConfig = () => {
  if (!aiConfigLoaded.value) return;
  if (saveAiConfigTimer !== null) {
    window.clearTimeout(saveAiConfigTimer);
  }
  saveAiConfigTimer = window.setTimeout(() => {
    void invoke('save_ai_config', {
      config: {
        baseUrl: apiBaseUrl.value.trim() || 'https://aihubmix.com/v1',
        apiKey: apiKey.value.trim(),
        model: model.value.trim() || 'gpt-4o-mini',
      },
    });
    saveAiConfigTimer = null;
  }, 220);
};

const loadModelOptions = async () => {
  modelLoading.value = true;
  modelLoadError.value = '';
  try {
    const response = await fetch('https://aihubmix.com/api/v1/models');
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    const data = await response.json();
    const ids = Array.isArray(data?.data)
      ? data.data
          .map((item: any) => String(item?.model_id || '').trim())
          .filter((v: string) => !!v)
      : [];
    const itemMap = new Map<string, ModelOptionItem>();
    if (Array.isArray(data?.data)) {
      for (const item of data.data) {
        const id = String(item?.model_id || '').trim();
        if (!id) continue;
        const modalities = String(item?.input_modalities || '').toLowerCase();
        const supportsImage = modalities
          .split(',')
          .map((s: string) => s.trim())
          .includes('image');
        itemMap.set(id, { id, isFree: /-free$/i.test(id), supportsImage });
      }
    }
    modelOptions.value = Array.from(itemMap.values())
      .sort((a, b) => {
        if (a.isFree !== b.isFree) return a.isFree ? -1 : 1;
        return a.id.localeCompare(b.id);
      });
    if (!modelOptions.value.length) throw new Error('Empty model list.');
    if (!modelOptions.value.some((m) => m.id === model.value)) {
      model.value = modelOptions.value[0].id;
    }
  } catch (error) {
    modelLoadError.value = `Load models failed: ${String(error)}`;
  } finally {
    modelLoading.value = false;
  }
};

const scrollToBottom = async () => {
  await nextTick();
  if (!chatScrollRef.value) return;
  chatScrollRef.value.scrollTop = chatScrollRef.value.scrollHeight;
};

watch(messages, () => {
  void scrollToBottom();
}, { deep: true });

watch([apiBaseUrl, apiKey, model], () => {
  scheduleSaveAiConfig();
});
watch([messages, model], () => {
  scheduleSaveAiChatState();
}, { deep: true });
watch(input, () => {
  scheduleSaveAiChatState();
});
watch(modelSupportsImage, (supports) => {
  if (!supports && pendingImageDataUrl.value) {
    clearPendingImage();
  }
});

onMounted(() => {
  void (async () => {
    await loadAiConfig();
    await loadAiChatState();
  })();
  void loadModelOptions();
  window.addEventListener('pointerdown', onGlobalPointerDown);
  window.addEventListener('keydown', onWindowKeydown, { capture: true });
  window.addEventListener('ai-add-code-ref', onAddCodeRefEvent as EventListener);
});

onBeforeUnmount(() => {
  window.removeEventListener('pointerdown', onGlobalPointerDown);
  window.removeEventListener('keydown', onWindowKeydown, { capture: true });
  window.removeEventListener('ai-add-code-ref', onAddCodeRefEvent as EventListener);
  if (saveAiConfigTimer !== null) {
    window.clearTimeout(saveAiConfigTimer);
    saveAiConfigTimer = null;
  }
  if (saveAiChatStateTimer !== null) {
    window.clearTimeout(saveAiChatStateTimer);
    saveAiChatStateTimer = null;
  }
});

const onGlobalPointerDown = (event: PointerEvent) => {
  const target = event.target as HTMLElement | null;
  if (!target) return;
  if (target.closest('.model-picker')) return;
  if (target.closest('.chat-session-picker')) return;
  if (target.closest('.history-btn')) return;
  if (target.closest('.header-more')) return;
  if (target.closest('.more-menu')) return;
  modelPicker.value = null;
  showSessionMenu.value = false;
  showMoreMenu.value = false;
};

const resolvePickerPlacement = (where: 'settings' | 'inline', triggerEl?: HTMLElement | null) => {
  if (!triggerEl) return;
  const rect = triggerEl.getBoundingClientRect();
  const viewportHeight = window.innerHeight;
  const estimatedMenuHeight = Math.min(220, Math.max(120, modelOptions.value.length * 30));
  const spaceBelow = viewportHeight - rect.bottom;
  const spaceAbove = rect.top;
  modelPickerPlacement.value[where] =
    spaceBelow >= estimatedMenuHeight || spaceBelow >= spaceAbove ? 'down' : 'up';
};

const toggleModelPicker = (where: 'settings' | 'inline', event?: MouseEvent) => {
  const triggerEl = event?.currentTarget as HTMLElement | null;
  resolvePickerPlacement(where, triggerEl);
  modelPicker.value = modelPicker.value === where ? null : where;
  if (!modelPicker.value) modelSearch.value = '';
};

const selectModel = (value: string) => {
  model.value = value;
  modelPicker.value = null;
  modelSearch.value = '';
};

const isFreeModel = (value: string) => /-free$/i.test(value);
const supportsImageModel = (value: string) => modelOptions.value.some((m) => m.id === value && m.supportsImage);

const getEditorContext = () => {
  const path = (window as any).__IDE_ACTIVE_FILE_PATH__ as string | undefined;
  const content = (window as any).__IDE_ACTIVE_FILE_CONTENT__ as string | undefined;
  if (!autoContext.value || !path || !content) return '';
  const snippet = content.length > 6000 ? `${content.slice(0, 6000)}\n...` : content;
  return `Current file: ${path}\n\nFile content:\n${snippet}`;
};

const newChat = () => {
  const session = createSession();
  chatSessions.value = [session, ...chatSessions.value].slice(0, 20);
  activeSessionId.value = session.id;
  showSessionMenu.value = false;
  applySessionToView(session);
  scheduleSaveAiChatState();
};

const switchSession = (id: string) => {
  syncActiveSessionFromView();
  const target = chatSessions.value.find((s) => s.id === id);
  if (!target) return;
  activeSessionId.value = target.id;
  showSessionMenu.value = false;
  applySessionToView(target);
  scheduleSaveAiChatState();
};

const toggleSessionMenu = () => {
  showMoreMenu.value = false;
  showSessionMenu.value = !showSessionMenu.value;
  if (!showSessionMenu.value) {
    sessionSearch.value = '';
    highlightedSessionId.value = '';
    return;
  }
  highlightedSessionId.value = activeSessionId.value;
  void nextTick(() => {
    sessionSearchInputRef.value?.focus();
    sessionSearchInputRef.value?.select();
  });
};

const toggleMoreMenu = () => {
  showSessionMenu.value = false;
  showMoreMenu.value = !showMoreMenu.value;
};

const renameSession = (id: string) => {
  const target = chatSessions.value.find((s) => s.id === id);
  if (!target) return;
  const next = window.prompt('Rename chat', target.title)?.trim();
  if (!next || next === target.title) return;
  const idx = chatSessions.value.findIndex((s) => s.id === id);
  if (idx < 0) return;
  chatSessions.value[idx] = { ...chatSessions.value[idx], title: next, updatedAt: Date.now() };
  scheduleSaveAiChatState();
};

const deleteSession = (id: string) => {
  const target = chatSessions.value.find((s) => s.id === id);
  if (!target) return;
  if (chatSessions.value.length <= 1) {
    const session = createSession();
    chatSessions.value = [session];
    activeSessionId.value = session.id;
    applySessionToView(session);
    scheduleSaveAiChatState();
    return;
  }
  chatSessions.value = chatSessions.value.filter((s) => s.id !== id);
  if (activeSessionId.value === id) {
    const next = [...chatSessions.value].sort((a, b) => b.updatedAt - a.updatedAt)[0];
    if (next) {
      activeSessionId.value = next.id;
      applySessionToView(next);
    }
  }
  scheduleSaveAiChatState();
};

const togglePinSession = (id: string) => {
  const idx = chatSessions.value.findIndex((s) => s.id === id);
  if (idx < 0) return;
  const willPin = !chatSessions.value[idx].pinned;
  const maxOrder = Math.max(0, ...chatSessions.value.filter((s) => s.pinned).map((s) => s.pinOrder));
  chatSessions.value[idx] = {
    ...chatSessions.value[idx],
    pinned: willPin,
    pinOrder: willPin ? maxOrder + 1 : 0,
    updatedAt: Date.now(),
  };
  scheduleSaveAiChatState();
};

const movePinnedSession = (id: string, direction: -1 | 1) => {
  const pinned = [...chatSessions.value.filter((s) => s.pinned)].sort((a, b) => a.pinOrder - b.pinOrder);
  const idx = pinned.findIndex((s) => s.id === id);
  if (idx < 0) return;
  const targetIdx = idx + direction;
  if (targetIdx < 0 || targetIdx >= pinned.length) return;
  const a = pinned[idx];
  const b = pinned[targetIdx];
  const aIndex = chatSessions.value.findIndex((s) => s.id === a.id);
  const bIndex = chatSessions.value.findIndex((s) => s.id === b.id);
  if (aIndex < 0 || bIndex < 0) return;
  const aOrder = chatSessions.value[aIndex].pinOrder;
  chatSessions.value[aIndex] = { ...chatSessions.value[aIndex], pinOrder: chatSessions.value[bIndex].pinOrder };
  chatSessions.value[bIndex] = { ...chatSessions.value[bIndex], pinOrder: aOrder };
  scheduleSaveAiChatState();
};

const exportChatSessions = () => {
  const payload: ExportedChatState = {
    version: 1,
    exportedAt: new Date().toISOString(),
    activeSessionId: activeSessionId.value,
    sessions: chatSessions.value,
  };
  const blob = new Blob([JSON.stringify(payload, null, 2)], { type: 'application/json' });
  const url = URL.createObjectURL(blob);
  const a = document.createElement('a');
  a.href = url;
  a.download = `hiphup-editor-ai-chats-${new Date().toISOString().slice(0, 19).replace(/[:T]/g, '-')}.json`;
  document.body.appendChild(a);
  a.click();
  a.remove();
  URL.revokeObjectURL(url);
  showMoreMenu.value = false;
  messages.value.push({ role: 'system', content: 'Chat sessions exported.' });
};

const openImportSessionsPicker = () => {
  showMoreMenu.value = false;
  importSessionsInputRef.value?.click();
};

const normalizeImportedSession = (raw: any): ChatSession | null => {
  if (!raw || typeof raw !== 'object') return null;
  const safeMessages = Array.isArray(raw.messages)
    ? raw.messages
        .map((m: any) => normalizePersistedMessage(m))
        .filter((m: PersistedChatMessage | null): m is PersistedChatMessage => !!m)
        .slice(-80)
    : [];
  if (!safeMessages.length) return null;
  return createSession({
    id: String(raw.id || ''),
    title: String(raw.title || ''),
    model: String(raw.model || ''),
    pinned: !!raw.pinned,
    pinOrder: Number(raw.pinOrder || raw.pin_order || 0),
    draftInput: String(raw.draftInput || raw.draft_input || ''),
    updatedAt: Number(raw.updatedAt || raw.updated_at || Date.now()),
    messages: safeMessages,
  });
};

const onImportSessionsFile = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;
  try {
    const text = await file.text();
    const json = JSON.parse(text);
    const list = Array.isArray(json?.sessions) ? json.sessions : [];
    const imported = list
      .map((item) => normalizeImportedSession(item))
      .filter((s): s is ChatSession => !!s)
      .slice(0, 100);
    if (!imported.length) {
      messages.value.push({ role: 'system', content: 'Import failed: no valid sessions found.' });
      return;
    }
    const existingIds = new Set(chatSessions.value.map((s) => s.id));
    const merged = [...chatSessions.value];
    for (const session of imported) {
      let id = session.id;
      if (!id || existingIds.has(id)) {
        id = `chat-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`;
      }
      existingIds.add(id);
      merged.push({ ...session, id });
    }
    chatSessions.value = merged.sort((a, b) => b.updatedAt - a.updatedAt).slice(0, 100);
    const preferred = String(json?.activeSessionId || '').trim();
    const active = chatSessions.value.find((s) => s.id === preferred) || chatSessions.value[0];
    if (active) {
      activeSessionId.value = active.id;
      applySessionToView(active);
    }
    scheduleSaveAiChatState();
    messages.value.push({ role: 'system', content: `Imported ${imported.length} chat session(s).` });
  } catch (error) {
    messages.value.push({ role: 'system', content: `Import failed: ${String(error)}` });
  } finally {
    target.value = '';
  }
};

const pruneOldSessions = (keepCount = 20) => {
  const safeKeep = Math.max(1, Math.min(100, keepCount));
  const keepIds = new Set(
    [...chatSessions.value]
      .sort((a, b) => b.updatedAt - a.updatedAt)
      .slice(0, safeKeep)
      .map((s) => s.id),
  );
  if (activeSessionId.value) keepIds.add(activeSessionId.value);
  const before = chatSessions.value.length;
  chatSessions.value = chatSessions.value.filter((s) => keepIds.has(s.id));
  if (!chatSessions.value.length) {
    const session = createSession();
    chatSessions.value = [session];
    activeSessionId.value = session.id;
    applySessionToView(session);
  } else if (!chatSessions.value.some((s) => s.id === activeSessionId.value)) {
    const next = [...chatSessions.value].sort((a, b) => b.updatedAt - a.updatedAt)[0];
    if (next) {
      activeSessionId.value = next.id;
      applySessionToView(next);
    }
  }
  scheduleSaveAiChatState();
  showMoreMenu.value = false;
  messages.value.push({ role: 'system', content: `Pruned ${Math.max(0, before - chatSessions.value.length)} old chat session(s).` });
};

const cleanupEmptySessions = () => {
  const before = chatSessions.value.length;
  chatSessions.value = chatSessions.value.filter((s) => {
    const meaningful = s.messages.some((m) => m.role === 'user' && m.content.trim());
    return meaningful || s.id === activeSessionId.value;
  });
  if (!chatSessions.value.length) {
    const session = createSession();
    chatSessions.value = [session];
    activeSessionId.value = session.id;
    applySessionToView(session);
  }
  scheduleSaveAiChatState();
  showMoreMenu.value = false;
  messages.value.push({ role: 'system', content: `Cleaned ${Math.max(0, before - chatSessions.value.length)} empty chat session(s).` });
};

const resetAllSessions = () => {
  const ok = window.confirm('Reset all chat sessions? This cannot be undone.');
  if (!ok) return;
  const session = createSession();
  chatSessions.value = [session];
  activeSessionId.value = session.id;
  applySessionToView(session);
  scheduleSaveAiChatState();
  showMoreMenu.value = false;
  messages.value.push({ role: 'system', content: 'All chat sessions were reset.' });
};

const clearCurrentChatScreen = () => {
  if (loading.value) return;
  messages.value = [{ role: 'assistant', content: 'Chat cleared. What should we work on next?' }];
  input.value = '';
  failedRequest.value = null;
  clearPendingImage();
  clearPendingCodeRefs();
  scheduleSaveAiChatState();
  showMoreMenu.value = false;
};

const cycleSession = (direction: 1 | -1) => {
  const list = sessionNavList.value;
  if (list.length < 2) return;
  const currentIdx = list.findIndex((s) => s.id === activeSessionId.value);
  const nextIdx = currentIdx < 0
    ? 0
    : (currentIdx + direction + list.length) % list.length;
  const next = list[nextIdx];
  if (!next || next.id === activeSessionId.value) return;
  switchSession(next.id);
  messages.value.push({ role: 'system', content: `Switched to chat: ${next.title}` });
};

const onWindowKeydown = (event: KeyboardEvent) => {
  if (!(event.ctrlKey || event.metaKey) || !event.shiftKey) return;
  const key = event.key;
  const code = event.code;
  const isPrev = key === '[' || code === 'BracketLeft';
  const isNext = key === ']' || code === 'BracketRight';
  if (!isPrev && !isNext) return;
  event.preventDefault();
  event.stopPropagation();
  cycleSession(isNext ? 1 : -1);
};

const moveSessionHighlight = (direction: 1 | -1) => {
  const list = flattenedVisibleSessions.value;
  if (!list.length) return;
  const current = list.findIndex((s) => s.id === highlightedSessionId.value);
  const next = current < 0
    ? (direction > 0 ? 0 : list.length - 1)
    : (current + direction + list.length) % list.length;
  highlightedSessionId.value = list[next].id;
};

const onSessionSearchKeydown = (event: KeyboardEvent) => {
  if (!showSessionMenu.value) return;
  if (event.key === 'ArrowDown') {
    event.preventDefault();
    moveSessionHighlight(1);
    return;
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault();
    moveSessionHighlight(-1);
    return;
  }
  if (event.key === 'Enter') {
    event.preventDefault();
    if (highlightedSessionId.value) {
      switchSession(highlightedSessionId.value);
    }
    return;
  }
  if (event.key === 'Escape') {
    event.preventDefault();
    showSessionMenu.value = false;
    sessionSearch.value = '';
    highlightedSessionId.value = '';
  }
};

const askQuick = (prompt: string) => {
  input.value = prompt;
  void send();
};

const openImagePicker = () => {
  fileInputRef.value?.click();
};

const clearPendingImage = () => {
  pendingImageDataUrl.value = '';
  pendingImageName.value = '';
  if (fileInputRef.value) fileInputRef.value.value = '';
};

const clearPendingCodeRefs = () => {
  pendingCodeRefs.value = [];
  previewCodeRefId.value = '';
};

const removePendingCodeRef = (id: string) => {
  pendingCodeRefs.value = pendingCodeRefs.value.filter((item) => item.id !== id);
  if (previewCodeRefId.value === id) previewCodeRefId.value = '';
};

const togglePreviewCodeRef = (id: string) => {
  previewCodeRefId.value = previewCodeRefId.value === id ? '' : id;
};

const fileNameFromPath = (path: string) => {
  const normalized = path.replace(/\\/g, '/');
  const parts = normalized.split('/');
  return parts[parts.length - 1] || path;
};

const resolveSnippetRange = (fileContent: string, snippet: string) => {
  const content = fileContent.replace(/\r\n/g, '\n');
  const directIndex = content.indexOf(snippet);
  let matchIndex = directIndex;
  let matchedSnippet = snippet;
  if (matchIndex < 0) {
    const trimmed = snippet.trim();
    if (!trimmed) return null;
    matchIndex = content.indexOf(trimmed);
    matchedSnippet = trimmed;
  }
  if (matchIndex < 0) return null;
  const startLine = content.slice(0, matchIndex).split('\n').length;
  const lineCount = Math.max(1, matchedSnippet.split('\n').length);
  return {
    startLine,
    endLine: startLine + lineCount - 1,
  };
};

const onPickImage = async (event: Event) => {
  const target = event.target as HTMLInputElement;
  const file = target.files?.[0];
  if (!file) return;
  if (!file.type.startsWith('image/')) return;
  const reader = new FileReader();
  reader.onload = () => {
    pendingImageDataUrl.value = typeof reader.result === 'string' ? reader.result : '';
    pendingImageName.value = file.name;
  };
  reader.readAsDataURL(file);
};

const onInputPaste = (event: ClipboardEvent) => {
  const text = event.clipboardData?.getData('text/plain') || '';
  if (!text.trim()) return;
  const normalized = text.replace(/\r\n/g, '\n').trim();
  if (normalized.split('\n').length < 2) return;

  const path = String((window as any).__IDE_ACTIVE_FILE_PATH__ || '').trim();
  const content = String((window as any).__IDE_ACTIVE_FILE_CONTENT__ || '');
  if (!path || !content) return;

  const range = resolveSnippetRange(content, normalized);
  if (!range) return;

  event.preventDefault();
  const id = `${path}:${range.startLine}-${range.endLine}:${normalized.slice(0, 80)}`;
  if (pendingCodeRefs.value.some((item) => item.id === id)) return;
  pendingCodeRefs.value = [
    ...pendingCodeRefs.value,
    {
      id,
      path,
      fileName: fileNameFromPath(path),
      startLine: range.startLine,
      endLine: range.endLine,
      snippet: normalized,
    },
  ].slice(-8);
};

const addCodeRef = (detail: AddCodeRefDetail) => {
  const path = String(detail.path || '').trim();
  const snippet = String(detail.snippet || '').replace(/\r\n/g, '\n').trim();
  if (!path || !snippet) return;
  const fileName = String(detail.fileName || '').trim() || fileNameFromPath(path);
  const startLine = Number(detail.startLine || 0);
  const endLine = Number(detail.endLine || 0);
  const safeStart = Number.isFinite(startLine) && startLine > 0 ? startLine : 1;
  const safeEnd = Number.isFinite(endLine) && endLine >= safeStart ? endLine : safeStart + Math.max(0, snippet.split('\n').length - 1);
  const id = `${path}:${safeStart}-${safeEnd}:${snippet.slice(0, 80)}`;
  if (pendingCodeRefs.value.some((item) => item.id === id)) return;
  pendingCodeRefs.value = [
    ...pendingCodeRefs.value,
    { id, path, fileName, startLine: safeStart, endLine: safeEnd, snippet },
  ].slice(-8);
};

const onAddCodeRefEvent = (event: Event) => {
  const custom = event as CustomEvent<AddCodeRefDetail>;
  if (!custom?.detail) return;
  addCodeRef(custom.detail);
};

const buildCodeRefContext = () => {
  if (!pendingCodeRefs.value.length) return '';
  return pendingCodeRefs.value
    .map((item) => `Code reference: ${item.path}:${item.startLine}-${item.endLine}\n\n${item.snippet}`)
    .join('\n\n---\n\n');
};

const getCodeRefLabel = () => {
  if (!pendingCodeRefs.value.length) return '';
  if (pendingCodeRefs.value.length === 1) return '[Code]';
  return `[Code x${pendingCodeRefs.value.length}]`;
};

const AI_RETRY_ATTEMPTS = 3;
const AI_RETRY_BASE_DELAY_MS = 500;
const sleep = (ms: number) => new Promise((resolve) => window.setTimeout(resolve, ms));

const requestChatWithRetry = async (payload: {
  model: string;
  messages: any[];
  temperature: number;
  stream: boolean;
}) => {
  let lastError: unknown = null;
  for (let attempt = 1; attempt <= AI_RETRY_ATTEMPTS; attempt += 1) {
    try {
      const result = await invoke<ChatResult>('aihub_chat', { payload });
      if (attempt > 1) {
        messages.value.push({
          role: 'system',
          content: `Recovered after retry (attempt ${attempt}/${AI_RETRY_ATTEMPTS}).`,
        });
      }
      return result;
    } catch (error) {
      lastError = error;
      if (attempt < AI_RETRY_ATTEMPTS) {
        const waitMs = AI_RETRY_BASE_DELAY_MS * attempt;
        messages.value.push({
          role: 'system',
          content: `Request failed (attempt ${attempt}/${AI_RETRY_ATTEMPTS}), retrying in ${waitMs}ms...`,
        });
        await sleep(waitMs);
      }
    }
  }
  throw lastError;
};

const escapeHtml = (text: string) => text
  .replace(/&/g, '&amp;')
  .replace(/</g, '&lt;')
  .replace(/>/g, '&gt;')
  .replace(/"/g, '&quot;')
  .replace(/'/g, '&#39;');

const escapeAttr = (text: string) => escapeHtml(text).replace(/`/g, '&#96;');

const sanitizeUrl = (url: string) => {
  const trimmed = url.trim();
  if (/^https?:\/\//i.test(trimmed)) return trimmed;
  return '';
};

const renderInlineMarkdown = (raw: string) => {
  let text = escapeHtml(raw);
  text = text.replace(/`([^`\n]+)`/g, '<code>$1</code>');
  text = text.replace(/\[([^\]]+)\]\(([^)]+)\)/g, (_m, label: string, url: string) => {
    const safeHref = sanitizeUrl(url);
    if (!safeHref) return label;
    return `<a href="${escapeAttr(safeHref)}" target="_blank" rel="noopener noreferrer">${label}</a>`;
  });
  text = text.replace(/\*\*([^*\n]+)\*\*/g, '<strong>$1</strong>');
  text = text.replace(/\*([^*\n]+)\*/g, '<em>$1</em>');
  return text;
};

const encodeUtf8Base64 = (text: string) => {
  try {
    return btoa(unescape(encodeURIComponent(text)));
  } catch {
    return '';
  }
};

const decodeUtf8Base64 = (text: string) => {
  try {
    return decodeURIComponent(escape(atob(text)));
  } catch {
    return '';
  }
};

const splitTableRow = (line: string) => line
  .trim()
  .replace(/^\|/, '')
  .replace(/\|$/, '')
  .split('|')
  .map((cell) => cell.trim());

const isTableSeparator = (line: string) => {
  const row = line.trim();
  if (!row.includes('|')) return false;
  const cells = splitTableRow(row);
  if (!cells.length) return false;
  return cells.every((cell) => /^:?-{3,}:?$/.test(cell));
};

const renderMarkdown = (raw: string) => {
  const lines = raw.replace(/\r\n/g, '\n').split('\n');
  const out: string[] = [];
  let paragraph: string[] = [];
  let listType: 'ul' | 'ol' | 'task' | '' = '';
  let inCode = false;
  let codeLang = '';
  let codeLines: string[] = [];

  const flushParagraph = () => {
    if (!paragraph.length) return;
    out.push(`<p>${renderInlineMarkdown(paragraph.join(' '))}</p>`);
    paragraph = [];
  };
  const closeList = () => {
    if (!listType) return;
    out.push('</ul>');
    if (listType === 'ol') {
      out[out.length - 1] = '</ol>';
    }
    listType = '';
  };
  const flushCode = () => {
    if (!inCode) return;
    const codeText = codeLines.join('\n');
    const codeB64 = encodeUtf8Base64(codeText);
    const cls = codeLang ? ` class="language-${escapeAttr(codeLang)}"` : '';
    out.push(
      `<div class="md-code-block">`
      + `<div class="md-code-head">`
      + `<span class="md-code-lang">${escapeHtml(codeLang || 'text')}</span>`
      + `<button class="md-code-copy-btn" data-copy-code="${escapeAttr(codeB64)}">Copy</button>`
      + '</div>'
      + `<pre><code${cls}>${escapeHtml(codeText)}</code></pre>`
      + '</div>',
    );
    inCode = false;
    codeLang = '';
    codeLines = [];
  };

  let i = 0;
  while (i < lines.length) {
    const line = lines[i];
    const fence = line.match(/^```(\w+)?\s*$/);
    if (fence) {
      flushParagraph();
      closeList();
      if (inCode) {
        flushCode();
      } else {
        inCode = true;
        codeLang = (fence[1] || '').trim();
      }
      i += 1;
      continue;
    }

    if (inCode) {
      codeLines.push(line);
      i += 1;
      continue;
    }

    if (!line.trim()) {
      flushParagraph();
      closeList();
      i += 1;
      continue;
    }

    const nextLine = lines[i + 1] || '';
    if (line.includes('|') && isTableSeparator(nextLine)) {
      flushParagraph();
      closeList();
      const headers = splitTableRow(line);
      const rows: string[][] = [];
      i += 2;
      while (i < lines.length && lines[i].trim() && lines[i].includes('|')) {
        rows.push(splitTableRow(lines[i]));
        i += 1;
      }
      out.push('<div class="md-table-wrap"><table><thead><tr>');
      for (const h of headers) out.push(`<th>${renderInlineMarkdown(h)}</th>`);
      out.push('</tr></thead><tbody>');
      for (const row of rows) {
        out.push('<tr>');
        for (let c = 0; c < headers.length; c += 1) {
          out.push(`<td>${renderInlineMarkdown(row[c] || '')}</td>`);
        }
        out.push('</tr>');
      }
      out.push('</tbody></table></div>');
      continue;
    }

    const heading = line.match(/^(#{1,4})\s+(.+)$/);
    if (heading) {
      flushParagraph();
      closeList();
      const level = heading[1].length;
      out.push(`<h${level}>${renderInlineMarkdown(heading[2])}</h${level}>`);
      i += 1;
      continue;
    }

    if (/^---+$/.test(line.trim())) {
      flushParagraph();
      closeList();
      out.push('<hr>');
      i += 1;
      continue;
    }

    const quote = line.match(/^>\s?(.*)$/);
    if (quote) {
      flushParagraph();
      closeList();
      out.push(`<blockquote>${renderInlineMarkdown(quote[1])}</blockquote>`);
      i += 1;
      continue;
    }

    const ordered = line.match(/^\d+\.\s+(.+)$/);
    if (ordered) {
      flushParagraph();
      if (listType && listType !== 'ol') closeList();
      if (!listType) {
        listType = 'ol';
        out.push('<ol>');
      }
      out.push(`<li>${renderInlineMarkdown(ordered[1])}</li>`);
      i += 1;
      continue;
    }

    const task = line.match(/^[-*]\s+\[( |x|X)\]\s+(.+)$/);
    if (task) {
      flushParagraph();
      if (listType && listType !== 'task') closeList();
      if (!listType) {
        listType = 'task';
        out.push('<ul class="task-list">');
      }
      const checked = task[1].toLowerCase() === 'x' ? ' checked' : '';
      out.push(
        `<li><label><input type="checkbox" disabled${checked}>`
        + `<span>${renderInlineMarkdown(task[2])}</span></label></li>`,
      );
      i += 1;
      continue;
    }

    const unordered = line.match(/^[-*]\s+(.+)$/);
    if (unordered) {
      flushParagraph();
      if (listType && listType !== 'ul') closeList();
      if (!listType) {
        listType = 'ul';
        out.push('<ul>');
      }
      out.push(`<li>${renderInlineMarkdown(unordered[1])}</li>`);
      i += 1;
      continue;
    }

    paragraph.push(line.trim());
    i += 1;
  }

  flushParagraph();
  closeList();
  flushCode();
  return out.join('');
};

const twemojiOptions = {
  base: 'https://cdn.jsdelivr.net/gh/twitter/twemoji@14.0.2/assets/',
  folder: 'svg',
  ext: '.svg',
  className: 'twemoji',
} as const;

const renderAssistantContent = (text: string) => twemoji.parse(renderMarkdown(text || ''), twemojiOptions);
const renderSessionTitle = (text: string) => twemoji.parse(escapeHtml(text || ''), twemojiOptions);

const onMarkdownClick = async (event: MouseEvent) => {
  const target = event.target as HTMLElement | null;
  const copyBtn = target?.closest('[data-copy-code]') as HTMLElement | null;
  if (!copyBtn) return;
  const encoded = copyBtn.getAttribute('data-copy-code') || '';
  const code = decodeUtf8Base64(encoded);
  if (!code) return;
  await copyText(code);
};

const copyText = async (text: string) => {
  if (!text.trim()) return;
  try {
    await navigator.clipboard.writeText(text);
    messages.value.push({ role: 'system', content: 'Copied response to clipboard.' });
  } catch {
    messages.value.push({ role: 'system', content: 'Copy failed: clipboard unavailable.' });
  }
};

const isAssistantErrorContent = (content: string) => {
  const t = (content || '').trim();
  return (
    t.startsWith('Request failed:')
    || t.startsWith('Retry failed:')
    || t === 'Please set API Key in AI settings first.'
  );
};

const dismissChatMessage = (idx: number) => {
  if (idx < 0 || idx >= messages.value.length) return;
  messages.value.splice(idx, 1);
};

const retryFailedSend = async () => {
  if (!failedRequest.value || loading.value) return;
  loading.value = true;
  messages.value.push({ role: 'system', content: 'Retrying last request...' });
  try {
    const result = await requestChatWithRetry({
      model: failedRequest.value.model,
      messages: failedRequest.value.messages,
      temperature: 0.2,
      stream: false,
    });
    if (result?.switched && result?.model) {
      messages.value.push({
        role: 'system',
        content: `Model fallback applied: automatically switched to ${result.model}.`,
      });
      if (Array.isArray(result?.fallbackFailures) && result.fallbackFailures.length) {
        messages.value.push({
          role: 'system',
          content: `Fallback failures: ${result.fallbackFailures.join(' | ')}`,
        });
      }
    }
    if (result?.model && result.model !== model.value) {
      model.value = result.model;
    }
    messages.value.push({ role: 'assistant', content: String(result?.content || 'No response.') });
    failedRequest.value = null;
  } catch (error) {
    messages.value.push({ role: 'assistant', content: `Retry failed: ${String(error)}` });
  } finally {
    loading.value = false;
  }
};

const makeRequestId = () => {
  if (typeof crypto !== 'undefined' && typeof crypto.randomUUID === 'function') {
    return crypto.randomUUID();
  }
  return `req-${Date.now()}-${Math.random().toString(36).slice(2, 8)}`;
};

const send = async () => {
  const text = input.value.trim();
  const hasImage = !!pendingImageDataUrl.value;
  const hasCodeRef = pendingCodeRefs.value.length > 0;
  if ((!text && !hasImage && !hasCodeRef) || loading.value) return;
  if (!apiKey.value.trim()) {
    messages.value.push({ role: 'assistant', content: 'Please set API Key in AI settings first.' });
    showSettings.value = true;
    return;
  }

  const context = getEditorContext();
  const baseText = text || (hasImage ? 'Please analyze the attached image.' : 'Please analyze the attached code.');
  const codeRefText = buildCodeRefContext();
  const composed = [baseText, codeRefText, context].filter((part) => !!part).join('\n\n---\n\n');
  const userContent = composed;
  messages.value.push({
    role: 'user',
    content: text || (hasImage ? '[Image]' : getCodeRefLabel()),
    imageUrl: pendingImageDataUrl.value || undefined,
  });
  const payloadMessages = messages.value
    .filter((m) => m.role !== 'system')
    .map((m) => {
      if (m.role === 'user' && m.imageUrl) {
        return {
          role: 'user',
          content: [
            { type: 'text', text: m.content || 'Please analyze the attached image.' },
            { type: 'image_url', image_url: { url: m.imageUrl } },
          ],
        };
      }
      return { role: m.role, content: m.content };
    });
  payloadMessages[payloadMessages.length - 1] = hasImage
    ? {
        role: 'user',
        content: [
          { type: 'text', text: userContent },
          { type: 'image_url', image_url: { url: pendingImageDataUrl.value } },
        ],
      }
    : { role: 'user', content: userContent };

  input.value = '';
  loading.value = true;
  messages.value.push({ role: 'assistant', content: '' });
  const assistantMsg = messages.value[messages.value.length - 1] as ChatMessage;
  streamingAssistantMessage.value = assistantMsg;
  const requestId = makeRequestId();
  let stopListen: UnlistenFn | null = null;

  try {
    stopListen = await listen<ChatStreamChunkEvent>('aihub_chat_chunk', (event) => {
      const data = event.payload;
      if (!data || data.requestId !== requestId) return;
      assistantMsg.content += String(data.chunk || '');
    });
    const result = await invoke<ChatResult>('aihub_chat_stream', {
      requestId,
      payload: {
        model: model.value,
        messages: payloadMessages,
        temperature: 0.2,
        stream: true,
      },
    });
    if (assistantMsg.content.trim() !== String(result?.content || '').trim()) {
      assistantMsg.content = String(result?.content || assistantMsg.content || 'No response.');
    } else if (!assistantMsg.content.trim()) {
      assistantMsg.content = 'No response.';
    }
    failedRequest.value = null;
    if (result?.switched && result?.model) {
      messages.value.push({
        role: 'system',
        content: `Model fallback applied: automatically switched to ${result.model}.`,
      });
      if (Array.isArray(result?.fallbackFailures) && result.fallbackFailures.length) {
        messages.value.push({
          role: 'system',
          content: `Fallback failures: ${result.fallbackFailures.join(' | ')}`,
        });
      }
    }
    if (result?.model && result.model !== model.value) {
      model.value = result.model;
    }
  } catch (error) {
    if (!assistantMsg.content.trim()) {
      const idx = messages.value.lastIndexOf(assistantMsg);
      if (idx >= 0) messages.value.splice(idx, 1);
    }
    failedRequest.value = {
      model: model.value,
      messages: payloadMessages,
    };
    messages.value.push({ role: 'assistant', content: `Request failed: ${String(error)}` });
  } finally {
    if (stopListen) stopListen();
    streamingAssistantMessage.value = null;
    loading.value = false;
    // Persist final assistant output after stream completion.
    scheduleSaveAiChatState();
    clearPendingImage();
    clearPendingCodeRefs();
  }
};

const onInputKeydown = (event: KeyboardEvent) => {
  if (event.isComposing) return;
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    void send();
    return;
  }
  if ((event.ctrlKey || event.metaKey) && event.key === 'Enter') {
    event.preventDefault();
    void send();
  }
};

watch(modelPicker, async (value) => {
  if (!value) return;
  await nextTick();
  modelSearchInputRef.value?.focus();
  modelSearchInputRef.value?.select();
});
</script>

<template>
  <aside class="tool-window" :class="[position, { open }]">
    <div class="tool-window-header">
      <span class="title">{{ title }}</span>
      <div class="actions">
        <button
          v-if="title === 'AI Assistant'"
          class="action-btn history-btn"
          title="Chat History"
          @click.stop="toggleSessionMenu"
        >
          <FontAwesomeIcon :icon="['far', 'clock']" />
        </button>
        <div v-if="title === 'AI Assistant' && showSessionMenu" class="chat-session-picker history-menu-panel">
          <div class="session-menu ide-scrollbar">
          <div class="session-search-wrap">
            <input
              ref="sessionSearchInputRef"
              v-model="sessionSearch"
              class="session-search-input"
              type="text"
              placeholder="Search chats..."
              @keydown="onSessionSearchKeydown"
            >
          </div>
          <div v-if="pinnedSessions.length" class="session-group-label">Pinned</div>
          <button
            v-for="session in pinnedSessions"
            :key="session.id"
            class="session-item"
            :class="{ active: activeSessionId === session.id, highlighted: highlightedSessionId === session.id }"
            :title="session.title"
            @click="switchSession(session.id)"
            @mouseenter="highlightedSessionId = session.id"
          >
            <span class="session-item-title" v-html="renderSessionTitle(session.title)" />
            <span class="session-item-actions">
              <span class="session-item-time">{{ new Date(session.updatedAt).toLocaleString() }}</span>
              <span class="session-item-btns">
                <button class="session-mini-btn" title="Move up" @click.stop="movePinnedSession(session.id, -1)">
                  <FontAwesomeIcon :icon="['fas', 'chevron-up']" />
                </button>
                <button class="session-mini-btn" title="Move down" @click.stop="movePinnedSession(session.id, 1)">
                  <FontAwesomeIcon :icon="['fas', 'chevron-down']" />
                </button>
                <button class="session-mini-btn" title="Unpin" @click.stop="togglePinSession(session.id)">
                  <FontAwesomeIcon :icon="['fas', 'tags']" />
                </button>
                <button class="session-mini-btn" title="Rename" @click.stop="renameSession(session.id)">
                  <FontAwesomeIcon :icon="['fas', 'pen']" />
                </button>
                <button class="session-mini-btn danger" title="Delete" @click.stop="deleteSession(session.id)">
                  <FontAwesomeIcon :icon="['fas', 'trash']" />
                </button>
              </span>
            </span>
          </button>
          <div v-if="recentSessions.length" class="session-group-label">Recent</div>
          <button
            v-for="session in recentSessions"
            :key="session.id"
            class="session-item"
            :class="{ active: activeSessionId === session.id, highlighted: highlightedSessionId === session.id }"
            :title="session.title"
            @click="switchSession(session.id)"
            @mouseenter="highlightedSessionId = session.id"
          >
            <span class="session-item-title" v-html="renderSessionTitle(session.title)" />
            <span class="session-item-actions">
              <span class="session-item-time">{{ new Date(session.updatedAt).toLocaleString() }}</span>
              <span class="session-item-btns">
                <button class="session-mini-btn" title="Pin" @click.stop="togglePinSession(session.id)">
                  <FontAwesomeIcon :icon="['fas', 'tags']" />
                </button>
                <button class="session-mini-btn" title="Rename" @click.stop="renameSession(session.id)">
                  <FontAwesomeIcon :icon="['fas', 'pen']" />
                </button>
                <button class="session-mini-btn danger" title="Delete" @click.stop="deleteSession(session.id)">
                  <FontAwesomeIcon :icon="['fas', 'trash']" />
                </button>
              </span>
            </span>
          </button>
          <div v-if="!flattenedVisibleSessions.length" class="session-empty">No matching chats</div>
          </div>
        </div>
        <button
          v-if="title === 'AI Assistant'"
          class="action-btn header-more"
          title="More"
          @click.stop="toggleMoreMenu"
        >
          <FontAwesomeIcon :icon="['fas', 'ellipsis-vertical']" />
        </button>
        <div v-if="title === 'AI Assistant' && showMoreMenu" class="more-menu ide-scrollbar">
          <button class="more-item" @click="newChat">
            <FontAwesomeIcon :icon="['fas', 'comment-dots']" />
            <span class="more-item-label">New Chat</span>
          </button>
          <button class="more-item" @click="exportChatSessions">
            <FontAwesomeIcon :icon="['fas', 'cloud']" />
            <span class="more-item-label">Export Chats</span>
          </button>
          <button class="more-item" @click="openImportSessionsPicker">
            <FontAwesomeIcon :icon="['fas', 'rotate-right']" />
            <span class="more-item-label">Import Chats</span>
          </button>
          <button class="more-item" @click="pruneOldSessions(20)">
            <FontAwesomeIcon :icon="['fas', 'broom']" />
            <span class="more-item-label">Keep 20 Chats</span>
          </button>
          <button class="more-item" @click="cleanupEmptySessions">
            <FontAwesomeIcon :icon="['fas', 'eye-slash']" />
            <span class="more-item-label">Clean Empty Chats</span>
          </button>
          <button class="more-item" @click="clearCurrentChatScreen">
            <FontAwesomeIcon :icon="['fas', 'broom']" />
            <span class="more-item-label">Clear Current Chat</span>
          </button>
          <button class="more-item danger" @click="resetAllSessions">
            <FontAwesomeIcon :icon="['fas', 'trash']" />
            <span class="more-item-label">Reset All Chats</span>
          </button>
          <input
            ref="importSessionsInputRef"
            class="more-hidden-file"
            type="file"
            accept=".json,application/json"
            @change="onImportSessionsFile"
          >
        </div>
        <button class="action-btn" title="Settings" @click="showSettings = !showSettings">
          <FontAwesomeIcon :icon="['fas', 'gear']" />
        </button>
        <button class="action-btn close drawer-close-btn" @click="emit('close')" title="Hide">
          <span class="drawer-hide-icon" :class="position">
            <span class="drawer-body"></span>
            <span class="drawer-rail"></span>
            <span class="drawer-arrow"></span>
          </span>
        </button>
      </div>
    </div>

    <div class="tool-window-content ide-scrollbar">
      <slot>
        <div v-if="title === 'AI Assistant'" class="ai-placeholder">
          <div v-if="showSettings" class="ai-settings">
            <label>
              <span>Base URL</span>
              <input v-model="apiBaseUrl" type="text" placeholder="https://aihubmix.com/v1">
            </label>
            <label>
              <span>Model</span>
              <div class="model-row">
                <div class="model-picker">
                  <button
                    class="model-select-btn"
                    :title="model"
                    @click.stop="toggleModelPicker('settings', $event)"
                  >
                    <span class="model-select-text">{{ model }}</span>
                    <span v-if="isFreeModel(model)" class="free-badge compact">FR</span>
                    <span class="modality-badge compact" :class="supportsImageModel(model) ? 'image' : 'text'">
                      {{ supportsImageModel(model) ? 'IM' : 'TX' }}
                    </span>
                    <span class="model-select-caret">▾</span>
                  </button>
                  <div
                    v-if="modelPicker === 'settings'"
                    class="model-select-menu ide-scrollbar"
                    :class="modelPickerPlacement.settings"
                  >
                    <div class="model-search-wrap">
                      <input
                        ref="modelSearchInputRef"
                        v-model="modelSearch"
                        class="model-search-input"
                        type="text"
                        placeholder="Search model..."
                      >
                    </div>
                    <button
                      v-for="m in filteredModelOptions"
                      :key="`settings-${m.id}`"
                      class="model-option"
                      :class="{ active: model === m.id }"
                      :title="m.id"
                      @click="selectModel(m.id)"
                    >
                      <span class="model-option-text">{{ m.id }}</span>
                      <span class="model-option-badges">
                        <span v-if="m.isFree" class="free-badge">FR</span>
                        <span class="modality-badge" :class="m.supportsImage ? 'image' : 'text'">
                          {{ m.supportsImage ? 'IM' : 'TX' }}
                        </span>
                      </span>
                    </button>
                    <div v-if="!filteredModelOptions.length" class="model-empty">No matching model</div>
                  </div>
                </div>
                <button class="mini-btn" :disabled="modelLoading" @click="loadModelOptions">
                  {{ modelLoading ? 'Loading...' : 'Refresh' }}
                </button>
              </div>
              <small v-if="modelLoadError" class="hint error">{{ modelLoadError }}</small>
            </label>
            <label>
              <span>API Key</span>
              <input v-model="apiKey" type="password" placeholder="sk-...">
            </label>
            <label class="check">
              <input v-model="autoContext" type="checkbox">
              <span>Attach current file context automatically</span>
            </label>
          </div>

          <div class="quick-actions">
            <button class="quick-btn" @click="askQuick('Explain current file')">Explain</button>
            <button class="quick-btn" @click="askQuick('Find potential bugs in current file')">Find Bugs</button>
            <button class="quick-btn" @click="askQuick('Suggest a refactor plan for current file')">Refactor</button>
          </div>

          <div ref="chatScrollRef" class="chat-history ide-scrollbar">
            <div
              v-for="(message, idx) in messages"
              :key="`${idx}-${message.role}`"
              class="message"
              :class="[
                message.role,
                {
                  'assistant-error':
                    message.role === 'assistant'
                    && message.content
                    && isAssistantErrorContent(message.content),
                },
              ]"
            >
              <button
                v-if="
                  message.content.trim()
                    && (
                      (message.role === 'assistant' && isAssistantErrorContent(message.content))
                      || message.role === 'system'
                    )
                "
                class="msg-dismiss-btn"
                type="button"
                title="Dismiss"
                @click.stop="dismissChatMessage(idx)"
              >
                <FontAwesomeIcon :icon="['fas', 'xmark']" />
              </button>
              <button
                v-if="message.role === 'assistant' && message.content"
                class="msg-copy-btn"
                title="Copy response"
                @click="copyText(message.content)"
              >
                <FontAwesomeIcon :icon="['far', 'clone']" />
              </button>
              <div
                v-if="message.role === 'assistant' && message.content"
                class="markdown-body"
                @click="onMarkdownClick"
                v-html="renderAssistantContent(message.content)"
              />
              <div v-else-if="message.content" class="message-content-plain">{{ message.content }}</div>
              <span
                v-if="streamingAssistantMessage === message"
                class="stream-caret"
                aria-hidden="true"
              />
              <span
                v-if="streamingAssistantMessage === message && !message.content"
                class="stream-wait-text"
                aria-hidden="true"
              >
                <span class="stream-wait-text-inner">Thinking...</span>
              </span>
              <img v-if="message.imageUrl" class="msg-image" :src="message.imageUrl" alt="uploaded image">
            </div>
          </div>
          <div class="chat-input-wrapper">
            <input
              ref="fileInputRef"
              class="hidden-file"
              type="file"
              accept="image/*"
              @change="onPickImage"
            >
            <div v-if="pendingImageDataUrl" class="pending-image">
              <img :src="pendingImageDataUrl" :alt="pendingImageName || 'image'">
              <button class="mini-btn danger" @click="clearPendingImage">Remove</button>
            </div>
            <div v-if="pendingCodeRefs.length" class="pending-code">
              <div class="code-chip-list">
                <span v-for="item in pendingCodeRefs" :key="item.id" class="code-chip">
                  <button class="code-chip-main" :title="`Preview ${item.fileName}:${item.startLine}-${item.endLine}`" @click="togglePreviewCodeRef(item.id)">
                    <FontAwesomeIcon :icon="['fas', 'file']" />
                    <span class="code-chip-label">{{ item.fileName }}:{{ item.startLine }}-{{ item.endLine }}</span>
                  </button>
                  <button class="code-chip-remove" title="Remove code reference" @click="removePendingCodeRef(item.id)">
                    <FontAwesomeIcon :icon="['fas', 'xmark']" />
                  </button>
                </span>
              </div>
              <button class="mini-btn danger clear-all-btn" @click="clearPendingCodeRefs">Clear</button>
            </div>
            <div v-if="previewCodeRefId" class="code-preview">
              <pre>{{ pendingCodeRefs.find((item) => item.id === previewCodeRefId)?.snippet || '' }}</pre>
            </div>
            <textarea
              v-model="input"
              placeholder="Ask AI... (Ctrl/Cmd + Enter to send)"
              rows="3"
              @keydown="onInputKeydown"
              @paste="onInputPaste"
            />
            <div class="input-actions">
              <div class="model-picker model-inline-select">
                <button
                  class="model-select-btn"
                  :title="model"
                  @click.stop="toggleModelPicker('inline', $event)"
                >
                  <span class="model-select-text">{{ model }}</span>
                  <span v-if="isFreeModel(model)" class="free-badge compact">FR</span>
                  <span class="modality-badge compact" :class="supportsImageModel(model) ? 'image' : 'text'">
                    {{ supportsImageModel(model) ? 'IM' : 'TX' }}
                  </span>
                  <span class="model-select-caret">▾</span>
                </button>
                <div
                  v-if="modelPicker === 'inline'"
                  class="model-select-menu ide-scrollbar"
                  :class="modelPickerPlacement.inline"
                >
                  <div class="model-search-wrap">
                    <input
                      ref="modelSearchInputRef"
                      v-model="modelSearch"
                      class="model-search-input"
                      type="text"
                      placeholder="Search model..."
                    >
                  </div>
                  <button
                    v-for="m in filteredModelOptions"
                    :key="`inline-${m.id}`"
                    class="model-option"
                    :class="{ active: model === m.id }"
                    :title="m.id"
                    @click="selectModel(m.id)"
                  >
                    <span class="model-option-text">{{ m.id }}</span>
                    <span class="model-option-badges">
                      <span v-if="m.isFree" class="free-badge">FR</span>
                      <span class="modality-badge" :class="m.supportsImage ? 'image' : 'text'">
                        {{ m.supportsImage ? 'IM' : 'TX' }}
                      </span>
                    </span>
                  </button>
                  <div v-if="!filteredModelOptions.length" class="model-empty">No matching model</div>
                </div>
              </div>
              <div class="right-actions">
                <button
                  v-if="failedRequest && !loading"
                  class="mini-btn"
                  title="Retry last failed request"
                  @click="retryFailedSend"
                >
                  Retry
                </button>
                <button v-if="modelSupportsImage" class="mini-btn image-btn" title="Upload Image" @click="openImagePicker">
                  <FontAwesomeIcon :icon="['fas', 'image']" />
                </button>
                <button class="send-btn" :disabled="loading || (!input.trim() && !pendingImageDataUrl && !pendingCodeRefs.length)" @click="send">
                  {{ loading ? 'Thinking...' : 'Send' }}
                </button>
              </div>
            </div>
          </div>
        </div>

        <div v-else-if="title === 'Notifications'" class="notification-list">
          <div class="note-item info">Deno Language Server started.</div>
          <div class="note-item warning">Unused import in main.rs</div>
        </div>
      </slot>
    </div>
  </aside>
</template>

<style lang="scss" scoped>
@keyframes stream-caret-blink {
  0%, 46% { opacity: 0.96; }
  47%, 100% { opacity: 0; }
}

@keyframes stream-text-shimmer {
  0% { background-position: -120% 0; }
  100% { background-position: 220% 0; }
}

.tool-window {
  width: var(--ide-toolwindow-width);
  display: flex;
  flex-direction: column;
  background-color: var(--ide-bg-main);
  height: 100%;
  /* position + inset: .ide-body > .tool-window in layout.scss (absolute overlay) */
  box-sizing: border-box;
  z-index: 25;
  opacity: 0;
  transform: translateX(8px);
  transition: opacity 0.16s ease, transform 0.16s ease;
  pointer-events: none;

  &.left { border-right: 1px solid var(--ide-border); }
  &.right {
    border-left: 1px solid var(--ide-border);
    box-shadow: -6px 0 16px rgba(0, 0, 0, 0.2);
  }

  &.open {
    opacity: 1;
    transform: translateX(0);
    pointer-events: auto;
  }

  .tool-window-header {
    height: 32px;
    padding: 0 10px;
    display: flex;
    align-items: center;
    justify-content: space-between;
    background-color: var(--ide-bg-main);
    border-bottom: 1px solid var(--ide-border);

    .title {
      font-size: 11px;
      font-weight: 600;
      text-transform: uppercase;
      color: var(--ide-text-muted);
    }
    .actions {
      position: relative;
      display: inline-flex;
      align-items: center;
      gap: 2px;
    }
    .chat-session-picker {
      position: relative;
      min-width: 0;
      max-width: calc(100% - 140px);
      .session-switch-btn {
        height: 24px;
        border: 1px solid var(--ide-border);
        border-radius: 6px;
        background: color-mix(in srgb, var(--ide-bg-main) 86%, transparent);
        color: var(--ide-text-muted);
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 0 8px;
        cursor: pointer;
        max-width: 100%;
      }
      .session-switch-btn:hover {
        color: var(--ide-text);
        background: var(--ide-hover);
      }
      .session-switch-text {
        min-width: 0;
        max-width: 210px;
        overflow: hidden;
        text-overflow: ellipsis;
        white-space: nowrap;
        font-size: 12px;
      }
      .session-menu {
        position: absolute;
        top: calc(100% + 6px);
        left: 0;
        width: 286px;
        max-height: 280px;
        overflow: auto;
        border: 1px solid color-mix(in srgb, var(--ide-border) 90%, rgba(255, 255, 255, 0.04));
        border-radius: 10px;
        background: color-mix(in srgb, var(--ide-bg-elevated) 97%, var(--ide-bg-main));
        box-shadow: 0 16px 34px rgba(0, 0, 0, 0.44);
        backdrop-filter: blur(4px);
        z-index: 120;
        padding: 8px;
        isolation: isolate;
      }
      .session-search-wrap {
        padding: 2px 2px 6px;
      }
      .session-group-label {
        font-size: 10px;
        color: var(--ide-text-muted);
        text-transform: uppercase;
        letter-spacing: 0.4px;
        padding: 6px 8px 4px;
      }
      .session-search-input {
        width: 100%;
        height: 26px;
        border: 1px solid var(--ide-border);
        border-radius: 6px;
        background: color-mix(in srgb, var(--ide-bg-main) 86%, transparent);
        color: var(--ide-text);
        padding: 0 8px;
        font-size: 12px;
        box-sizing: border-box;
        font-family:
          Inter,
          "SF Pro Text",
          "Segoe UI",
          Roboto,
          "Helvetica Neue",
          Arial,
          "Apple Color Emoji",
          "Segoe UI Emoji",
          "Segoe UI Symbol",
          "Noto Color Emoji",
          "Noto Emoji",
          sans-serif;
      }
      .session-search-input:focus {
        outline: none;
        border-color: color-mix(in srgb, var(--ide-accent) 55%, var(--ide-border));
      }
      .session-item {
        width: 100%;
        border: 1px solid transparent;
        border-radius: 6px;
        background: transparent;
        color: var(--ide-text-muted);
        cursor: pointer;
        padding: 6px 8px;
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 10px;
        margin-bottom: 1px;
      }
      .session-item:hover {
        background: var(--ide-hover);
        color: var(--ide-text);
      }
      .session-item.active {
        border-color: color-mix(in srgb, var(--ide-accent) 46%, var(--ide-border));
        background: color-mix(in srgb, var(--ide-accent) 14%, transparent);
        color: var(--ide-text);
      }
      .session-item.highlighted {
        border-color: color-mix(in srgb, var(--ide-accent) 30%, var(--ide-border));
        background: color-mix(in srgb, var(--ide-hover) 76%, transparent);
        color: var(--ide-text);
      }
      .session-item-title {
        min-width: 0;
        flex: 1;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
        text-align: left;
        font-size: 11px;
        color: inherit;
        line-height: 1.35;
        font-family:
          Inter,
          "SF Pro Text",
          "Segoe UI",
          Roboto,
          "Helvetica Neue",
          Arial,
          "Apple Color Emoji",
          "Segoe UI Emoji",
          "Segoe UI Symbol",
          "Noto Color Emoji",
          "Noto Emoji",
          sans-serif;
        font-variant-emoji: emoji;
        :deep(.twemoji) {
          width: 0.95em;
          height: 0.95em;
          vertical-align: -0.14em;
          margin: 0 0.01em;
          object-fit: contain;
        }
      }
      .session-item-time {
        font-size: 9px;
        opacity: 0.78;
        white-space: nowrap;
      }
      .session-item-actions {
        display: inline-flex;
        align-items: center;
        gap: 5px;
      }
      .session-item-btns {
        display: inline-flex;
        align-items: center;
        gap: 4px;
        opacity: 0;
        transition: opacity 0.12s ease;
      }
      .session-item:hover .session-item-btns,
      .session-item.active .session-item-btns {
        opacity: 1;
      }
      .session-mini-btn {
        width: 17px;
        height: 17px;
        border-radius: 4px;
        border: 1px solid color-mix(in srgb, var(--ide-border) 88%, transparent);
        background: color-mix(in srgb, var(--ide-bg-main) 90%, transparent);
        color: color-mix(in srgb, var(--ide-text-muted) 92%, transparent);
        display: inline-flex;
        align-items: center;
        justify-content: center;
        cursor: pointer;
        padding: 0;
        transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease, transform 0.08s ease;
        :deep(svg) {
          width: 9px;
          height: 9px;
          display: block;
        }
      }
      .session-mini-btn:hover {
        color: var(--ide-text);
        background: color-mix(in srgb, var(--ide-hover) 88%, transparent);
        border-color: color-mix(in srgb, var(--ide-text-muted) 34%, var(--ide-border));
        transform: translateY(-0.5px);
      }
      .session-mini-btn.danger:hover {
        color: #ef8a8a;
        border-color: #9f3b3b;
        background: color-mix(in srgb, #9f3b3b 16%, var(--ide-bg-main));
      }
      .session-empty {
        font-size: 10px;
        color: var(--ide-text-muted);
        padding: 8px 8px;
      }
    }
    .history-menu-panel {
      z-index: 10;
      position: absolute;
      top: calc(100% + 6px);
      right: 0;
      max-width: none;
      min-width: 0;
      .session-menu {
        position: static;
        margin: 0;
      }
    }

    .action-btn {
      background: none;
      border: none;
      color: var(--ide-text-muted);
      cursor: pointer;
      padding: 2px 6px;
      font-size: 12px;
      &:hover { color: var(--ide-text); background: var(--ide-hover); }
    }
    .header-more {
      position: relative;
    }
    .more-menu {
      position: absolute;
      top: calc(100% + 6px);
      right: 0;
      width: 204px;
      max-height: 280px;
      overflow: auto;
      border: 1px solid var(--ide-border);
      border-radius: 8px;
      background: color-mix(in srgb, var(--ide-bg-elevated) 92%, var(--ide-bg-main));
      box-shadow: 0 10px 26px rgba(0, 0, 0, 0.36);
      z-index: 121;
      padding: 6px;
      display: flex;
      flex-direction: column;
      gap: 4px;
      .more-item {
        width: 100%;
        height: 28px;
        border: 1px solid transparent;
        border-radius: 6px;
        background: transparent;
        color: var(--ide-text);
        display: inline-flex;
        align-items: center;
        gap: 8px;
        padding: 0 9px;
        cursor: pointer;
        text-align: left;
        font-size: 12px;
        line-height: 1;
      }
      .more-item-label {
        font-size: 12px;
        font-weight: 500;
        color: inherit;
      }
      .more-item:hover {
        background: var(--ide-hover);
        color: var(--ide-text);
      }
      .more-item.danger {
        color: #e28a8a;
      }
      .more-item.danger:hover {
        background: color-mix(in srgb, #9f3b3b 24%, var(--ide-bg-main));
        color: #ffb3b3;
      }
      .more-hidden-file {
        display: none;
      }
    }
    .history-btn {
      width: 22px;
      height: 20px;
      padding: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      border-radius: 6px;
      border: 1px solid transparent;
      :deep(svg) {
        width: 11px;
        height: 11px;
      }
    }
    .history-btn:hover {
      border-color: color-mix(in srgb, var(--ide-text-muted) 35%, var(--ide-border));
    }
    .drawer-close-btn {
      width: 24px;
      height: 22px;
      padding: 0;
      display: inline-flex;
      align-items: center;
      justify-content: center;
    }
    .drawer-hide-icon {
      position: relative;
      width: 15px;
      height: 13px;
      display: inline-flex;
      align-items: center;
      justify-content: center;
      .drawer-body {
        position: absolute;
        inset: 0;
        border: 1px solid color-mix(in srgb, var(--ide-text-muted) 72%, transparent);
        border-radius: 2px;
        background: transparent;
      }
      .drawer-rail {
        position: absolute;
        top: 2px;
        bottom: 2px;
        width: 1px;
        background: color-mix(in srgb, var(--ide-text-muted) 72%, transparent);
      }
      .drawer-arrow {
        position: absolute;
        top: 50%;
        width: 5px;
        height: 5px;
        margin-top: -2px;
        border-top: 1px solid color-mix(in srgb, var(--ide-text-muted) 84%, transparent);
        border-right: 1px solid color-mix(in srgb, var(--ide-text-muted) 84%, transparent);
      }
      &.right {
        .drawer-rail { left: 3px; }
        .drawer-arrow {
          right: 3px;
          transform: rotate(45deg);
        }
      }
      &.left {
        .drawer-rail { right: 3px; }
        .drawer-arrow {
          left: 3px;
          transform: rotate(-135deg);
        }
      }
    }
    .drawer-close-btn:hover .drawer-hide-icon {
      .drawer-body {
        border-color: color-mix(in srgb, var(--ide-text) 72%, transparent);
      }
      .drawer-rail,
      .drawer-arrow {
        background: color-mix(in srgb, var(--ide-text) 70%, transparent);
        border-color: color-mix(in srgb, var(--ide-text) 84%, transparent);
      }
    }
  }

  .tool-window-content {
    flex: 1;
    overflow-y: auto;
    padding: 12px;
  }
}

/* AI 对话样式示例 */
.ai-placeholder {
  display: flex;
  flex-direction: column;
  height: 100%;
  gap: 10px;

  .ai-toolbar {
    display: flex;
    gap: 8px;
    padding-bottom: 2px;
  }

  .mini-btn {
    height: 24px;
    padding: 0 10px;
    border-radius: 6px;
    background: var(--ide-bg-main);
    color: var(--ide-text-muted);
    border: 1px solid var(--ide-border);
    cursor: pointer;
    font-size: 11px;
    font-weight: 500;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease, transform 0.08s ease;
  }
  .mini-btn:hover {
    background: var(--ide-hover);
    color: var(--ide-text);
    border-color: color-mix(in srgb, var(--ide-text-muted) 40%, var(--ide-border));
  }
  .mini-btn:active {
    transform: translateY(0.5px);
  }
  .mini-btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .ai-settings {
    border: 1px solid var(--ide-border);
    border-radius: 10px;
    padding: 12px;
    display: grid;
    gap: 10px;
    background: color-mix(in srgb, var(--ide-bg-elevated) 86%, transparent);
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.03);
    label {
      display: grid;
      gap: 6px;
      min-width: 0;
      span {
        font-size: 11px;
        color: var(--ide-text-muted);
      }
      input:not([type='checkbox']) {
        width: 100%;
        min-width: 0;
        box-sizing: border-box;
        height: 28px;
        border: 1px solid var(--ide-border);
        border-radius: 5px;
        background: var(--ide-bg-editor);
        color: var(--ide-text);
        padding: 0 8px;
        transition: border-color 0.12s ease, box-shadow 0.12s ease;
      }
      input:not([type='checkbox']):focus {
        outline: none;
        border-color: color-mix(in srgb, var(--ide-accent) 60%, var(--ide-border));
        box-shadow: 0 0 0 2px color-mix(in srgb, var(--ide-accent) 24%, transparent);
      }
      &.check {
        display: flex;
        align-items: center;
        gap: 8px;
        input[type='checkbox'] {
          width: 14px;
          height: 14px;
          margin: 0;
          accent-color: var(--ide-accent);
          cursor: pointer;
          flex: 0 0 auto;
        }
        span {
          line-height: 1.2;
        }
      }
    }
    .model-row {
      display: flex;
      gap: 6px;
      min-width: 0;
      .model-picker {
        flex: 1;
        min-width: 0;
      }
      .mini-btn {
        flex: 0 0 auto;
        min-width: 74px;
      }
    }
    .hint {
      font-size: 11px;
      color: var(--ide-text-muted);
      &.error { color: #ef6b6b; }
    }
  }

  .quick-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    padding-bottom: 2px;
  }
  .quick-btn {
    height: 24px;
    padding: 0 8px;
    border-radius: 999px;
    border: 1px solid var(--ide-border);
    background: var(--ide-bg-main);
    color: var(--ide-text-muted);
    cursor: pointer;
    font-size: 11px;
    transition: background 0.12s ease, color 0.12s ease, border-color 0.12s ease;
  }
  .quick-btn:hover {
    background: var(--ide-hover);
    color: var(--ide-text);
  }

  .chat-history {
    flex: 1;
    min-height: 0;
    overflow: auto;
    font-size: 12px;
    color: var(--ide-text);
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding-right: 4px;
  }
  .chat-history,
  .chat-session-picker,
  .chat-input-wrapper textarea {
    font-family:
      Inter,
      "SF Pro Text",
      "Segoe UI",
      Roboto,
      "Helvetica Neue",
      Arial,
      "Apple Color Emoji",
      "Segoe UI Emoji",
      "Segoe UI Symbol",
      "Noto Color Emoji",
      "Noto Emoji",
      sans-serif;
  }
  .message {
    position: relative;
    min-width: 0;
    max-width: 100%;
    border-radius: 8px;
    padding: 7px 9px;
    line-height: 1.56;
    letter-spacing: 0.01em;
    &.assistant {
      background: color-mix(in srgb, var(--ide-bg-elevated) 68%, transparent);
    }
    &.assistant-error {
      border: 1px solid color-mix(in srgb, #ef6b6b 42%, var(--ide-border));
      background: color-mix(in srgb, #ef6b6b 11%, var(--ide-bg-elevated));
    }
    &.user {
      background: color-mix(in srgb, var(--ide-accent) 18%, transparent);
      align-self: flex-end;
    }
    &.system {
      background: color-mix(in srgb, #d09b2d 18%, transparent);
      padding-right: 28px;
    }
    .msg-image {
      margin-top: 6px;
      max-width: 100%;
      border-radius: 6px;
      border: 1px solid var(--ide-border);
    }
    .message-content-plain {
      white-space: pre-wrap;
      overflow-wrap: anywhere;
      word-break: break-word;
    }
    .stream-caret {
      display: inline-block;
      width: 2px;
      height: 1em;
      margin-left: 2px;
      vertical-align: -2px;
      background: color-mix(in srgb, var(--ide-text) 92%, transparent);
      border-radius: 1px;
      animation: stream-caret-blink 0.95s steps(1, end) infinite;
      opacity: 0.9;
    }
    .stream-wait-text {
      display: inline-flex;
      align-items: center;
      margin-left: 2px;
      font-size: 11px;
      color: var(--ide-text-muted);
      .stream-wait-text-inner {
        background-image: linear-gradient(
          100deg,
          color-mix(in srgb, var(--ide-text-muted) 65%, transparent) 20%,
          color-mix(in srgb, var(--ide-text) 90%, transparent) 50%,
          color-mix(in srgb, var(--ide-text-muted) 65%, transparent) 80%
        );
        background-size: 220% 100%;
        background-repeat: no-repeat;
        -webkit-background-clip: text;
        background-clip: text;
        color: transparent;
        animation: stream-text-shimmer 1.35s linear infinite;
      }
    }
    .markdown-body {
      display: block;
      width: 100%;
      max-width: 100%;
      min-width: 0;
      color: var(--ide-text);
      overflow-wrap: anywhere;
      word-break: break-word;
      :deep(*) {
        max-width: 100%;
        min-width: 0;
        box-sizing: border-box;
      }
      p {
        margin: 0 0 8px;
      }
      p:last-child {
        margin-bottom: 0;
      }
      h1, h2, h3, h4 {
        margin: 4px 0 8px;
        line-height: 1.3;
      }
      h1 { font-size: 15px; }
      h2 { font-size: 14px; }
      h3 { font-size: 13px; }
      h4 { font-size: 12px; }
      ul, ol {
        margin: 0 0 8px 18px;
        padding: 0;
      }
      li {
        margin: 2px 0;
      }
      blockquote {
        margin: 0 0 8px;
        padding: 4px 10px;
        border-left: 3px solid color-mix(in srgb, var(--ide-accent) 58%, var(--ide-border));
        color: var(--ide-text-muted);
        background: color-mix(in srgb, var(--ide-bg-main) 86%, transparent);
      }
      hr {
        border: 0;
        border-top: 1px solid var(--ide-border);
        margin: 8px 0;
      }
      a {
        color: #5ea9ff;
        text-decoration: none;
      }
      a:hover {
        text-decoration: underline;
      }
      code {
        font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, "Liberation Mono", "Courier New", monospace;
        font-size: 11px;
        padding: 1px 4px;
        border-radius: 4px;
        background: color-mix(in srgb, var(--ide-bg-main) 82%, transparent);
        border: 1px solid color-mix(in srgb, var(--ide-border) 88%, transparent);
      }
      pre {
        margin: 0 0 8px;
        padding: 8px 10px;
        border-radius: 0 0 8px 8px;
        border: 1px solid var(--ide-border);
        border-top: none;
        background: color-mix(in srgb, var(--ide-bg-main) 94%, transparent);
        overflow-x: hidden;
        overflow-y: auto;
        overflow-wrap: anywhere;
        word-break: break-word;
        max-width: 100%;
      }
      pre code {
        display: block;
        border: none;
        background: transparent;
        padding: 0;
        white-space: pre-wrap;
        overflow-wrap: anywhere;
        word-break: break-word;
        max-width: 100%;
      }
      .md-code-block {
        margin-bottom: 8px;
        max-width: 100%;
        overflow: hidden;
      }
      .md-code-head {
        height: 26px;
        padding: 0 8px;
        border: 1px solid var(--ide-border);
        border-radius: 8px 8px 0 0;
        background: color-mix(in srgb, var(--ide-bg-elevated) 70%, transparent);
        display: flex;
        align-items: center;
        justify-content: space-between;
      }
      .md-code-lang {
        font-size: 10px;
        color: var(--ide-text-muted);
        text-transform: lowercase;
      }
      .md-code-copy-btn {
        border: 1px solid var(--ide-border);
        background: color-mix(in srgb, var(--ide-bg-main) 80%, transparent);
        color: var(--ide-text-muted);
        border-radius: 6px;
        height: 18px;
        padding: 0 7px;
        font-size: 10px;
        cursor: pointer;
      }
      .md-code-copy-btn:hover {
        color: var(--ide-text);
        background: var(--ide-hover);
      }
      .md-table-wrap {
        margin: 0 0 8px;
        overflow: auto;
        border: 1px solid var(--ide-border);
        border-radius: 6px;
        max-width: 100%;
      }
      table {
        width: 100%;
        border-collapse: collapse;
        min-width: 0;
        table-layout: fixed;
      }
      th, td {
        border-bottom: 1px solid var(--ide-border);
        border-right: 1px solid var(--ide-border);
        padding: 6px 8px;
        text-align: left;
        vertical-align: top;
        overflow-wrap: anywhere;
        word-break: break-word;
      }
      th:last-child, td:last-child {
        border-right: none;
      }
      tbody tr:last-child td {
        border-bottom: none;
      }
      th {
        color: var(--ide-text);
        background: color-mix(in srgb, var(--ide-bg-elevated) 72%, transparent);
        font-weight: 600;
      }
      .task-list {
        list-style: none;
        margin-left: 0;
      }
      .task-list li label {
        display: inline-flex;
        align-items: center;
        gap: 6px;
      }
      .task-list input[type='checkbox'] {
        width: 13px;
        height: 13px;
        accent-color: var(--ide-accent);
      }
      :deep(.twemoji) {
        width: 1.05em;
        height: 1.05em;
        vertical-align: -0.14em;
        margin: 0 0.02em;
        object-fit: contain;
      }
      /* v-html content needs deep selectors in scoped styles */
      :deep(pre) {
        overflow-x: hidden !important;
        overflow-y: auto !important;
        white-space: pre-wrap !important;
        overflow-wrap: anywhere !important;
        word-break: break-word !important;
        max-width: 100% !important;
      }
      :deep(pre code) {
        white-space: pre-wrap !important;
        overflow-wrap: anywhere !important;
        word-break: break-word !important;
        max-width: 100% !important;
      }
      :deep(code) {
        overflow-wrap: anywhere;
        word-break: break-word;
      }
    }
    .msg-dismiss-btn {
      position: absolute;
      top: 6px;
      right: 6px;
      width: 18px;
      height: 18px;
      border: 1px solid color-mix(in srgb, #ef6b6b 35%, var(--ide-border));
      border-radius: 4px;
      background: color-mix(in srgb, var(--ide-bg-main) 84%, transparent);
      color: color-mix(in srgb, #ef6b6b 88%, var(--ide-text-muted));
      display: inline-flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      font-size: 10px;
      opacity: 0.9;
      transition: opacity 0.12s ease, color 0.12s ease, background 0.12s ease, border-color 0.12s ease;
    }
    &.system .msg-dismiss-btn {
      border-color: color-mix(in srgb, #d09b2d 42%, var(--ide-border));
      color: color-mix(in srgb, #d09b2d 95%, var(--ide-text-muted));
    }
    .msg-dismiss-btn:hover {
      color: var(--ide-text);
      background: var(--ide-hover);
      border-color: var(--ide-border);
      opacity: 1;
    }
    .msg-copy-btn {
      position: absolute;
      top: 6px;
      right: 6px;
      width: 18px;
      height: 18px;
      border: 1px solid var(--ide-border);
      border-radius: 4px;
      background: color-mix(in srgb, var(--ide-bg-main) 84%, transparent);
      color: var(--ide-text-muted);
      display: inline-flex;
      align-items: center;
      justify-content: center;
      cursor: pointer;
      opacity: 0;
      transition: opacity 0.12s ease, color 0.12s ease, background 0.12s ease;
    }
    &.assistant-error .msg-copy-btn {
      right: 28px;
    }
    &:hover .msg-copy-btn {
      opacity: 1;
    }
    .msg-copy-btn:hover {
      color: var(--ide-text);
      background: var(--ide-hover);
    }
  }

  .chat-input-wrapper {
    margin-top: 10px;
    background: var(--ide-bg-editor);
    border: 1px solid var(--ide-border);
    border-radius: 4px;
    padding: 8px;
    .hidden-file { display: none; }
    .pending-image {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 8px;
      img {
        max-height: 80px;
        max-width: 100%;
        border-radius: 6px;
        border: 1px solid var(--ide-border);
      }
      .mini-btn.danger {
        border-color: #9f3b3b;
        color: #ef8a8a;
      }
    }
    .pending-code {
      display: flex;
      align-items: flex-start;
      justify-content: space-between;
      gap: 8px;
      margin-bottom: 8px;
      .code-chip-list {
        flex: 1;
        min-width: 0;
        display: flex;
        align-items: center;
        gap: 6px;
        flex-wrap: wrap;
      }
      .code-chip {
        max-width: 100%;
        min-width: 0;
        display: inline-flex;
        align-items: center;
        gap: 6px;
        height: 22px;
        padding: 0 7px;
        border-radius: 8px;
        border: 1px solid color-mix(in srgb, var(--ide-accent) 40%, var(--ide-border));
        background: color-mix(in srgb, var(--ide-accent) 14%, transparent);
        color: var(--ide-text);
        font-size: 11px;
      }
      .code-chip-main {
        min-width: 0;
        max-width: 100%;
        border: none;
        background: transparent;
        color: inherit;
        display: inline-flex;
        align-items: center;
        gap: 6px;
        padding: 0;
        cursor: pointer;
      }
      .code-chip-label {
        min-width: 0;
        max-width: 240px;
        white-space: nowrap;
        overflow: hidden;
        text-overflow: ellipsis;
      }
      .code-chip-remove {
        width: 14px;
        height: 14px;
        border: none;
        background: transparent;
        color: var(--ide-text-muted);
        border-radius: 4px;
        display: inline-flex;
        align-items: center;
        justify-content: center;
        padding: 0;
        cursor: pointer;
      }
      .code-chip-remove:hover {
        color: var(--ide-text);
        background: color-mix(in srgb, var(--ide-hover) 82%, transparent);
      }
      .mini-btn.danger {
        border-color: #9f3b3b;
        color: #ef8a8a;
      }
      .clear-all-btn {
        height: 22px;
        padding: 0 8px;
      }
    }
    .code-preview {
      margin-bottom: 8px;
      border: 1px solid var(--ide-border);
      border-radius: 6px;
      background: color-mix(in srgb, var(--ide-bg-main) 90%, transparent);
      max-height: 180px;
      overflow: auto;
      pre {
        margin: 0;
        padding: 8px;
        font-size: 11px;
        line-height: 1.45;
        color: var(--ide-text);
        white-space: pre;
      }
    }

    textarea {
      width: 100%;
      background: transparent;
      border: none;
      color: var(--ide-text);
      resize: none;
      font-size: 12px;
      line-height: 1.5;
      letter-spacing: 0.01em;
      &::placeholder {
        color: color-mix(in srgb, var(--ide-text-muted) 86%, transparent);
      }
      &:focus { outline: none; }
    }

    .input-actions {
      display: flex;
      justify-content: space-between;
      align-items: center;
      margin-top: 5px;

      .model-inline-select {
        max-width: 52%;
        min-width: 160px;
      }
      .model-inline-select .model-select-btn {
        height: 24px;
      }
      .right-actions {
        display: flex;
        align-items: center;
        gap: 6px;
        .image-btn {
          width: 24px;
          height: 24px;
          padding: 0;
          display: inline-flex;
          align-items: center;
          justify-content: center;
          border-radius: 6px;
          background: color-mix(in srgb, var(--ide-bg-main) 82%, transparent);
          border-color: color-mix(in srgb, var(--ide-text-muted) 28%, var(--ide-border));
        }
        .image-btn:hover {
          border-color: color-mix(in srgb, var(--ide-accent) 45%, var(--ide-border));
          color: var(--ide-text);
          background: color-mix(in srgb, var(--ide-accent) 14%, transparent);
        }
      }
      .send-btn {
        background: var(--ide-accent);
        color: white;
        border: none;
        padding: 4px 10px;
        border-radius: 6px;
        font-size: 11px;
        font-weight: 600;
        transition: filter 0.12s ease, transform 0.08s ease;
      }
      .send-btn:hover:not(:disabled) {
        filter: brightness(1.08);
      }
      .send-btn:active:not(:disabled) {
        transform: translateY(0.5px);
      }
      .send-btn:disabled {
        opacity: 0.6;
        cursor: not-allowed;
      }
    }
  }
}

.model-picker {
  position: relative;
  min-width: 0;
}

.model-select-btn {
  width: 100%;
  min-width: 0;
  height: 28px;
  border: 1px solid var(--ide-border);
  border-radius: 5px;
  background: var(--ide-bg-editor);
  color: var(--ide-text);
  padding: 0 8px;
  font-size: 11px;
  font-weight: 500;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  cursor: pointer;
  transition: border-color 0.12s ease, background-color 0.12s ease, box-shadow 0.12s ease;
}
.model-select-btn:hover {
  background: var(--ide-hover);
}
.model-select-btn:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--ide-accent) 60%, var(--ide-border));
  box-shadow: 0 0 0 2px color-mix(in srgb, var(--ide-accent) 24%, transparent);
}
.model-select-text {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
  flex: 1;
}
.model-select-caret {
  color: var(--ide-text-muted);
  flex: 0 0 auto;
}

.model-select-menu {
  position: absolute;
  top: calc(100% + 4px);
  left: 0;
  right: 0;
  max-height: 220px;
  overflow: auto;
  border: 1px solid var(--ide-border);
  border-radius: 8px;
  background: var(--ide-bg-main);
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.35);
  z-index: 60;
  scrollbar-color: #5d6672 #242a33;
}
.model-select-menu.up {
  top: auto;
  bottom: calc(100% + 4px);
}
.model-select-menu::-webkit-scrollbar {
  width: 8px;
  height: 8px;
}
.model-select-menu::-webkit-scrollbar-thumb {
  background: #5d6672;
  border-radius: 8px;
}
.model-select-menu::-webkit-scrollbar-track {
  background: #242a33;
}

.model-option {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 8px;
  width: 100%;
  border: none;
  background: transparent;
  color: var(--ide-text);
  text-align: left;
  font-size: 12px;
  padding: 7px 10px;
  cursor: pointer;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.model-option-text {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
.model-option-badges {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  flex: 0 0 auto;
}
.model-option:hover {
  background: var(--ide-hover);
}
.model-option.active {
  background: color-mix(in srgb, var(--ide-accent) 22%, transparent);
}

.free-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 5px;
  height: 14px;
  border-radius: 999px;
  border: 1px solid color-mix(in srgb, #25c278 40%, var(--ide-border));
  background: color-mix(in srgb, #25c278 20%, transparent);
  color: #8ef0be;
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.1px;
  line-height: 1;
  flex: 0 0 auto;
}
.free-badge.compact {
  height: 12px;
  padding: 0 4px;
}

.modality-badge {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  padding: 0 5px;
  height: 14px;
  border-radius: 999px;
  font-size: 9px;
  font-weight: 700;
  letter-spacing: 0.1px;
  line-height: 1;
  flex: 0 0 auto;
  border: 1px solid transparent;
}
.modality-badge.image {
  border-color: color-mix(in srgb, #5ea2ff 45%, var(--ide-border));
  background: color-mix(in srgb, #5ea2ff 20%, transparent);
  color: #b8d7ff;
}
.modality-badge.text {
  border-color: color-mix(in srgb, #a7adb8 35%, var(--ide-border));
  background: color-mix(in srgb, #a7adb8 14%, transparent);
  color: #c6ccd6;
}
.modality-badge.compact {
  height: 12px;
  padding: 0 4px;
}

.model-search-wrap {
  position: sticky;
  top: 0;
  z-index: 1;
  padding: 6px;
  border-bottom: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-bg-main) 96%, transparent);
}
.model-search-input {
  width: 100%;
  height: 24px;
  box-sizing: border-box;
  border: 1px solid var(--ide-border);
  border-radius: 6px;
  background: var(--ide-bg-editor);
  color: var(--ide-text);
  padding: 0 8px;
  font-size: 11px;
}
.model-search-input:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--ide-accent) 60%, var(--ide-border));
}
.model-empty {
  padding: 10px;
  font-size: 11px;
  color: var(--ide-text-muted);
  text-align: center;
}

.note-item {
  font-size: 12px;
  padding: 8px;
  border-bottom: 1px solid var(--ide-border);
  &.warning { color: #e2b012; }
}
</style>