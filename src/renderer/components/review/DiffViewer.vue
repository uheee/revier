<script setup lang="ts">
import { computed } from 'vue';
import type { DiffBlock, FileOverlay, SideBySideDiffRow, WordChange } from '../../../shared/reviewTypes';
import DiffBlockAuthors from './DiffBlockAuthors.vue';

const props = defineProps<{
  overlay?: FileOverlay;
  selectedBlockId?: string;
  loading?: boolean;
}>();

const emit = defineEmits<{
  selected: [block: DiffBlock];
}>();

function lineClass(row: SideBySideDiffRow): string {
  return `diff-row diff-row--${row.type}`;
}

const visibleRows = computed(() => props.overlay?.rows ?? props.overlay?.blocks.flatMap((block) => block.rows) ?? []);
const blocksById = computed(() => new Map((props.overlay?.blocks ?? []).map((block) => [block.id, block])));

function selectRow(row: SideBySideDiffRow): void {
  if (!row.blockId) {
    return;
  }

  const block = blocksById.value.get(row.blockId);
  if (block) {
    emit('selected', block);
  }
}

function blockForRow(row: SideBySideDiffRow): DiffBlock | undefined {
  return row.blockId ? blocksById.value.get(row.blockId) : undefined;
}

function isBlockStart(row: SideBySideDiffRow, index: number): boolean {
  const block = blockForRow(row);
  return Boolean(block && block.rowStartIndex === index);
}

function visibleWordChanges(row: SideBySideDiffRow, side: 'old' | 'new'): WordChange[] {
  if (!row.wordChanges) {
    return [{ value: side === 'old' ? (row.oldText ?? '') : (row.newText ?? '') }];
  }

  return row.wordChanges.filter((change) => (side === 'old' ? !change.added : !change.removed));
}
</script>

<template>
  <section class="diff-viewer">
    <n-spin :show="Boolean(loading)" class="diff-viewer__spin">
      <n-empty v-if="!overlay" size="large" description="选择文件后显示 diff" />
      <template v-else-if="overlay.file.isBinary || !overlay.file.isPreviewable">
        <header class="diff-viewer__header">
          <h2>{{ overlay.file.path }}</h2>
        </header>
        <n-alert title="不可预览" type="info" show-icon :closable="false" />
      </template>
      <template v-else>
        <header class="diff-viewer__header">
          <div>
            <h2>{{ overlay.file.path }}</h2>
            <span>{{ overlay.range.baseCommit.slice(0, 8) }}..{{ overlay.range.headCommit.slice(0, 8) }}</span>
          </div>
          <span class="diff-viewer__count">{{ overlay.blocks.length }} 个变更块</span>
        </header>

        <n-empty v-if="overlay.blocks.length === 0" size="large" description="无可显示变更" />
        <div v-else class="diff-table diff-table--full">
          <div
            v-for="(row, index) in visibleRows"
            :key="index"
            :class="[
              lineClass(row),
              {
                'is-clickable': row.blockId,
                'is-selected': row.blockId === selectedBlockId
              }
            ]"
            :data-block-id="row.blockId"
            @click="selectRow(row)"
          >
            <span class="line-number">{{ row.oldLineNumber ?? '' }}</span>
            <code class="code-cell code-cell--old">
              <template v-for="(change, changeIndex) in visibleWordChanges(row, 'old')" :key="changeIndex">
                <mark v-if="change.removed" class="word word--removed">{{ change.value }}</mark>
                <span v-else>{{ change.value }}</span>
              </template>
            </code>
            <span class="line-number">{{ row.newLineNumber ?? '' }}</span>
            <code class="code-cell code-cell--new">
              <template v-for="(change, changeIndex) in visibleWordChanges(row, 'new')" :key="changeIndex">
                <mark v-if="change.added" class="word word--added">{{ change.value }}</mark>
                <span v-else>{{ change.value }}</span>
              </template>
            </code>
            <DiffBlockAuthors
              v-if="isBlockStart(row, index)"
              class="diff-row__authors"
              :authors="blockForRow(row)?.authors ?? []"
              :attribution="blockForRow(row)?.attribution"
            />
          </div>
        </div>
      </template>
    </n-spin>
  </section>
</template>
