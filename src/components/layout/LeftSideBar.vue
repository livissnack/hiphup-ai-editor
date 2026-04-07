<script setup lang="ts">
type LeftTool = 'explorer' | 'search' | 'source' | 'run' | 'extensions';

const props = withDefaults(defineProps<{
  active: boolean;
  leftTool?: LeftTool;
}>(), {
  leftTool: 'explorer',
});

const emit = defineEmits<{
  (e: 'update:active', value: boolean): void;
  (e: 'select-left-tool', tool: LeftTool): void;
  (e: 'open-git'): void;
  (e: 'open-terminal'): void;
}>();

const leftTools: Array<{ id: LeftTool; icon: [string, string]; title: string }> = [
  { id: 'explorer', icon: ['fas', 'folder'], title: 'Explorer' },
  { id: 'search', icon: ['fas', 'magnifying-glass'], title: 'Search' },
  { id: 'extensions', icon: ['fas', 'puzzle-piece'], title: 'Extensions' },
];

const onLeftToolClick = (tool: LeftTool) => {
  emit('select-left-tool', tool);
  emit('update:active', true);
};

</script>

<template>
  <aside class="sidebar-icons left">
    <div class="group top">
      <button
        v-for="tool in leftTools"
        :key="tool.id"
        class="icon-btn"
        :class="{ active: leftTool === tool.id, collapsed: !active && leftTool === tool.id }"
        :title="tool.title"
        @click="onLeftToolClick(tool.id)"
      >
        <FontAwesomeIcon class="icon" :icon="tool.icon" />
      </button>
    </div>

    <div class="group bottom">
      <button class="icon-btn setting terminal-shortcut" title="Terminal" @click="emit('open-terminal')">
        <span class="terminal-icon-stack" aria-hidden="true">
          <FontAwesomeIcon class="icon frame" :icon="['fas', 'square']" />
          <FontAwesomeIcon class="icon glyph" :icon="['fas', 'terminal']" />
        </span>
      </button>
      <button class="icon-btn setting" title="Git" @click="emit('open-git')">
        <FontAwesomeIcon class="icon" :icon="['fas', 'code-branch']" />
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
  border-right: 1px solid var(--ide-border);

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
      box-shadow: inset 2px 0 0 var(--ide-accent);
    }

    &.collapsed {
      background: transparent;
    }

    &:hover:not(.active) { background: var(--ide-hover); }

    &:focus-visible {
      outline: 1px solid color-mix(in srgb, var(--ide-accent) 55%, transparent);
      outline-offset: -1px;
    }
  }

  .group.bottom .icon-btn:hover {
    color: var(--ide-text);
  }

  .icon {
    font-size: 14px;
    line-height: 1;
  }

  .terminal-shortcut .terminal-icon-stack {
    position: relative;
    width: 16px;
    height: 16px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }

  .terminal-shortcut .terminal-icon-stack .icon {
    position: absolute;
    line-height: 1;
  }

  .terminal-shortcut .terminal-icon-stack .frame {
    font-size: 16px;
    color: color-mix(in srgb, var(--ide-text-muted) 82%, var(--ide-border));
  }

  .terminal-shortcut .terminal-icon-stack .glyph {
    font-size: 9.5px;
    color: color-mix(in srgb, var(--ide-text) 92%, var(--ide-text-muted));
    transform: translateY(0.4px);
    text-shadow: 0 0 0.01px currentColor;
  }

  .terminal-shortcut:hover .terminal-icon-stack .frame {
    color: color-mix(in srgb, var(--ide-accent) 45%, var(--ide-text-muted));
  }

  .terminal-shortcut:hover .terminal-icon-stack .glyph,
  .terminal-shortcut:focus-visible .terminal-icon-stack .glyph,
  .terminal-shortcut.active .terminal-icon-stack .glyph {
    color: #111111;
  }

  .group.bottom .setting:hover .icon {
    color: var(--ide-text);
  }
}
</style>

