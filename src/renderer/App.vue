<template>
  <n-config-provider
    :locale="zhCN"
    :date-locale="dateZhCN"
    :theme="effectiveTheme === 'dark' ? darkTheme : null"
    :theme-overrides="themeOverrides"
  >
    <n-message-provider>
      <div class="app-theme-root" :data-theme="effectiveTheme">
        <n-alert
          v-if="snapshot.warning && warningVisible"
          class="editor-settings-warning"
          title="编辑器配置未能加载"
          type="warning"
          closable
          @close="warningVisible = false"
        >
          {{ snapshot.warning }}（配置：{{ snapshot.configPath }}）
        </n-alert>
        <RouterView />
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue';
import { darkTheme, dateZhCN, zhCN } from 'naive-ui';
import { RouterView } from 'vue-router';
import { useEditorSettings } from './composables/useEditorSettings';
import { toNaiveThemeOverrides } from './editor/editorTheme';

const { snapshot, effectiveTheme, activeColors, fontFamily } = useEditorSettings();
const warningVisible = ref(true);
const themeOverrides = computed(() => toNaiveThemeOverrides(activeColors.value, fontFamily.value));
</script>
