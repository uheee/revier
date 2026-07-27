<script setup lang="ts">
import { NPopover } from 'naive-ui';
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import type { editor } from 'monaco-editor';
import { logError } from '../../api/logger';
import type { AuthorSummary, DiffBlock } from '../../generated/bindings';
import {
  AUTHOR_POPOVER_MAX_HEIGHT,
  AUTHOR_RAIL_WIDTH,
  fitAuthors,
  getDiffBlockGeometry,
  sortBlockAuthors,
  type DiffBlockGeometry
} from '../../editor/diffBlockGeometry';

const props = defineProps<{
  blocks: DiffBlock[];
  draft: boolean;
  selectedBlockId?: string;
  attributionState?: 'computing' | 'attributing' | 'ready' | 'empty' | 'failed';
  originalEditor?: editor.ICodeEditor;
  modifiedEditor?: editor.ICodeEditor;
}>();

const emit = defineEmits<{
  selected: [block: DiffBlock];
}>();

interface PositionedBlock {
  block: DiffBlock;
  geometry: DiffBlockGeometry;
  authors: AuthorSummary[];
  visibleAuthors: AuthorSummary[];
  hasMore: boolean;
}

const positionedBlocks = ref<PositionedBlock[]>([]);
const openBlockId = ref<string>();
const railStyle = computed(() => ({ width: `${AUTHOR_RAIL_WIDTH}px` }));
const popoverStyle = { maxHeight: `${AUTHOR_POPOVER_MAX_HEIGHT}px` };
const listeners: Array<{ dispose(): void }> = [];
let scheduledFrame: number | undefined;

function recomputeLayout(): void {
  if (props.draft || !props.originalEditor || !props.modifiedEditor) {
    positionedBlocks.value = [];
    return;
  }
  positionedBlocks.value = props.blocks.flatMap((block) => {
    const geometry = getDiffBlockGeometry(
      block,
      props.originalEditor!,
      props.modifiedEditor!
    );
    if (!geometry) {
      return [];
    }
    const authors = sortBlockAuthors(block.authors);
    const fitted = fitAuthors(geometry.height, authors);
    return [{ block, geometry, authors, visibleAuthors: fitted.visible, hasMore: fitted.hasMore }];
  });
}

function cancelScheduledLayout(): void {
  if (scheduledFrame === undefined) {
    return;
  }
  cancelAnimationFrame(scheduledFrame);
  scheduledFrame = undefined;
}

function scheduleLayout(): void {
  if (scheduledFrame !== undefined) {
    return;
  }
  scheduledFrame = requestAnimationFrame(() => {
    scheduledFrame = undefined;
    recomputeLayout();
  });
}

function disposeListeners(): void {
  for (const listener of listeners.splice(0)) {
    try {
      listener.dispose();
    } catch (error) {
      logError('AuthorRail 编辑器监听清理失败', error, { source: 'diff-author-rail' });
    }
  }
}

function bindEditor(editorInstance: editor.ICodeEditor): void {
  listeners.push(editorInstance.onDidScrollChange(scheduleLayout));
  listeners.push(editorInstance.onDidLayoutChange(scheduleLayout));
  listeners.push(editorInstance.onDidChangeHiddenAreas(scheduleLayout));
}

function resetSubscriptions(): void {
  cancelScheduledLayout();
  disposeListeners();
  openBlockId.value = undefined;
  if (!props.draft && props.originalEditor && props.modifiedEditor) {
    bindEditor(props.originalEditor);
    if (props.modifiedEditor !== props.originalEditor) {
      bindEditor(props.modifiedEditor);
    }
  }
  recomputeLayout();
}

function selectBlock(block: DiffBlock): void {
  openBlockId.value = undefined;
  emit('selected', block);
}

function setPopoverOpen(blockId: string, show: boolean): void {
  openBlockId.value = show ? blockId : undefined;
}

function displayCommittedAt(value: string): string {
  return value && Number.isFinite(Date.parse(value)) ? value : '未知';
}

function blockLabel(block: DiffBlock, authors: AuthorSummary[]): string {
  const start = block.newStart || block.oldStart;
  const end = block.newEnd || block.oldEnd;
  return `选择 ${start}-${end} 行变更块，${authors.length} 位作者`;
}

function handleBlocksChanged(): void {
  if (openBlockId.value && !props.blocks.some((block) => block.id === openBlockId.value)) {
    openBlockId.value = undefined;
  }
  recomputeLayout();
}

watch(
  [() => props.originalEditor, () => props.modifiedEditor, () => props.draft],
  resetSubscriptions,
  { immediate: true }
);
watch(() => props.blocks, handleBlocksChanged, { deep: true });

onBeforeUnmount(() => {
  cancelScheduledLayout();
  disposeListeners();
});
</script>

<template>
  <aside v-if="!draft" class="diff-author-rail" :style="railStyle">
    <div
      v-for="item in positionedBlocks"
      :key="item.block.id"
      class="diff-author-rail__block"
      :class="[
        `diff-author-rail__block--${item.block.changeType}`,
        {
          'is-selected': item.block.id === selectedBlockId,
          'is-loading': attributionState === 'attributing' && item.authors.length === 0,
          'is-failed': attributionState === 'failed' && item.authors.length === 0
        }
      ]"
      :aria-busy="attributionState === 'attributing' && item.authors.length === 0"
      :style="{ top: `${item.geometry.top}px`, height: `${item.geometry.height}px` }"
    >
      <button
        type="button"
        class="diff-author-rail__select"
        :aria-label="blockLabel(item.block, item.authors)"
        :aria-pressed="item.block.id === selectedBlockId"
        @click="selectBlock(item.block)"
      >
        <span
          v-for="author in item.visibleAuthors"
          :key="`${author.name}:${author.email ?? ''}`"
          class="diff-author-rail__author"
          :title="author.email ? `${author.name} <${author.email}>` : author.name"
        >{{ author.name }}</span>
        <span
          v-if="attributionState === 'attributing' && item.authors.length === 0"
          class="diff-author-rail__status"
        >加载中</span>
        <span
          v-else-if="attributionState === 'failed' && item.authors.length === 0"
          class="diff-author-rail__status"
        >不可用</span>
      </button>
      <NPopover
        v-if="item.hasMore"
        trigger="manual"
        placement="left"
        :show="openBlockId === item.block.id"
        @update:show="setPopoverOpen(item.block.id, $event)"
        @clickoutside="setPopoverOpen(item.block.id, false)"
      >
        <template #trigger>
          <button
            type="button"
            class="diff-author-rail__more"
            aria-label="查看全部作者"
            @click.stop="setPopoverOpen(item.block.id, openBlockId !== item.block.id)"
          >…</button>
        </template>
        <div class="diff-author-rail__popover" :style="popoverStyle">
          <article
            v-for="author in item.authors"
            :key="`${author.name}:${author.email ?? ''}`"
            class="diff-author-rail__popover-author"
          >
            <strong>{{ author.name }}</strong>
            <span>{{ author.email ?? '未知' }}</span>
            <span>{{ author.commitCount }} 次提交</span>
            <time>{{ displayCommittedAt(author.lastCommittedAt) }}</time>
          </article>
        </div>
      </NPopover>
    </div>
  </aside>
</template>

<style scoped>
.diff-author-rail {
  position: relative;
  flex: 0 0 auto;
  min-width: 112px;
  height: 100%;
  overflow: hidden;
  border-left: 1px solid var(--border-color);
  background: var(--panel-background);
}

.diff-author-rail__block {
  position: absolute;
  inset-inline: 4px;
  display: flex;
  align-items: stretch;
  gap: 3px;
  overflow: hidden;
  box-sizing: border-box;
  border-radius: 4px;
  background: color-mix(in srgb, var(--panel-background) 88%, var(--selection-color));
  box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--border-color) 76%, transparent);
}

.diff-author-rail__block::before {
  content: "";
  flex: 0 0 3px;
  width: 3px;
  background: var(--accent-color);
}

.diff-author-rail__block--added::before {
  background: var(--diff-added-strong);
}

.diff-author-rail__block--deleted::before {
  background: var(--diff-removed-strong);
}

.diff-author-rail__block--modified::before {
  background: var(--accent-color);
}

.diff-author-rail__block:hover,
.diff-author-rail__block.is-selected {
  background: color-mix(in srgb, var(--panel-background) 76%, var(--selection-color));
  box-shadow: inset 0 0 0 1px var(--accent-color);
}

.diff-author-rail__block.is-selected {
  box-shadow: inset 0 0 0 2px var(--accent-color);
}

.diff-author-rail__select {
  display: flex;
  flex: 1 1 auto;
  min-width: 0;
  flex-direction: column;
  gap: 2px;
  overflow: hidden;
  padding: 2px 0;
  border: 0;
  color: inherit;
  background: transparent;
  cursor: pointer;
}

.diff-author-rail__select:focus-visible,
.diff-author-rail__more:focus-visible {
  outline: 2px solid var(--accent-color);
  outline-offset: -2px;
}

.diff-author-rail__author,
.diff-author-rail__status,
.diff-author-rail__more {
  display: block;
  flex: 0 0 20px;
  width: 100%;
  min-width: 0;
  overflow: hidden;
  color: var(--muted-color);
  font-size: 12px;
  line-height: 20px;
  text-align: left;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.diff-author-rail__status {
  color: var(--muted-color);
}

.diff-author-rail__more {
  flex: 0 0 22px;
  width: 22px;
  margin: 4px 3px 4px 0;
  padding: 0;
  border: 1px solid var(--border-color);
  border-radius: 3px;
  background: var(--editor-background);
  cursor: pointer;
  text-align: center;
}

.diff-author-rail__more:hover {
  color: var(--accent-color);
}

.diff-author-rail__popover {
  min-width: 240px;
  overflow-y: auto;
}

.diff-author-rail__popover-author {
  display: grid;
  gap: 2px;
  padding: 8px 4px;
  color: var(--foreground-color);
}

.diff-author-rail__popover-author + .diff-author-rail__popover-author {
  border-top: 1px solid var(--border-color);
}

.diff-author-rail__popover-author span,
.diff-author-rail__popover-author time {
  color: var(--muted-color);
  font-size: 12px;
}
</style>
