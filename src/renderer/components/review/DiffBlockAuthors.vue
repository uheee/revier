<script setup lang="ts">
import type { AuthorSummary, BlockAttributionSummary } from '../../../shared/reviewTypes';

defineProps<{
  authors: AuthorSummary[];
  attribution?: BlockAttributionSummary;
}>();
</script>

<template>
  <div class="diff-authors">
    <n-tooltip v-if="attribution?.warnings.length" placement="top">
      <template #trigger>
        <n-tag size="small" type="warning" :bordered="false">!</n-tag>
      </template>
      {{ attribution.warnings.map((warning) => warning.message).join('；') }}
    </n-tooltip>
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
