<script setup lang="ts">
const props = defineProps<{ open: boolean }>();
const emit = defineEmits<{ (e: 'close'): void }>();
</script>

<template>
  <div v-if="props.open" class="about-mask" @click.self="emit('close')">
    <section class="about-dialog">
      <header class="about-header">
        <h3>About AI Editor</h3>
        <button class="close-btn" type="button" @click="emit('close')">x</button>
      </header>
      <div class="about-body ide-scrollbar">
        <p class="lead">
          AI Editor is a lightweight IDE shell inspired by WebStorm and Cursor, built for fast coding, Git workflow,
          terminal operations, and integrated AI assistance.
        </p>
        <div class="block">
          <div class="title">Core Features</div>
          <ul>
            <li>Project tree with context menu, file ops, and bookmarks</li>
            <li>Monaco-based editor with split view and formatting</li>
            <li>Markdown preview and image preview in editor area</li>
            <li>Integrated terminal panel with tab support</li>
            <li>Git panel with refs, timeline, and common actions</li>
            <li>AI assistant panel with selection-aware context</li>
          </ul>
        </div>
        <div class="block">
          <div class="title">Tech Stack</div>
          <p>Vue 3 + TypeScript + Monaco + Tauri (Rust backend)</p>
        </div>
        <div class="block">
          <div class="title">Author</div>
          <div class="author-grid">
            <div class="k">Name</div>
            <div class="v">Livis Snack</div>
            <div class="k">Email</div>
            <div class="v"><a href="mailto:brucesnack@outlook.com">brucesnack@outlook.com</a></div>
            <div class="k">GitHub</div>
            <div class="v"><a href="https://github.com/livissnack" target="_blank" rel="noopener noreferrer">https://github.com/livissnack</a></div>
          </div>
        </div>
        <div class="block">
          <div class="title">Tip</div>
          <p>Open <b>Help → Keyboard Shortcuts</b> to search all available hotkeys.</p>
        </div>
      </div>
    </section>
  </div>
</template>

<style scoped>
.about-mask { position: fixed; inset: 0; z-index: 170; background: rgba(0, 0, 0, 0.45); display: flex; align-items: center; justify-content: center; }
.about-dialog { width: min(680px, calc(100vw - 28px)); max-height: min(78vh, 760px); background: var(--ide-bg-main); border: 1px solid var(--ide-border); display: flex; flex-direction: column; }
.about-header { height: 40px; border-bottom: 1px solid var(--ide-border); display: flex; align-items: center; justify-content: space-between; padding: 0 12px; }
.about-header h3 { margin: 0; font-size: 13px; color: var(--ide-text); }
.close-btn { width: 24px; height: 24px; color: var(--ide-text-muted); cursor: pointer; }
.close-btn:hover { background: var(--ide-hover); color: var(--ide-text); }
.about-body { padding: 12px; overflow: auto; color: var(--ide-text); font-size: 12px; line-height: 1.5; }
.lead { margin: 0 0 12px; color: var(--ide-text); }
.block { margin-top: 10px; padding-top: 10px; border-top: 1px solid color-mix(in srgb, var(--ide-border) 55%, transparent); }
.block .title { font-size: 12px; font-weight: 600; margin-bottom: 6px; color: var(--ide-text); }
ul { margin: 0; padding-left: 18px; }
p { margin: 0; color: var(--ide-text-muted); }
.author-grid {
  display: grid;
  grid-template-columns: 68px minmax(0, 1fr);
  row-gap: 6px;
  column-gap: 8px;
  font-size: 12px;
}
.author-grid .k {
  color: var(--ide-text-muted);
}
.author-grid .v {
  color: var(--ide-text);
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
}
.author-grid .v a {
  color: var(--ide-accent);
  text-decoration: none;
}
.author-grid .v a:hover {
  text-decoration: underline;
}
</style>
