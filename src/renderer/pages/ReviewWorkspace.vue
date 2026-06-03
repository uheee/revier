<script setup lang="ts">
import { computed } from 'vue';
import { useRoute, useRouter } from 'vue-router';

// TODO: Task 10 will replace this route target with the full review workspace.
const route = useRoute();
const router = useRouter();
const projectId = computed(() => String(route.params.projectId ?? ''));
</script>

<template>
  <main class="review-placeholder">
    <header class="review-placeholder__header">
      <el-button text @click="router.back()">返回</el-button>
      <h1>Review</h1>
      <span>{{ projectId }}</span>
    </header>
  </main>
</template>
