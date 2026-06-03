<script setup lang="ts">
import type { DiffBlock, FileOverlay, SideBySideDiffRow, WordChange } from '../../../shared/reviewTypes';
import DiffBlockAuthors from './DiffBlockAuthors.vue';

defineProps<{
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

function visibleWordChanges(row: SideBySideDiffRow, side: 'old' | 'new'): WordChange[] {
  if (!row.wordChanges) {
    return [{ value: side === 'old' ? (row.oldText ?? '') : (row.newText ?? '') }];
  }

  return row.wordChanges.filter((change) => (side === 'old' ? !change.added : !change.removed));
}
</script>

<template>
  <section class="diff-viewer" v-loading="loading">
    <el-empty v-if="!overlay" :image-size="96" description="选择文件后显示 diff" />
    <template v-else-if="overlay.file.isBinary || !overlay.file.isPreviewable">
      <header class="diff-viewer__header">
        <h2>{{ overlay.file.path }}</h2>
      </header>
      <el-alert title="不可预览" type="info" show-icon :closable="false" />
    </template>
    <template v-else>
      <header class="diff-viewer__header">
        <div>
          <h2>{{ overlay.file.path }}</h2>
          <span>{{ overlay.range.baseCommit.slice(0, 8) }}..{{ overlay.range.headCommit.slice(0, 8) }}</span>
        </div>
        <span class="diff-viewer__count">{{ overlay.blocks.length }} 个变更块</span>
      </header>

      <el-empty v-if="overlay.blocks.length === 0" :image-size="96" description="无可显示变更" />
      <article
        v-for="block in overlay.blocks"
        v-else
        :key="block.id"
        class="diff-block"
        :class="{ 'is-selected': block.id === selectedBlockId }"
        @click="emit('selected', block)"
      >
        <header class="diff-block__header">
          <span>Lines {{ block.newStart || block.oldStart }}-{{ block.newEnd || block.oldEnd }}</span>
          <DiffBlockAuthors :authors="block.authors" />
        </header>

        <div class="diff-table">
          <div v-for="(row, index) in block.rows" :key="index" :class="lineClass(row)">
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
          </div>
        </div>
      </article>
    </template>
  </section>
</template>
