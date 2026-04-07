<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref } from 'vue';

const props = withDefaults(defineProps<{
  branch: string;
  filePath: string;
  encoding: string;
  lineEnding?: 'LF' | 'CRLF';
  line: number;
  col: number;
  indentLabel: string; // e.g. "Spaces: 2" / "Tab Size: 4"
  memoryText: string;  // e.g. "145 / 2048 MB" or "—"
  memoryPercent: number; // 0..100
  aiOnline: boolean;
  notificationsCount: number;
}>(), {
  lineEnding: 'LF',
});

const emit = defineEmits<{
  (e: 'change-encoding', encoding: string): void;
  (e: 'change-line-ending', value: 'LF' | 'CRLF'): void;
  (e: 'click-position'): void;
  (e: 'click-indent'): void;
  (e: 'toggle-ai'): void;
  (e: 'open-notifications'): void;
}>();

const encodingWrapRef = ref<HTMLElement | null>(null);
const lineEndingWrapRef = ref<HTMLElement | null>(null);
const encodingMenuOpen = ref(false);
const lineEndingMenuOpen = ref(false);
const showMoreEncodings = ref(false);

const commonEncodings = ['UTF-8', 'UTF-16 LE', 'UTF-16 BE', 'GBK', 'GB2312', 'BIG5'];
const moreEncodings = ['ISO-8859-1', 'Windows-1252', 'Shift_JIS', 'EUC-JP', 'EUC-KR', 'KOI8-R'];

const toggleEncodingMenu = () => {
  lineEndingMenuOpen.value = false;
  encodingMenuOpen.value = !encodingMenuOpen.value;
};

const toggleLineEndingMenu = () => {
  encodingMenuOpen.value = false;
  showMoreEncodings.value = false;
  lineEndingMenuOpen.value = !lineEndingMenuOpen.value;
};

const selectEncoding = (value: string) => {
  emit('change-encoding', value);
  encodingMenuOpen.value = false;
  showMoreEncodings.value = false;
};

const selectLineEnding = (value: 'LF' | 'CRLF') => {
  emit('change-line-ending', value);
  lineEndingMenuOpen.value = false;
};

const onDocPointerDown = (event: PointerEvent) => {
  if (!encodingMenuOpen.value) return;
  const root = encodingWrapRef.value;
  if (!root) return;
  const lineRoot = lineEndingWrapRef.value;
  const target = event.target;
  if (!(target instanceof Node)) return;
  if (!root.contains(target)) {
    encodingMenuOpen.value = false;
    showMoreEncodings.value = false;
  }
  if (lineRoot && !lineRoot.contains(target)) {
    lineEndingMenuOpen.value = false;
  }
};

onMounted(() => {
  document.addEventListener('pointerdown', onDocPointerDown, { capture: true });
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', onDocPointerDown, { capture: true } as any);
});
</script>

<template>
  <footer class="status-bar">
    <div class="section left">
      <div class="item branch">
        <FontAwesomeIcon :icon="['fas','code-branch']" />&nbsp;{{ branch }}
      </div>
      <div class="item path" :title="filePath">{{ filePath }}</div>
    </div>

    <div class="section right">
      <div ref="encodingWrapRef" class="item encoding-wrap">
        <button class="encoding-trigger" type="button" @click="toggleEncodingMenu">
          <span>{{ encoding }}</span>
        </button>
        <div v-if="encodingMenuOpen" class="encoding-menu">
          <button
            v-for="name in commonEncodings"
            :key="`c-${name}`"
            class="encoding-option"
            :class="{ active: props.encoding === name }"
            @click="selectEncoding(name)"
          >
            <span>{{ name }}</span>
            <span v-if="props.encoding === name">✓</span>
          </button>
          <button class="encoding-more" @click="showMoreEncodings = !showMoreEncodings">
            <FontAwesomeIcon :icon="['fas','ellipsis-vertical']" />
            <span>{{ showMoreEncodings ? 'Less' : 'More' }}</span>
          </button>
          <div v-if="showMoreEncodings" class="encoding-more-list">
            <button
              v-for="name in moreEncodings"
              :key="`m-${name}`"
              class="encoding-option"
              :class="{ active: props.encoding === name }"
              @click="selectEncoding(name)"
            >
              <span>{{ name }}</span>
              <span v-if="props.encoding === name">✓</span>
            </button>
          </div>
        </div>
      </div>
      <div ref="lineEndingWrapRef" class="item encoding-wrap">
        <button class="encoding-trigger" type="button" @click="toggleLineEndingMenu">
          <span>{{ lineEnding }}</span>
        </button>
        <div v-if="lineEndingMenuOpen" class="encoding-menu line-ending-menu">
          <button
            class="encoding-option"
            :class="{ active: props.lineEnding === 'LF' }"
            @click="selectLineEnding('LF')"
          >
            <span>LF</span>
            <span v-if="props.lineEnding === 'LF'">✓</span>
          </button>
          <button
            class="encoding-option"
            :class="{ active: props.lineEnding === 'CRLF' }"
            @click="selectLineEnding('CRLF')"
          >
            <span>CRLF</span>
            <span v-if="props.lineEnding === 'CRLF'">✓</span>
          </button>
        </div>
      </div>
      <div class="item clickable" @click="emit('click-position')">Line {{ line }}, Col {{ col }}</div>
      <div class="item clickable" @click="emit('click-indent')">{{ indentLabel }}</div>
      <div class="item memory" :title="memoryText">
        <span>{{ memoryText }}</span>
        <div class="bar"><div class="fill" :style="{ width: `${Math.max(0, Math.min(100, memoryPercent))}%` }" /></div>
      </div>
      <div class="item ai-status clickable" :class="aiOnline ? 'online' : 'offline'" @click="emit('toggle-ai')">
        <FontAwesomeIcon :icon="['fas','robot']" />&nbsp;AI {{ aiOnline ? 'Online' : 'Offline' }}
      </div>
      <div class="item notifications clickable" @click="emit('open-notifications')">
        <FontAwesomeIcon :icon="['fas','bell']" />
        <span v-if="notificationsCount > 0" class="badge">{{ notificationsCount }}</span>
      </div>
    </div>
  </footer>
</template>

<style lang="scss" scoped>
.status-bar {
  height: var(--ide-statusbar-height);
  background: var(--ide-bg-main);
  border-top: 1px solid var(--ide-border);
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0 10px;
  font-size: 11px;
  color: var(--ide-text-muted);

  .section { display: flex; align-items: center; gap: 12px; }
  .item { display: flex; align-items: center; cursor: default; }
  .clickable:hover { color: var(--ide-text); cursor: pointer; }
  .encoding-wrap {
    position: relative;
    margin-right: -2px;
  }
  .encoding-trigger {
    height: 22px;
    border: 1px solid transparent;
    background: transparent;
    color: inherit;
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    cursor: pointer;
    border-radius: 0;
    font-size: inherit;
  }
  .encoding-trigger:hover {
    color: var(--ide-text);
    background: var(--ide-hover);
    border-color: transparent;
  }
  .encoding-caret {
    font-size: 9px;
    opacity: 0.9;
  }
  .encoding-menu {
    position: absolute;
    right: -40px;
    bottom: calc(100% + 6px);
    width: 124px;
    border: 1px solid var(--ide-border);
    background: var(--ide-bg-main);
    z-index: 60;
    box-shadow: 0 10px 24px rgba(0, 0, 0, 0.35);
    padding: 4px;
  }
  .line-ending-menu {
    right: -22px;
    width: 72px;
  }
  .encoding-option {
    width: 100%;
    height: 26px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--ide-text);
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0 8px;
    cursor: pointer;
    font-size: 11px;
  }
  .encoding-option:hover {
    background: var(--ide-hover);
  }
  .encoding-option.active {
    background: color-mix(in srgb, var(--ide-accent) 24%, transparent);
  }
  .encoding-more {
    width: 100%;
    height: 24px;
    margin-top: 2px;
    border: 1px solid transparent;
    background: transparent;
    color: var(--ide-text-muted);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    cursor: pointer;
    font-size: 10px;
  }
  .encoding-more:hover {
    color: var(--ide-text);
    background: var(--ide-hover);
  }
  .encoding-more-list {
    margin-top: 2px;
    border-top: 1px solid var(--ide-border);
    padding-top: 4px;
  }
  .path {
    max-width: 420px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .memory {
    display: flex;
    align-items: center;
    gap: 6px;
    .bar {
      width: 40px; height: 4px; background: #43454a; border-radius: 2px;
      .fill { height: 100%; background: #62656c; border-radius: 2px; }
    }
  }

  .ai-status {
    &.online { color: #67d18f; }
    &.offline { color: #ef7f7f; }
  }
  .ai-status.clickable:hover {
    filter: brightness(1.08);
  }

  .notifications {
    position: relative;
    gap: 6px;
  }
  .badge {
    margin-left: 6px;
    font-size: 10px;
    line-height: 1;
    padding: 2px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--ide-accent) 35%, transparent);
    color: var(--ide-text);
  }
}
</style>