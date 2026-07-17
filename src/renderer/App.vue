<template>
  <n-config-provider
    :locale="zhCN"
    :date-locale="dateZhCN"
    :theme="effectiveTheme === 'dark' ? darkTheme : null"
    :theme-overrides="themeOverrides"
  >
    <n-message-provider>
      <div
        class="app-theme-root"
        :data-theme="effectiveTheme"
        :style="{ fontFamily }"
      >
        <RouterView />
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { darkTheme, dateZhCN, zhCN } from 'naive-ui';
import { RouterView } from 'vue-router';
import { useEditorSettings } from './composables/useEditorSettings';
import { toNaiveThemeOverrides } from './editor/editorTheme';

const { effectiveTheme, activeColors, fontFamily } = useEditorSettings();
const themeOverrides = computed(() => toNaiveThemeOverrides(activeColors.value, fontFamily.value));
</script>
