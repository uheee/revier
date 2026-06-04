import { createApp } from 'vue';
import naive from 'naive-ui';
import App from './App.vue';
import router from './router';
import { createPinia } from 'pinia';
import './styles.css';

createApp(App).use(createPinia()).use(router).use(naive).mount('#app');
