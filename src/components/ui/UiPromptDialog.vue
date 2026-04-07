<script setup lang="ts">
import { nextTick, ref, watch } from 'vue';

const props = withDefaults(defineProps<{
  open: boolean;
  mode?: 'input' | 'confirm' | 'select';
  title: string;
  message?: string;
  modelValue?: string;
  options?: Array<{ label: string; value: string }>;
  placeholder?: string;
  confirmText?: string;
  cancelText?: string;
}>(), {
  mode: 'input',
  message: '',
  modelValue: '',
  options: () => [],
  placeholder: '',
  confirmText: 'OK',
  cancelText: 'Cancel',
});

const emit = defineEmits<{
  (e: 'update:open', value: boolean): void;
  (e: 'update:modelValue', value: string): void;
  (e: 'confirm'): void;
  (e: 'cancel'): void;
}>();

const inputRef = ref<HTMLInputElement | null>(null);

watch(
  () => props.open,
  (open) => {
    if (!open) return;
    nextTick(() => {
      if (props.mode === 'input') inputRef.value?.focus();
    });
  },
);

const close = () => {
  emit('update:open', false);
  emit('cancel');
};

const submit = () => {
  emit('confirm');
};

const onMaskClick = () => {
  close();
};
</script>

<template>
  <div v-if="open" class="ui-prompt-mask" @click.self="onMaskClick">
    <section class="ui-prompt-dialog" @keydown.esc.prevent="close">
      <header class="ui-prompt-header">
        <h3>{{ title }}</h3>
      </header>
      <div class="ui-prompt-body">
        <p v-if="message" class="ui-prompt-message">{{ message }}</p>
        <input
          v-if="mode === 'input'"
          ref="inputRef"
          class="ui-prompt-input"
          type="text"
          :value="modelValue"
          :placeholder="placeholder"
          @input="emit('update:modelValue', ($event.target as HTMLInputElement).value)"
          @keydown.enter.prevent="submit"
        >
        <select
          v-else-if="mode === 'select'"
          class="ui-prompt-input"
          :value="modelValue"
          @change="emit('update:modelValue', ($event.target as HTMLSelectElement).value)"
          @keydown.enter.prevent="submit"
        >
          <option v-for="opt in options" :key="opt.value" :value="opt.value">{{ opt.label }}</option>
        </select>
      </div>
      <footer class="ui-prompt-actions">
        <button class="ui-btn ghost" type="button" @click="close">{{ cancelText }}</button>
        <button class="ui-btn primary" type="button" @click="submit">{{ confirmText }}</button>
      </footer>
    </section>
  </div>
</template>

<style scoped>
.ui-prompt-mask {
  position: fixed;
  inset: 0;
  z-index: 180;
  background: rgba(0, 0, 0, 0.42);
  display: flex;
  align-items: center;
  justify-content: center;
}
.ui-prompt-dialog {
  width: min(460px, calc(100vw - 28px));
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  box-shadow: 0 14px 34px rgba(0, 0, 0, 0.35);
  border-radius: 0;
}
.ui-prompt-header {
  height: 40px;
  display: flex;
  align-items: center;
  padding: 0 12px;
  border-bottom: 1px solid var(--ide-border);
}
.ui-prompt-header h3 {
  margin: 0;
  font-size: 13px;
  color: var(--ide-text);
}
.ui-prompt-body {
  padding: 12px;
}
.ui-prompt-message {
  margin: 0 0 10px;
  font-size: 12px;
  color: var(--ide-text-muted);
  line-height: 1.45;
  white-space: pre-wrap;
}
.ui-prompt-input {
  width: 100%;
  height: 30px;
  border: 1px solid var(--ide-border);
  background: var(--ide-bg-editor);
  color: var(--ide-text);
  font-size: 12px;
  padding: 0 8px;
}
.ui-prompt-input:focus {
  outline: none;
  border-color: color-mix(in srgb, var(--ide-accent) 55%, var(--ide-border));
}
.ui-prompt-actions {
  display: flex;
  justify-content: flex-end;
  gap: 8px;
  padding: 0 12px 12px;
}
.ui-btn {
  min-width: 78px;
  height: 28px;
  border: 1px solid var(--ide-border);
  font-size: 12px;
  color: var(--ide-text);
  cursor: pointer;
}
.ui-btn.ghost {
  background: transparent;
}
.ui-btn.primary {
  background: color-mix(in srgb, var(--ide-accent) 16%, var(--ide-bg-main));
  border-color: color-mix(in srgb, var(--ide-accent) 45%, var(--ide-border));
}
.ui-btn:hover {
  background: var(--ide-hover);
}
</style>
