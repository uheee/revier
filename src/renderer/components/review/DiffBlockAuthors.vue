<script setup lang="ts">
import type { AuthorSummary } from '../../../shared/reviewTypes';

defineProps<{
  authors: AuthorSummary[];
}>();
</script>

<template>
  <div class="diff-authors">
    <n-tag v-if="authors.length === 0" size="small" :bordered="false">未知作者</n-tag>
    <n-tooltip
      v-for="author in authors"
      v-else
      :key="`${author.name}-${author.email ?? ''}`"
      placement="top"
    >
      <template #trigger>
        <n-tag size="small" :bordered="false">{{ author.name }}</n-tag>
      </template>
      {{ author.email || author.name }}
    </n-tooltip>
  </div>
</template>
