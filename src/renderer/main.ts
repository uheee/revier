import { createApp } from 'vue';
import naive from 'naive-ui';
import App from './App.vue';
import router from './router';
import { createPinia } from 'pinia';
import './styles.css';
import { initializeEditorSettings } from './composables/useEditorSettings';

async function bootstrap(): Promise<void> {
  try {
    await initializeEditorSettings();
  } catch (error) {
    console.error('初始化编辑器设置失败，应用将继续启动。', error);
  }

  createApp(App).use(createPinia()).use(router).use(naive).mount('#app');
}

void bootstrap();
