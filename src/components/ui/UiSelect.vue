<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';

export type UiSelectOption = {
  value: string;
  label: string;
  disabled?: boolean;
};

const props = withDefaults(defineProps<{
  modelValue: string;
  options: UiSelectOption[];
  disabled?: boolean;
  placeholder?: string;
}>(), {
  disabled: false,
  placeholder: '',
});

const emit = defineEmits<{
  (e: 'update:modelValue', value: string): void;
  (e: 'change', value: string): void;
}>();

const rootRef = ref<HTMLElement | null>(null);
const open = ref(false);
const activeIndex = ref(-1);

const selected = computed(() => props.options.find((o) => o.value === props.modelValue) ?? null);
const selectedLabel = computed(() => selected.value?.label ?? props.placeholder ?? '');

const enabledIndices = computed(() =>
  props.options
    .map((o, idx) => ({ o, idx }))
    .filter((x) => !x.o.disabled)
    .map((x) => x.idx),
);

const setOpen = async (value: boolean) => {
  if (props.disabled) return;
  open.value = value;
  if (open.value) {
    const idx = props.options.findIndex((o) => o.value === props.modelValue && !o.disabled);
    activeIndex.value = idx >= 0 ? idx : (enabledIndices.value[0] ?? -1);
    await nextTick();
  }
};

const toggle = async () => {
  await setOpen(!open.value);
};

const selectIndex = async (idx: number) => {
  const opt = props.options[idx];
  if (!opt || opt.disabled) return;
  emit('update:modelValue', opt.value);
  emit('change', opt.value);
  await setOpen(false);
};

const moveActive = (delta: number) => {
  const enabled = enabledIndices.value;
  if (!enabled.length) return;
  const currentPos = Math.max(0, enabled.indexOf(activeIndex.value));
  const nextPos = (currentPos + delta + enabled.length) % enabled.length;
  activeIndex.value = enabled[nextPos];
  const list = rootRef.value?.querySelector<HTMLElement>('.ui-select-list');
  const el = rootRef.value?.querySelector<HTMLElement>(`[data-idx="${activeIndex.value}"]`);
  if (list && el) {
    const top = el.offsetTop;
    const bottom = top + el.offsetHeight;
    if (top < list.scrollTop) list.scrollTop = top;
    else if (bottom > list.scrollTop + list.clientHeight) list.scrollTop = bottom - list.clientHeight;
  }
};

const onKeydown = async (e: KeyboardEvent) => {
  if (props.disabled) return;
  if (e.key === 'Enter' || e.key === ' ') {
    e.preventDefault();
    if (!open.value) {
      await setOpen(true);
      return;
    }
    if (activeIndex.value >= 0) await selectIndex(activeIndex.value);
    return;
  }
  if (e.key === 'Escape') {
    if (!open.value) return;
    e.preventDefault();
    await setOpen(false);
    return;
  }
  if (e.key === 'ArrowDown') {
    e.preventDefault();
    if (!open.value) await setOpen(true);
    moveActive(1);
    return;
  }
  if (e.key === 'ArrowUp') {
    e.preventDefault();
    if (!open.value) await setOpen(true);
    moveActive(-1);
    return;
  }
};

const onDocPointerDown = (e: PointerEvent) => {
  if (!open.value) return;
  const el = rootRef.value;
  if (!el) return;
  if (e.target instanceof Node && !el.contains(e.target)) {
    void setOpen(false);
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
  <div
    ref="rootRef"
    class="ui-select"
    :class="{ open, disabled: props.disabled }"
    tabindex="0"
    role="combobox"
    aria-haspopup="listbox"
    :aria-expanded="open ? 'true' : 'false'"
    @keydown="onKeydown"
  >
    <button class="ui-select-trigger" type="button" :disabled="props.disabled" @click="toggle">
      <span class="ui-select-value" :class="{ placeholder: !selected }">
        {{ selectedLabel }}
      </span>
      <span class="ui-select-caret" aria-hidden="true">
        <FontAwesomeIcon :icon="['fas', 'chevron-down']" />
      </span>
    </button>

    <div v-if="open" class="ui-select-popover" role="listbox">
      <div class="ui-select-list ide-scrollbar">
        <button
          v-for="(opt, idx) in props.options"
          :key="opt.value"
          class="ui-select-option"
          :class="{ selected: opt.value === props.modelValue, active: idx === activeIndex, disabled: !!opt.disabled }"
          type="button"
          :disabled="!!opt.disabled"
          :data-idx="idx"
          @click="selectIndex(idx)"
          @mouseenter="activeIndex = opt.disabled ? activeIndex : idx"
        >
          <span class="ui-select-option-label">{{ opt.label }}</span>
          <span v-if="opt.value === props.modelValue" class="ui-select-check" aria-hidden="true">✓</span>
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.ui-select {
  position: relative;
  width: 100%;
  outline: none;
}

.ui-select-trigger {
  height: 32px;
  width: 100%;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
  border: 1px solid color-mix(in srgb, var(--ide-border) 92%, transparent);
  background: color-mix(in srgb, var(--ide-bg-elevated) 88%, transparent);
  color: var(--ide-text);
  border-radius: 0;
  padding: 0 10px;
  font-size: 0.95em;
  cursor: pointer;
}

.ui-select:not(.disabled):hover .ui-select-trigger {
  border-color: color-mix(in srgb, var(--ide-border) 70%, var(--ide-text-muted));
}

.ui-select:focus-visible .ui-select-trigger,
.ui-select.open .ui-select-trigger {
  border-color: color-mix(in srgb, var(--ide-accent) 62%, var(--ide-border));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--ide-accent) 55%, transparent);
}

.ui-select.disabled .ui-select-trigger {
  opacity: 0.6;
  cursor: not-allowed;
}

.ui-select-value {
  min-width: 0;
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
}

.ui-select-value.placeholder {
  color: color-mix(in srgb, var(--ide-text-muted) 90%, var(--ide-text));
}

.ui-select-caret {
  color: var(--ide-text-muted);
  font-size: 0.9em;
  display: inline-flex;
  align-items: center;
  justify-content: center;
}

.ui-select-popover {
  position: absolute;
  z-index: 200;
  top: calc(100% + 6px);
  left: 0;
  width: 100%;
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-main);
  border-radius: 0;
  box-shadow: 0 12px 26px rgba(0, 0, 0, 0.35);
}

.ui-select-list {
  max-height: 240px;
  overflow: auto;
  padding: 4px;
}

.ui-select-option {
  width: 100%;
  height: 30px;
  padding: 0 8px;
  border-radius: 0;
  border: 1px solid transparent;
  background: transparent;
  color: var(--ide-text);
  cursor: pointer;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 10px;
}

.ui-select-option:hover:not(.disabled),
.ui-select-option.active:not(.disabled) {
  background: var(--ide-hover);
}

.ui-select-option.selected:not(.disabled) {
  background: color-mix(in srgb, var(--ide-accent) 22%, transparent);
}

.ui-select-option.disabled {
  opacity: 0.55;
  cursor: not-allowed;
}

.ui-select-option-label {
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  text-align: left;
}

.ui-select-check {
  color: color-mix(in srgb, var(--ide-accent) 72%, var(--ide-text));
  font-size: 0.9em;
}
</style>

