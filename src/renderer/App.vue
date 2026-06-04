<template>
  <n-config-provider :locale="zhCN" :date-locale="dateZhCN" :theme-overrides="themeOverrides">
    <n-message-provider>
      <RouterView />
    </n-message-provider>
  </n-config-provider>
</template>

<script setup lang="ts">
import { dateZhCN, zhCN, type GlobalThemeOverrides } from 'naive-ui';
import { RouterView } from 'vue-router';

const themeOverrides: GlobalThemeOverrides = {
  common: {
    primaryColor: '#0f766e',
    primaryColorHover: '#115e59',
    primaryColorPressed: '#134e4a',
    primaryColorSuppl: '#0f766e',
    successColor: '#1f8a4c',
    successColorHover: '#187641',
    successColorPressed: '#135f34',
    errorColor: '#d93025',
    errorColorHover: '#b8271f',
    errorColorPressed: '#9f1d18',
    bodyColor: '#f3f5f7',
    cardColor: '#ffffff',
    tableColor: '#ffffff',
    tableHeaderColor: '#f8fafc',
    hoverColor: '#f1f5f9',
    borderColor: '#d8dee8',
    dividerColor: '#edf0f4',
    textColorBase: '#172033',
    textColor1: '#172033',
    textColor2: '#4b5563',
    textColor3: '#64748b',
    placeholderColor: '#9ca3af',
    fontFamily:
      'Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif',
    fontFamilyMono: '"JetBrains Mono", "SFMono-Regular", Consolas, "Liberation Mono", monospace',
    borderRadius: '6px',
    borderRadiusSmall: '4px',
    heightMedium: '34px',
    heightSmall: '28px',
    fontSize: '13px',
    fontSizeMedium: '13px',
    fontSizeSmall: '12px'
  }
};
</script>
