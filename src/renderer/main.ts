import { createApp } from 'vue';
import naive from 'naive-ui';
import App from './App.vue';
import router from './router';
import { createPinia } from 'pinia';
import { PiniaColada } from '@pinia/colada';
import './styles.css';
import {
  initializeEditorSettings,
  useEditorSettings
} from './composables/useEditorSettings';
import { addNotification } from './composables/useNotifications';
import { initializeRendererLogger, logError } from './api/logger';
import { initializeMonacoSyntax } from './editor/monacoEnvironment';
import {
  createRevierQueryHooksPlugin,
  revierQueryDefaults
} from './queries/queryClient';

function errorDetail(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function bootstrap(): Promise<void> {
  try {
    await initializeRendererLogger();
  } catch (error) {
    logError('前端日志桥接初始化失败', error, { source: 'bootstrap' });
  }

  await initializeEditorSettings();
  const { snapshot } = useEditorSettings();

  try {
    await initializeMonacoSyntax(snapshot.value.settings.themes);
  } catch (error) {
    logError('Monaco 语法高亮初始化失败', error, { source: 'Shiki' });
    addNotification({
      type: 'error',
      title: 'Monaco 语法高亮初始化失败',
      message: errorDetail(error),
      source: 'Shiki'
    });
  }

  const pinia = createPinia();
  createApp(App)
    .use(pinia)
    .use(PiniaColada, {
      queryOptions: revierQueryDefaults,
      plugins: [createRevierQueryHooksPlugin()]
    })
    .use(router)
    .use(naive)
    .mount('#app');
}

void bootstrap();
