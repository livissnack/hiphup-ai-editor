<script setup lang="ts">
import { ref, watch } from 'vue';
import UiSelect from '../ui/UiSelect.vue';
import UiNumberInput from '../ui/UiNumberInput.vue';

type SettingsSection = 'appearance' | 'editor' | 'autosave' | 'shell';

const props = defineProps<{
  open: boolean;
  initialSection?: SettingsSection;
  theme: 'dark' | 'light' | 'monaco';
  editorFontSize: number;
  treeFontSize: number;
  autoSaveDelay: number;
  indentTabSize: number;
  indentInsertSpaces: boolean;
}>();

const emit = defineEmits<{
  (e: 'close'): void;
  (e: 'update:theme', value: 'dark' | 'light' | 'monaco'): void;
  (e: 'update:editorFontSize', value: number): void;
  (e: 'update:treeFontSize', value: number): void;
  (e: 'update:autoSaveDelay', value: number): void;
  (e: 'update:indentTabSize', value: number): void;
  (e: 'update:indentInsertSpaces', value: boolean): void;
  (e: 'install-cli'): void;
}>();

const activeSection = ref<SettingsSection>('appearance');

watch(
  () => [props.open, props.initialSection] as const,
  ([open, section]) => {
    if (open && section) {
      activeSection.value = section;
    }
  },
  { immediate: true },
);
</script>

<template>
  <div v-if="open" class="settings-mask" @click.self="emit('close')">
    <section class="settings-dialog" :style="{ '--settings-font-size': `${props.editorFontSize}px` }">
      <header class="settings-header">
        <h3>Settings</h3>
        <button class="close-btn" @click="emit('close')">x</button>
      </header>

      <div class="settings-main">
        <aside class="settings-nav">
          <button class="nav-item" :class="{ active: activeSection === 'appearance' }" @click="activeSection = 'appearance'">
            Appearance
          </button>
          <button class="nav-item" :class="{ active: activeSection === 'editor' }" @click="activeSection = 'editor'">
            Editor
          </button>
          <button class="nav-item" :class="{ active: activeSection === 'autosave' }" @click="activeSection = 'autosave'">
            Auto Save
          </button>
          <button class="nav-item" :class="{ active: activeSection === 'shell' }" @click="activeSection = 'shell'">
            Shell / CLI
          </button>
        </aside>

        <div class="settings-body ide-scrollbar">
          <template v-if="activeSection === 'appearance'">
            <label class="setting-row">
              <span class="label">Theme</span>
              <UiSelect
                :model-value="props.theme"
                :options="[
                  { value: 'dark', label: 'JetBrains Dark' },
                  { value: 'light', label: 'IntelliJ Light' },
                  { value: 'monaco', label: 'Monaco Dark' },
                ]"
                @update:model-value="emit('update:theme', $event as any)"
              />
            </label>
            <label class="setting-row">
              <span class="label">Tree Font Size</span>
              <UiNumberInput
                :model-value="props.treeFontSize"
                :min="10"
                :max="20"
                :step="1"
                @update:model-value="emit('update:treeFontSize', $event)"
              />
            </label>
          </template>

          <template v-else-if="activeSection === 'editor'">
            <label class="setting-row">
              <span class="label">Editor Font Size</span>
              <UiNumberInput
                :model-value="props.editorFontSize"
                :min="11"
                :max="24"
                :step="1"
                @update:model-value="emit('update:editorFontSize', $event)"
              />
            </label>
            <label class="setting-row">
              <span class="label">Indent Mode</span>
              <UiSelect
                :model-value="props.indentInsertSpaces ? 'spaces' : 'tabs'"
                :options="[
                  { value: 'spaces', label: 'Spaces' },
                  { value: 'tabs', label: 'Tabs' },
                ]"
                @update:model-value="emit('update:indentInsertSpaces', $event === 'spaces')"
              />
            </label>
            <label class="setting-row">
              <span class="label">{{ props.indentInsertSpaces ? 'Spaces Per Indent' : 'Tab Size' }}</span>
              <UiNumberInput
                :model-value="props.indentTabSize"
                :min="1"
                :max="16"
                :step="1"
                @update:model-value="emit('update:indentTabSize', $event)"
              />
            </label>
          </template>

          <template v-else-if="activeSection === 'autosave'">
            <label class="setting-row">
              <span class="label">Auto Save Delay (ms)</span>
              <UiNumberInput
                :model-value="props.autoSaveDelay"
                :min="200"
                :max="5000"
                :step="100"
                @update:model-value="emit('update:autoSaveDelay', $event)"
              />
            </label>
          </template>
          <template v-else>
            <p class="shell-intro">
              Install a small launcher and add it to your user PATH (Windows), or get a script path to add manually (macOS / Linux).
              Then in a terminal you can run <code>ai-editor .</code> to open the current folder, or <code>ai-editor path/to/file.ts</code> to open a file.
            </p>
            <div class="setting-row shell-actions">
              <button type="button" class="shell-install-btn" @click="emit('install-cli')">
                Install CLI / update PATH
              </button>
            </div>
          </template>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.settings-mask {
  position: fixed;
  inset: 0;
  z-index: 120;
  background: rgba(0, 0, 0, 0.45);
  display: flex;
  align-items: center;
  justify-content: center;
}

.settings-dialog {
  width: 700px;
  max-width: calc(100vw - 32px);
  background: var(--ide-bg-main);
  border: 1px solid var(--ide-border);
  border-radius: 0;
  box-shadow: 0 16px 42px rgba(0, 0, 0, 0.35);
  font-size: var(--settings-font-size, 13px);
  color: var(--ide-text);
}

.settings-header {
  height: 42px;
  padding: 0 12px;
  border-bottom: 1px solid var(--ide-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.settings-header h3 {
  margin: 0;
  font-size: 1em;
  color: var(--ide-text);
}

.close-btn {
  width: 24px;
  height: 24px;
  border-radius: 0;
  color: var(--ide-text-muted);
  cursor: pointer;
}

.close-btn:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}

.settings-main {
  display: grid;
  grid-template-columns: 180px 1fr;
  min-height: 340px;
}

.settings-nav {
  border-right: 1px solid var(--ide-border);
  padding: 10px 8px;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.nav-item {
  height: 30px;
  text-align: left;
  padding: 0 10px;
  border-radius: 0;
  color: var(--ide-text-muted);
  cursor: pointer;
  font-size: 0.95em;
}

.nav-item:hover {
  background: var(--ide-hover);
  color: var(--ide-text);
}

.nav-item.active {
  background: color-mix(in srgb, var(--ide-accent) 22%, transparent);
  color: var(--ide-text);
}

.settings-body {
  padding: 16px 14px;
  display: flex;
  flex-direction: column;
  gap: 12px;
  max-height: 420px;
  overflow: auto;
}

.setting-row {
  display: grid;
  grid-template-columns: 180px 1fr;
  align-items: center;
  gap: 10px;
}

.label {
  font-size: 0.92em;
  color: var(--ide-text-muted);
}

.shell-intro {
  margin: 0;
  font-size: 0.92em;
  line-height: 1.55;
  color: var(--ide-text-muted);
}

.shell-intro code {
  font-family: ui-monospace, Menlo, Consolas, monospace;
  font-size: 0.9em;
  padding: 1px 5px;
  border-radius: 0;
  background: color-mix(in srgb, var(--ide-bg-elevated) 80%, transparent);
  border: 1px solid var(--ide-border);
  color: var(--ide-text);
}

.shell-actions {
  display: block;
}

.shell-install-btn {
  height: 32px;
  padding: 0 14px;
  border-radius: 0;
  border: 1px solid var(--ide-border);
  background: color-mix(in srgb, var(--ide-accent) 24%, transparent);
  color: var(--ide-text);
  cursor: pointer;
  font-size: 0.95em;
}

.shell-install-btn:hover {
  background: color-mix(in srgb, var(--ide-accent) 34%, transparent);
}
</style>
