import { createRouter, createWebHashHistory } from 'vue-router';
import ProjectHome from './pages/ProjectHome.vue';

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/',
      name: 'projects',
      component: ProjectHome
    },
    {
      path: '/review/:projectId',
      name: 'review',
      component: () => import('./pages/ReviewWorkspace.vue')
    }
  ]
});

export default router;
