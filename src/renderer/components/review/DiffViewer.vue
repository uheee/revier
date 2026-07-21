<script setup lang="ts">
import { computed, ref, shallowRef, watch } from 'vue';
import type { editor } from 'monaco-editor';
import type {
  DiffBlock,
  EditorSettings,
  FileOverlay,
  TextEncoding
} from '../../generated/bindings';
import type { MonacoDiffBlocksPayload } from '../../editor/monacoDiffBlocks';
import { detectEditorLanguage } from '../../editor/editorLanguages';
import DiffAuthorRail from './DiffAuthorRail.vue';
import EditorStatusBar from './EditorStatusBar.vue';
import MonacoDiffSurface from './MonacoDiffSurface.vue';

type DiffViewerMode = 'deferred' | 'original' | 'draft';

// 独立复用与组件测试使用兼容默认值；工作台始终显式传入启动配置快照。
const COMPATIBLE_SETTINGS = {
  version: 1,
  theme: 'system',
  defaultEncoding: 'auto',
  editor: {
    fontFamilies: ['JetBrainsMono Nerd Font Mono', 'Microsoft YaHei', 'monospace'],
    fontSize: 13,
    lineHeight: 22,
    minimap: true
  },
  largeFile: { maxBytes: 1_048_576, maxLines: 5_000 },
  themes: {}
} as EditorSettings;

const props = withDefaults(defineProps<{
  overlay?: FileOverlay;
  selectedBlockId?: string;
  loading?: boolean;
  settings?: EditorSettings;
  themeName?: 'revier-light' | 'revier-dark';
  requestedEncoding?: TextEncoding;
  contextKey?: string | number;
  diffState?: 'computing' | 'attributing' | 'ready' | 'empty' | 'failed';
  hideUnchangedRegions?: boolean;
}>(), {
  selectedBlockId: undefined,
  loading: false,
  settings: undefined,
  themeName: undefined,
  requestedEncoding: undefined,
  contextKey: undefined,
  diffState: 'computing',
  hideUnchangedRegions: false
});

const emit = defineEmits<{
  selected: [block: DiffBlock];
  diffBlocksChange: [payload: MonacoDiffBlocksPayload];
  draftChange: [draft: boolean];
  encodingChange: [encoding: TextEncoding];
  cursorChange: [line: number, column: number];
  editorsReady: [original: editor.ICodeEditor, modified: editor.ICodeEditor];
}>();

const mode = ref<DiffViewerMode>('original');
const manualLanguage = ref<string>();
const cursorLine = ref(1);
const cursorColumn = ref(1);
const originalEditor = shallowRef<editor.ICodeEditor>();
const modifiedEditor = shallowRef<editor.ICodeEditor>();
const sessionVersion = ref(0);

const activeSettings = computed(() => props.settings ?? COMPATIBLE_SETTINGS);
const activeThemeName = computed(() => props.themeName ?? 'revier-light');
const requestedEncoding = computed(() =>
  props.requestedEncoding ?? activeSettings.value.defaultEncoding
);
const automaticLanguage = computed(() => detectEditorLanguage(props.overlay?.file.path ?? ''));
const languageId = computed(() => manualLanguage.value ?? automaticLanguage.value);
const selectedBlock = computed(() =>
  mode.value === 'original'
    ? props.overlay?.blocks.find((block) => block.id === props.selectedBlockId)
    : undefined
);
const selectedBlockIndex = computed(() => {
  if (!selectedBlock.value || !props.overlay) {
    return undefined;
  }
  return props.overlay.blocks.findIndex((block) => block.id === selectedBlock.value?.id);
});
const baseContextKey = computed(() => String(props.contextKey ?? props.overlay?.file.path ?? ''));
const surfaceContextKey = computed(() => `${baseContextKey.value}:${sessionVersion.value}`);

function textLineCount(content: string): number {
  return content.length === 0 ? 0 : content.split('\n').length;
}

const fileMetrics = computed(() => {
  const overlay = props.overlay;
  if (!overlay) {
    return { bytes: 0, lines: 0 };
  }
  return {
    bytes: Math.max(
      new TextEncoder().encode(overlay.oldContent).byteLength,
      new TextEncoder().encode(overlay.newContent).byteLength
    ),
    lines: Math.max(textLineCount(overlay.oldContent), textLineCount(overlay.newContent))
  };
});

function exceedsLargeFileLimit(overlay: FileOverlay): boolean {
  if (overlay.file.isBinary || !overlay.file.isPreviewable) {
    return false;
  }
  return fileMetrics.value.bytes > activeSettings.value.largeFile.maxBytes
    || fileMetrics.value.lines > activeSettings.value.largeFile.maxLines;
}

function resetForOverlay(overlay?: FileOverlay): void {
  mode.value = overlay && exceedsLargeFileLimit(overlay) ? 'deferred' : 'original';
  cursorLine.value = 1;
  cursorColumn.value = 1;
  originalEditor.value = undefined;
  modifiedEditor.value = undefined;
  sessionVersion.value += 1;
}

function enterDraft(): void {
  if (mode.value !== 'original') {
    return;
  }
  mode.value = 'draft';
  emit('draftChange', true);
}

function restoreOriginal(): void {
  mode.value = 'original';
  originalEditor.value = undefined;
  modifiedEditor.value = undefined;
  sessionVersion.value += 1;
  emit('draftChange', false);
}

function loadLargeFile(): void {
  mode.value = 'original';
}

function setEditors(original: editor.ICodeEditor, modified: editor.ICodeEditor): void {
  originalEditor.value = original;
  modifiedEditor.value = modified;
  emit('editorsReady', original, modified);
}

function setCursor(line: number, column: number): void {
  cursorLine.value = line;
  cursorColumn.value = column;
  emit('cursorChange', line, column);
}

function selectBlock(block: DiffBlock): void {
  if (mode.value === 'original') {
    emit('selected', block);
  }
}

function handleDiffBlocksChange(payload: MonacoDiffBlocksPayload): void {
  if (payload.contextKey !== surfaceContextKey.value) {
    return;
  }
  emit('diffBlocksChange', { ...payload, contextKey: baseContextKey.value });
}

let contextInitialized = false;
watch(
  [() => props.overlay, () => props.contextKey],
  ([overlay, contextKey], [previousOverlay, previousContextKey]) => {
    if (contextInitialized && mode.value === 'draft') {
      emit('draftChange', false);
    }
    if (
      !contextInitialized
      || overlay?.file.path !== previousOverlay?.file.path
      || contextKey !== previousContextKey
    ) {
      manualLanguage.value = undefined;
    }
    resetForOverlay(overlay);
    contextInitialized = true;
  },
  { immediate: true }
);
</script>

<template>
  <section class="diff-viewer">
    <n-spin :show="Boolean(loading)" class="diff-viewer__spin">
      <n-empty v-if="!overlay" size="large" description="选择文件后显示 diff" />
      <template v-else>
        <header class="diff-viewer__header">
          <div>
            <h2>{{ overlay.file.path }}</h2>
            <span>{{ overlay.range.baseCommit.slice(0, 8) }}..{{ overlay.range.headCommit.slice(0, 8) }}</span>
          </div>
          <div class="diff-viewer__header-actions">
            <span class="diff-viewer__count">{{ props.diffState === 'computing' && overlay.blocks.length === 0 ? '计算变更块' : `${overlay.blocks.length} 个变更块` }}</span>
            <button
              v-if="mode === 'draft'"
              type="button"
              data-testid="restore-original"
              @click="restoreOriginal"
            >恢复原始内容</button>
          </div>
        </header>

        <main class="diff-viewer__body">
          <n-alert
            v-if="overlay.file.isBinary || !overlay.file.isPreviewable"
            title="不可预览"
            type="info"
            show-icon
            :closable="false"
          />
          <section v-else-if="mode === 'deferred'" class="diff-viewer__large-file">
            <strong>大文件加载确认</strong>
            <span>{{ overlay.file.path }}</span>
            <span>{{ fileMetrics.bytes }} 字节 · {{ fileMetrics.lines }} 行</span>
            <p>文件超过编辑器软限制，确认后再创建编辑器。</p>
            <div>
              <button type="button" data-testid="load-large-file" @click="loadLargeFile">仍然加载</button>
              <button type="button" data-testid="cancel-large-file">取消</button>
            </div>
          </section>
          <n-empty
            v-else-if="props.diffState === 'empty'"
            size="large"
            description="无可显示变更"
          />
          <div v-else class="diff-viewer__editor-layout">
            <MonacoDiffSurface
              :key="surfaceContextKey"
              :path="overlay.file.path"
              :old-content="overlay.oldContent"
              :new-content="overlay.newContent"
              :context-key="surfaceContextKey"
              :language-id="languageId"
              :settings="activeSettings"
              :theme-name="activeThemeName"
              :blocks="overlay.blocks"
              :selected-block="selectedBlock"
              :hide-unchanged-regions="hideUnchangedRegions"
              @draft-change="enterDraft"
              @selected="selectBlock"
              @diff-blocks-change="handleDiffBlocksChange"
              @cursor-change="setCursor"
              @editors-ready="setEditors"
            />
            <DiffAuthorRail
              v-if="mode === 'original'"
              :blocks="overlay.blocks"
              :draft="false"
              :original-editor="originalEditor"
              :modified-editor="modifiedEditor"
              :selected-block-id="selectedBlock?.id"
              :attribution-state="props.diffState"
              @selected="selectBlock"
            />
          </div>
        </main>

        <EditorStatusBar
          :mode="mode === 'draft' ? 'draft' : 'original'"
          :line="cursorLine"
          :column="cursorColumn"
          :resolved-encoding="overlay.resolvedEncoding"
          :requested-encoding="requestedEncoding"
          :language-id="languageId"
          :binary="overlay.file.isBinary"
          :selected-block-index="selectedBlockIndex"
          :block-count="overlay.blocks.length"
          @encoding-change="emit('encodingChange', $event)"
          @language-change="manualLanguage = $event"
        />
      </template>
    </n-spin>
  </section>
</template>

<style scoped>
.diff-viewer {
  height: 100%;
  min-height: 0;
}

.diff-viewer__spin,
.diff-viewer__spin :deep(.n-spin-content) {
  height: 100%;
  min-height: 0;
}

.diff-viewer__spin :deep(.n-spin-content) {
  display: flex;
  flex-direction: column;
}

.diff-viewer__header {
  display: flex;
  flex: 0 0 auto;
  align-items: center;
  justify-content: space-between;
  gap: 16px;
  padding: 10px 14px;
  background: var(--panel-background);
  border-bottom: 1px solid var(--border-color);
}

.diff-viewer__header h2 {
  margin: 0;
  overflow: hidden;
  color: var(--foreground-color);
  font-family: var(--editor-font-family);
  font-size: 13px;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diff-viewer__header span,
.diff-viewer__count {
  color: var(--muted-color);
  font-size: 12px;
}

.diff-viewer__header-actions {
  display: flex;
  align-items: center;
  gap: 12px;
}

.diff-viewer__body {
  display: flex;
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
  background: var(--editor-background);
}

.diff-viewer__editor-layout {
  display: flex;
  flex: 1 1 auto;
  min-width: 0;
  min-height: 0;
}

.diff-viewer__editor-layout > :first-child {
  flex: 1 1 auto;
  min-width: 0;
}

.diff-viewer__large-file {
  display: grid;
  place-content: center;
  justify-items: center;
  gap: 8px;
  width: 100%;
  color: var(--foreground-color);
}

.diff-viewer__large-file > span,
.diff-viewer__large-file > p {
  color: var(--muted-color);
}

:global(.revier-selected-block--old) {
  box-shadow: inset 6px 0 0 var(--diff-removed-strong);
}

:global(.revier-selected-block--new) {
  box-shadow: inset -6px 0 0 var(--diff-added-strong);
}

:global(.revier-selected-block--top) {
  border-top: 2px solid var(--accent-color);
}

:global(.revier-selected-block--bottom) {
  border-bottom: 2px solid var(--accent-color);
}
</style>
