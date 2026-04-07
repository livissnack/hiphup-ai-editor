<script setup lang="ts">
import { computed } from 'vue';

const props = withDefaults(defineProps<{
  modelValue: number;
  min?: number;
  max?: number;
  step?: number;
  placeholder?: string;
  disabled?: boolean;
  clamp?: boolean;
}>(), {
  min: undefined,
  max: undefined,
  step: 1,
  placeholder: '',
  disabled: false,
  clamp: true,
});

const emit = defineEmits<{
  (e: 'update:modelValue', value: number): void;
  (e: 'change', value: number): void;
}>();

const clampValue = (v: number) => {
  let out = v;
  if (typeof props.min === 'number') out = Math.max(props.min, out);
  if (typeof props.max === 'number') out = Math.min(props.max, out);
  return out;
};

const normalizedValue = computed(() => {
  const v = Number(props.modelValue);
  if (Number.isFinite(v)) return v;
  return typeof props.min === 'number' ? props.min : 0;
});

const onInput = (e: Event) => {
  const raw = (e.target as HTMLInputElement).value;
  const v = Number(raw);
  if (!Number.isFinite(v)) return;
  const next = props.clamp ? clampValue(v) : v;
  emit('update:modelValue', next);
};

const onChange = (e: Event) => {
  const raw = (e.target as HTMLInputElement).value;
  const v = Number(raw);
  if (!Number.isFinite(v)) return;
  const next = props.clamp ? clampValue(v) : v;
  emit('update:modelValue', next);
  emit('change', next);
};
</script>

<template>
  <input
    class="ui-number"
    type="number"
    :value="String(normalizedValue)"
    :min="props.min"
    :max="props.max"
    :step="props.step"
    :placeholder="props.placeholder"
    :disabled="props.disabled"
    @input="onInput"
    @change="onChange"
  >
</template>

<style scoped>
.ui-number {
  height: 32px;
  width: 100%;
  border: 1px solid color-mix(in srgb, var(--ide-border) 92%, transparent);
  background: color-mix(in srgb, var(--ide-bg-elevated) 88%, transparent);
  color: var(--ide-text);
  border-radius: 0;
  padding: 0 10px;
  font-size: 0.95em;
  outline: none;
  box-shadow: none;
  line-height: 32px;
}

.ui-number:hover:not(:disabled) {
  border-color: color-mix(in srgb, var(--ide-border) 70%, var(--ide-text-muted));
}

.ui-number:focus {
  border-color: color-mix(in srgb, var(--ide-accent) 62%, var(--ide-border));
  box-shadow: 0 0 0 1px color-mix(in srgb, var(--ide-accent) 55%, transparent);
}

.ui-number:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

/* Remove native spinners for a cleaner IDE look */
input[type='number']::-webkit-outer-spin-button,
input[type='number']::-webkit-inner-spin-button {
  -webkit-appearance: none;
  margin: 0;
}
input[type='number'] {
  -moz-appearance: textfield;
  appearance: textfield;
}
</style>

