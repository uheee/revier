<script setup lang="ts">
import type { AuthorSummary } from '../../../shared/reviewTypes';

defineProps<{
  authors: AuthorSummary[];
}>();
</script>

<template>
  <div class="diff-authors">
    <el-tag v-if="authors.length === 0" size="small" effect="plain">未知作者</el-tag>
    <el-tooltip
      v-for="author in authors"
      v-else
      :key="`${author.name}-${author.email ?? ''}`"
      :content="author.email || author.name"
      placement="top"
    >
      <el-tag size="small" effect="plain">{{ author.name }}</el-tag>
    </el-tooltip>
  </div>
</template>
