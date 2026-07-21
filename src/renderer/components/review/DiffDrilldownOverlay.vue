<script setup lang="ts">
import { ArrowLeft, LoaderCircle, OctagonX } from 'lucide-vue-next';
import type {
  EditorSettings,
  FileOverlay,
  RelatedCommit,
  TextEncoding
} from '../../generated/bindings';
import DiffViewer from './DiffViewer.vue';

defineProps<{
  overlay?: FileOverlay;
  loading?: boolean;
  settings: EditorSettings;
  themeName: 'revier-light' | 'revier-dark';
  requestedEncoding: TextEncoding;
  selectedCommitHash?: string;
  selectedCommit?: RelatedCommit;
  error?: string;
}>();

const emit = defineEmits<{
  close: [];
  cancel: [];
  draftChange: [draft: boolean];
  encodingChange: [encoding: TextEncoding];
}>();
</script>

<template>
  <section v-if="selectedCommitHash || overlay || loading" class="diff-drilldown">
    <header class="diff-drilldown__bar">
      <n-button text type="primary" @click="emit('close')">
        <ArrowLeft :size="16" aria-hidden="true" />
        <span>返回</span>
      </n-button>
      <div class="diff-drilldown__title">
        <code>{{ overlay?.commit?.shortHash ?? selectedCommit?.shortHash ?? selectedCommitHash?.slice(0, 8) ?? 'loading' }}</code>
        <span>{{ overlay?.commit?.subject ?? selectedCommit?.subject ?? '加载提交变更' }}</span>
      </div>
      <n-button v-if="loading" class="cancel-work-button" size="small" secondary type="warning" @click="emit('cancel')">
        <OctagonX :size="14" aria-hidden="true" />
        <LoaderCircle class="spin-icon" :size="14" aria-hidden="true" />
        <span>取消</span>
      </n-button>
    </header>
    <div v-if="error && !overlay" class="diff-drilldown__error">
      <n-alert title="提交变更加载失败" type="error" show-icon :closable="false">
        {{ error }}
      </n-alert>
    </div>
    <DiffViewer
      v-else
      :overlay="overlay"
      :loading="loading"
      :settings="settings"
      :theme-name="themeName"
      :requested-encoding="requestedEncoding"
      :context-key="`${overlay?.commit?.hash ?? ''}:${overlay?.parentHash ?? ''}`"
      @draft-change="emit('draftChange', $event)"
      @encoding-change="emit('encodingChange', $event)"
    />
  </section>
</template>
