<script setup lang="ts">
const props = defineProps<{
  active: boolean;
  agentActive: boolean;
}>();

const emit = defineEmits<{
  (e: 'update:active', value: boolean): void;
  (e: 'update:agentActive', value: boolean): void;
}>();
</script>

<template>
  <aside class="sidebar-icons right">
    <div class="group top">
      <button
        class="icon-btn"
        :class="{ active: active && !agentActive }"
        title="AI Assistant"
        @click="emit('update:active', !active)"
      >
        <span class="v-text">AI Assistant</span>
      </button>
      <button
        class="icon-btn"
        :class="{ active: agentActive }"
        title="Agent"
        @click="emit('update:agentActive', !agentActive)"
      >
        <span class="v-text">Agent</span>
      </button>
    </div>
  </aside>
</template>

<style lang="scss" scoped>
.sidebar-icons {
  width: var(--ide-sidebar-width);
  background: var(--ide-bg-main);
  display: flex;
  flex-direction: column;
  justify-content: space-between;
  position: relative;
  z-index: 30;
  border-left: 1px solid var(--ide-border);

  .icon-btn {
    width: 100%;
    padding: 15px 0;
    display: flex;
    justify-content: center;
    align-items: center;
    border: 0;
    background: transparent;
    cursor: pointer;
    transition: background 0.2s;
    color: var(--ide-text-muted);

    &.active {
      background: color-mix(in srgb, var(--ide-bg-editor) 70%, transparent);
      color: var(--ide-accent);
      box-shadow: inset -2px 0 0 var(--ide-accent);
    }

    &:hover:not(.active) { background: var(--ide-hover); }

    &:focus-visible {
      outline: 1px solid color-mix(in srgb, var(--ide-accent) 55%, transparent);
      outline-offset: -1px;
    }
  }

  .v-text {
    writing-mode: vertical-lr;
    transform: rotate(180deg);
    font-size: 11px;
    font-weight: 500;
  }
}
</style>

