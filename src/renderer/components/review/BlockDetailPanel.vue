<script setup lang="ts">
import type { DiffBlock, RelatedCommit } from '../../../shared/reviewTypes';
import DiffBlockAuthors from './DiffBlockAuthors.vue';

defineProps<{
  block?: DiffBlock;
  selectedCommitHash?: string;
}>();

const emit = defineEmits<{
  commitSelected: [commit: RelatedCommit];
}>();

function formatDate(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', {
    year: 'numeric',
    month: '2-digit',
    day: '2-digit',
    hour: '2-digit',
    minute: '2-digit'
  }).format(new Date(value));
}

function commitKey(commit: RelatedCommit): string {
  return `${commit.hash}-${commit.touchedRanges.length}`;
}
</script>

<template>
  <aside class="block-detail-panel">
    <header class="section-heading">
      <h2>详情</h2>
    </header>

    <el-empty v-if="!block" :image-size="80" description="未选择变更块" />
    <template v-else>
      <section class="detail-section">
        <span class="detail-kicker">行区间</span>
        <strong>Lines {{ block.newStart || block.oldStart }}-{{ block.newEnd || block.oldEnd }}</strong>
      </section>

      <section class="detail-section">
        <span class="detail-kicker">作者</span>
        <DiffBlockAuthors :authors="block.authors" />
      </section>

      <section class="detail-section">
        <span class="detail-kicker">相关提交</span>
        <el-empty v-if="block.relatedCommits.length === 0" :image-size="64" description="暂无提交" />
        <ul v-else class="commit-list">
          <li v-for="commit in block.relatedCommits" :key="commitKey(commit)" class="commit-list__item">
            <button
              class="commit-row"
              :class="{ 'is-selected': commit.hash === selectedCommitHash }"
              :data-commit-hash="commit.hash"
              type="button"
              @click="emit('commitSelected', commit)"
            >
              <div class="commit-row__top">
                <code>{{ commit.shortHash }}</code>
                <el-tag v-if="commit.matchedByFilter" size="small" type="success" effect="plain">命中筛选</el-tag>
              </div>
              <p>{{ commit.subject }}</p>
              <span>{{ commit.authorName }} · {{ formatDate(commit.committedAt) }}</span>
            </button>
          </li>
        </ul>
      </section>
    </template>
  </aside>
</template>
