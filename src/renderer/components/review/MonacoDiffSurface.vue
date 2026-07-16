<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, watch } from 'vue';
import type { editor } from 'monaco-editor';
import type { DiffBlock, EditorSettings } from '../../generated/bindings';
import {
  createMonacoDiffSession,
  type MonacoDiffSession
} from '../../editor/monacoDiffSession';

const props = defineProps<{
  path: string;
  oldContent: string;
  newContent: string;
  contextKey: string | number;
  languageId: string;
  settings: EditorSettings;
  themeName: 'revier-light' | 'revier-dark';
  blocks: DiffBlock[];
  selectedBlock?: DiffBlock;
}>();

const emit = defineEmits<{
  draftChange: [value: true];
  selected: [block: DiffBlock];
  cursorChange: [line: number, column: number];
  editorsReady: [original: editor.ICodeEditor, modified: editor.ICodeEditor];
}>();

const container = ref<HTMLElement>();
const loadError = ref<string>();
let session: MonacoDiffSession | undefined;
let resizeObserver: ResizeObserver | undefined;

function disposeSession(): void {
  try {
    session?.dispose();
  } finally {
    session = undefined;
  }
}

function createSession(): void {
  disposeSession();
  loadError.value = undefined;
  if (!container.value) {
    return;
  }

  try {
    session = createMonacoDiffSession({
      container: container.value,
      path: props.path,
      oldContent: props.oldContent,
      newContent: props.newContent,
      languageId: props.languageId,
      settings: props.settings,
      themeName: props.themeName,
      blocks: props.blocks,
      onDraftChange: (value) => emit('draftChange', value),
      onBlockSelected: (block) => emit('selected', block),
      onCursorChange: (line, column) => emit('cursorChange', line, column),
      onEditorsReady: (original, modified) => emit('editorsReady', original, modified)
    });
    session.setSelectedBlock(props.selectedBlock);
  } catch (error) {
    disposeSession();
    loadError.value = error instanceof Error ? error.message : String(error);
  }
}

watch([
  () => props.path,
  () => props.oldContent,
  () => props.newContent,
  () => props.contextKey,
  () => props.blocks
], createSession);
watch(() => props.languageId, (languageId) => session?.setLanguage(languageId));
watch(() => props.themeName, (themeName) => session?.setTheme(themeName));
watch(() => props.selectedBlock, (block) => session?.setSelectedBlock(block));

onMounted(() => {
  createSession();
  resizeObserver = new ResizeObserver(() => session?.layout());
  resizeObserver.observe(container.value!);
});

onBeforeUnmount(() => {
  resizeObserver?.disconnect();
  resizeObserver = undefined;
  disposeSession();
});
</script>

<template>
  <section class="monaco-diff-surface">
    <div ref="container" class="monaco-diff-surface__editor" />
    <div v-if="loadError" class="monaco-diff-surface__error" role="alert">
      <strong>编辑器加载失败</strong>
      <span>{{ loadError }}</span>
      <button type="button" @click="createSession">重试加载</button>
    </div>
  </section>
</template>

<style scoped>
.monaco-diff-surface,
.monaco-diff-surface__editor {
  width: 100%;
  height: 100%;
  min-width: 0;
  min-height: 0;
}

.monaco-diff-surface {
  position: relative;
}

.monaco-diff-surface__error {
  position: absolute;
  inset: 0;
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 8px;
  color: var(--foreground-color);
  background: var(--editor-background);
}
</style>
