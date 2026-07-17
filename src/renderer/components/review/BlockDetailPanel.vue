<script setup lang="ts">
import { LoaderCircle, OctagonX } from 'lucide-vue-next';
import type { DiffBlock, RelatedCommit } from '../../generated/bindings';
import NotificationCenter from '../NotificationCenter.vue';

defineProps<{
  block?: DiffBlock;
  selectedCommitHash?: string;
  activeCommitHash?: string;
  draft?: boolean;
}>();

const emit = defineEmits<{
  commitSelected: [commit: RelatedCommit];
  cancelCommit: [];
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
      <NotificationCenter />
    </header>

    <n-empty v-if="draft" size="small" description="临时草稿不提供归因" />
    <n-empty v-else-if="!block" size="small" description="未选择变更块" />
    <template v-else>
      <section class="detail-section">
        <span class="detail-kicker">行区间</span>
        <strong>Lines {{ block.newStart || block.oldStart }}-{{ block.newEnd || block.oldEnd }}</strong>
      </section>

      <section v-if="block.attribution?.warnings.length" class="detail-section">
        <span class="detail-kicker">归因可信度</span>
        <n-alert type="warning" :bordered="false">
          {{ block.attribution.warnings.map((warning) => warning.message).join('；') }}
        </n-alert>
      </section>

      <section class="detail-section">
        <span class="detail-kicker">相关提交</span>
        <n-empty v-if="block.relatedCommits.length === 0" size="small" description="暂无提交" />
        <ul v-else class="commit-list">
          <li v-for="commit in block.relatedCommits" :key="commitKey(commit)" class="commit-list__item">
            <div
              class="commit-row"
              :class="{ 'is-selected': commit.hash === selectedCommitHash }"
              :data-commit-hash="commit.hash"
              role="button"
              tabindex="0"
              @click="emit('commitSelected', commit)"
              @keydown.enter.prevent="emit('commitSelected', commit)"
              @keydown.space.prevent="emit('commitSelected', commit)"
            >
              <div class="commit-row__top">
                <code>{{ commit.shortHash }}</code>
                <div class="commit-row__top-actions">
                  <button
                    v-if="activeCommitHash === commit.hash"
                    class="commit-row__cancel"
                    type="button"
                    title="取消提交下钻"
                    aria-label="取消提交下钻"
                    @click.stop="emit('cancelCommit')"
                  >
                    <LoaderCircle class="commit-row__cancel-spinner spin-icon" :size="14" aria-hidden="true" />
                    <OctagonX class="commit-row__cancel-close" :size="14" aria-hidden="true" />
                  </button>
                  <n-tag v-if="commit.matchedByFilter" size="small" type="success" :bordered="false">命中筛选</n-tag>
                </div>
              </div>
              <p>{{ commit.subject }}</p>
              <span>{{ commit.authorName }} · {{ formatDate(commit.committedAt) }}</span>
              <small v-if="commit.attribution?.viaMergeHashes.length">Merge 链路：{{ commit.attribution.viaMergeHashes.map((hash) => hash.slice(0, 8)).join(' → ') }}</small>
            </div>
          </li>
        </ul>
      </section>
    </template>
  </aside>
</template>
