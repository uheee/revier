<template>
  <n-config-provider
    :locale="zhCN"
    :date-locale="dateZhCN"
    :theme="effectiveTheme === 'dark' ? darkTheme : null"
    :theme-overrides="themeOverrides"
  >
    <n-message-provider>
      <div class="app-theme-root" :data-theme="effectiveTheme">
        <div class="app-notification-center">
          <NotificationCenter />
        </div>
        <RouterView />
      </div>
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { computed } from 'vue';
import { darkTheme, dateZhCN, zhCN } from 'naive-ui';
import { RouterView } from 'vue-router';
import NotificationCenter from './components/NotificationCenter.vue';
import { useEditorSettings } from './composables/useEditorSettings';
import { toNaiveThemeOverrides } from './editor/editorTheme';

const { effectiveTheme, activeColors, fontFamily } = useEditorSettings();
const themeOverrides = computed(() => toNaiveThemeOverrides(activeColors.value, fontFamily.value));
</script>

<style scoped>
.app-notification-center {
  position: fixed;
  top: 12px;
  right: 16px;
  z-index: 1000;
}
</style>
