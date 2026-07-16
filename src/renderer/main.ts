import { createApp } from 'vue';
import naive from 'naive-ui';
import App from './App.vue';
import router from './router';
import { createPinia } from 'pinia';
import './styles.css';
import {
  initializeEditorSettings,
  useEditorSettings
} from './composables/useEditorSettings';
import { addNotification } from './composables/useNotifications';
import { initializeMonacoSyntax } from './editor/monacoEnvironment';

function errorDetail(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function bootstrap(): Promise<void> {
  await initializeEditorSettings();
  const { snapshot } = useEditorSettings();

  try {
    await initializeMonacoSyntax(snapshot.value.settings.themes);
  } catch (error) {
    addNotification({
      type: 'error',
      title: 'Monaco 语法高亮初始化失败',
      message: errorDetail(error),
      source: 'Shiki'
    });
  }

  createApp(App).use(createPinia()).use(router).use(naive).mount('#app');
}

void bootstrap();
