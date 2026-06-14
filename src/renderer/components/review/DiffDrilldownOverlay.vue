<script setup lang="ts">
import { ArrowLeft, LoaderCircle, OctagonX } from 'lucide-vue-next';
import type { FileOverlay } from '../../../shared/reviewTypes';
import DiffViewer from './DiffViewer.vue';

defineProps<{
  overlay?: FileOverlay;
  loading?: boolean;
}>();

const emit = defineEmits<{
  close: [];
  cancel: [];
}>();
</script>

<template>
  <section v-if="overlay || loading" class="diff-drilldown">
    <header class="diff-drilldown__bar">
      <n-button text type="primary" @click="emit('close')">
        <ArrowLeft :size="16" aria-hidden="true" />
        <span>返回</span>
      </n-button>
      <div class="diff-drilldown__title">
        <code>{{ overlay?.commit?.shortHash ?? 'loading' }}</code>
        <span>{{ overlay?.commit?.subject ?? '加载提交变更' }}</span>
      </div>
      <n-button v-if="loading" class="cancel-work-button" size="small" secondary type="warning" @click="emit('cancel')">
        <OctagonX :size="14" aria-hidden="true" />
        <LoaderCircle class="spin-icon" :size="14" aria-hidden="true" />
        <span>取消</span>
      </n-button>
    </header>
    <DiffViewer :overlay="overlay" :loading="loading" />
  </section>
</template>
