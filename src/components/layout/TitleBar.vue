<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref } from 'vue';
import { getCurrentWindow } from '@tauri-apps/api/window';
import { menuMap, menuOrder, type FullMenuKey } from '../../config/menuConfig';
import { formatShortcutForDisplay } from '../../utils/shortcutDisplay';

const appWindow = getCurrentWindow();
const props = withDefaults(defineProps<{
  hasWorkspace?: boolean;
  isGitRepo?: boolean;
}>(), {
  hasWorkspace: false,
  isGitRepo: false,
});

const minimize = async () => await appWindow.minimize();
const toggleMaximize = async () => {
  await appWindow.toggleMaximize();
};
const close = async () => await appWindow.close();
const emit = defineEmits<{
  (e: 'menu-action', action: string): void;
}>();

const fullActiveMenu = ref<FullMenuKey | null>(null);
const menuTriggerRefs = ref<Partial<Record<FullMenuKey, HTMLElement>>>({});

const toggleMenu = (menu: FullMenuKey) => {
  fullActiveMenu.value = fullActiveMenu.value === menu ? null : menu;
};

const setMenuTriggerRef = (menu: FullMenuKey, element: Element | null) => {
  if (!element) return;
  menuTriggerRefs.value[menu] = element as HTMLElement;
};

const dropdownStyle = computed(() => {
  if (!fullActiveMenu.value) return {};
  const anchor = menuTriggerRefs.value[fullActiveMenu.value];
  if (!anchor) return {};
  return {
    left: `${anchor.offsetLeft}px`,
    top: `${anchor.offsetTop + anchor.offsetHeight + 4}px`,
  };
});

const closeMenu = () => {
  fullActiveMenu.value = null;
};

const onMenuItemClick = (action?: string) => {
  if (action && isActionDisabled(action)) return;
  if (action) {
    emit('menu-action', action);
  }
  closeMenu();
};

const isActionDisabled = (action?: string) => {
  if (!action) return false;
  if (!action.startsWith('git-')) return false;
  if (!props.hasWorkspace) return true;
  // Allow opening git tool window even outside repo to show message panel.
  if (action === 'git-open') return false;
  return !props.isGitRepo;
};

const onWindowPointerDown = (event: PointerEvent) => {
  const target = event.target as HTMLElement | null;
  if (!target?.closest('.menu-area')) {
    closeMenu();
  }
};

onMounted(() => {
  window.addEventListener('pointerdown', onWindowPointerDown, { passive: true });
});

onBeforeUnmount(() => {
  window.removeEventListener('pointerdown', onWindowPointerDown);
});
</script>

<template>
  <div class="title-bar" data-tauri-drag-region>
    <div class="left-section">
      <img src="../../assets/logo.svg" class="app-icon" alt="" />
      <div class="menu-area" data-tauri-no-drag @pointerdown.stop>
        <button
          v-for="menu in menuOrder"
          :key="menu.key"
          class="menu-item"
          :class="{ active: fullActiveMenu === menu.key }"
          :ref="(el) => setMenuTriggerRef(menu.key, el)"
          @click="toggleMenu(menu.key)"
        >
          {{ menu.label }}
        </button>

        <div v-if="fullActiveMenu" class="menu-dropdown" :style="dropdownStyle">
          <template v-for="(item, itemIdx) in menuMap[fullActiveMenu]" :key="`${fullActiveMenu}-${item.label}-${itemIdx}`">
            <div v-if="item.separator" class="menu-separator" />
            <button
              v-else
              class="menu-dropdown-item"
              :class="{ disabled: isActionDisabled(item.action) }"
              :disabled="isActionDisabled(item.action)"
              @click="onMenuItemClick(item.action)"
            >
              <span>{{ item.label }}</span>
              <span v-if="item.shortcut" class="shortcut">{{ formatShortcutForDisplay(item.shortcut) }}</span>
              <span v-else-if="item.children?.length" class="shortcut">▶</span>
            </button>
            <div
              v-if="item.children?.length"
              class="menu-dropdown submenu"
            >
              <template v-for="(child, childIdx) in item.children" :key="`${fullActiveMenu}-${item.label}-${child.label}-${childIdx}`">
                <div v-if="child.separator" class="menu-separator" />
                <div v-else-if="child.header" class="menu-header">{{ child.label }}</div>
                <button
                  v-else
                  class="menu-dropdown-item"
                  :class="{ disabled: isActionDisabled(child.action) }"
                  :disabled="isActionDisabled(child.action)"
                  @click="onMenuItemClick(child.action)"
                >
                  <span>{{ child.label }}</span>
                  <span v-if="child.shortcut" class="shortcut">{{ formatShortcutForDisplay(child.shortcut) }}</span>
                </button>
              </template>
            </div>
          </template>
        </div>
      </div>
    </div>

    <div class="center-section">
      <span class="app-title">hiphup-ai-editor</span>
    </div>

    <div class="right-section">
      <div class="window-controls" data-tauri-no-drag>
        <button class="control-btn control-minimize" title="Minimize" @click="minimize">
          <FontAwesomeIcon :icon="['fas', 'minus']" />
        </button>
        <button class="control-btn control-maximize" title="Toggle Window Size" @click="toggleMaximize">
          <FontAwesomeIcon :icon="['far', 'clone']" />
        </button>
        <button class="control-btn control-close close" title="Close" @click="close">
          <FontAwesomeIcon :icon="['fas', 'xmark']" />
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.title-bar {
  height: var(--ide-titlebar-height);
  background-color: var(--ide-bg-main);
  position: relative;
  /* Above .ide-body (isolation + .ide-center z-index); below .notice-stack (160). */
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0 8px;
  user-select: none;
  border-bottom: 1px solid var(--ide-border);
  /* Avoid paint containment: menu dropdown extends below this bar. */
  contain: layout style;
}
.left-section { display: flex; align-items: center; gap: 10px; font-size: 12px; }
.app-icon { width: 16px; height: 16px; }
.menu-area {
  position: relative;
  display: flex;
  align-items: center;
}
.menu-item {
  height: 24px;
  padding: 0 10px;
  color: var(--ide-text);
  border-radius: 4px;
  cursor: pointer;
}
.menu-item:hover,
.menu-item.active {
  background: var(--ide-hover);
}
.menu-dropdown {
  position: absolute;
  top: calc(100% + 6px);
  left: 0;
  min-width: 200px;
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  border-radius: 6px;
  padding: 6px 0;
  z-index: 1;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.28);
}
.menu-dropdown.submenu {
  top: 0;
  left: calc(100% - 2px);
  z-index: 2;
  display: none;
}
.menu-dropdown-item:has(+ .menu-dropdown.submenu):hover + .menu-dropdown.submenu,
.menu-dropdown.submenu:hover {
  display: block;
}
.menu-dropdown-item {
  width: 100%;
  height: 28px;
  padding: 0 12px;
  display: flex;
  align-items: center;
  justify-content: space-between;
  color: var(--ide-text);
  font-size: 12px;
  cursor: pointer;
}
.menu-dropdown-item:hover {
  background: var(--ide-hover);
}
.menu-dropdown-item.disabled {
  color: var(--ide-text-muted);
  opacity: 0.6;
  cursor: not-allowed;
}
.menu-dropdown-item.disabled:hover {
  background: transparent;
}
.shortcut {
  color: var(--ide-text-muted);
  margin-left: 14px;
  font-size: 11px;
}
.menu-separator {
  height: 1px;
  margin: 6px 8px;
  background: var(--ide-border);
}
.menu-header {
  height: 22px;
  display: flex;
  align-items: center;
  padding: 0 12px;
  font-size: 10px;
  font-weight: 700;
  letter-spacing: 0.04em;
  text-transform: uppercase;
  color: var(--ide-text-muted);
  opacity: 0.9;
}
.center-section {
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
  display: inline-flex;
  align-items: center;
  justify-content: center;
  pointer-events: none;
}
.app-title {
  color: var(--ide-text);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0.02em;
  opacity: 0.92;
}

.window-controls { display: flex; height: 100%; }
.control-btn {
  width: 38px;
  height: 38px;
  display: flex;
  align-items: center;
  justify-content: center;
  cursor: pointer;
  color: var(--ide-text-muted);
  :deep(svg) { width: 11px; height: 11px; }
}
.control-minimize :deep(svg) { width: 10.5px; height: 10.5px; }
.control-maximize :deep(svg) { width: 10.5px; height: 10.5px; }
.control-close :deep(svg) { width: 10.5px; height: 10.5px; }
.control-btn:hover { background-color: var(--ide-hover); color: var(--ide-text); }
</style>