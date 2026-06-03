<script setup lang="ts">
import { ArrowLeft } from 'lucide-vue-next';
import type { FileOverlay } from '../../../shared/reviewTypes';
import DiffViewer from './DiffViewer.vue';

defineProps<{
  overlay?: FileOverlay;
  loading?: boolean;
}>();

const emit = defineEmits<{
  close: [];
}>();
</script>

<template>
  <Transition name="drilldown-cover" mode="out-in">
    <section
      v-if="overlay || loading"
      :key="overlay?.commit?.hash ?? 'loading'"
      class="diff-drilldown"
    >
      <header class="diff-drilldown__bar">
        <el-button text type="primary" @click="emit('close')">
          <ArrowLeft :size="16" aria-hidden="true" />
          <span>返回</span>
        </el-button>
        <div class="diff-drilldown__title">
          <code>{{ overlay?.commit?.shortHash ?? 'loading' }}</code>
          <span>{{ overlay?.commit?.subject ?? '加载提交变更' }}</span>
        </div>
      </header>
      <DiffViewer :overlay="overlay" :loading="loading" />
    </section>
  </Transition>
</template>
