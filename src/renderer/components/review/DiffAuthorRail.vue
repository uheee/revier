<script setup lang="ts">
import { NPopover } from 'naive-ui';
import { computed, onBeforeUnmount, ref, watch } from 'vue';
import { editor } from 'monaco-editor';
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
      props.modifiedEditor!,
      editor.EditorOption.lineHeight
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
      console.error('AuthorRail 编辑器监听清理失败', error);
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

function activateBlock(event: KeyboardEvent, block: DiffBlock): void {
  if (event.target !== event.currentTarget || (event.key !== 'Enter' && event.key !== ' ')) {
    return;
  }
  event.preventDefault();
  selectBlock(block);
}

function displayCommittedAt(value: string): string {
  return value && Number.isFinite(Date.parse(value)) ? value : '未知';
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
      role="button"
      tabindex="0"
      :style="{ top: `${item.geometry.top}px`, height: `${item.geometry.height}px` }"
      @click="selectBlock(item.block)"
      @keydown="activateBlock($event, item.block)"
    >
      <span
        v-for="author in item.visibleAuthors"
        :key="`${author.name}:${author.email ?? ''}`"
        class="diff-author-rail__author"
        :title="author.email ? `${author.name} <${author.email}>` : author.name"
      >{{ author.name }}</span>
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
  inset-inline: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
  overflow: hidden;
  box-sizing: border-box;
  padding-inline: 5px;
  cursor: pointer;
}

.diff-author-rail__author,
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

.diff-author-rail__more {
  padding: 0;
  border: 0;
  background: transparent;
  cursor: pointer;
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
