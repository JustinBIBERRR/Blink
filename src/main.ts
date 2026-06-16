import { createApp } from 'vue';
import { createRouter, createWebHashHistory } from 'vue-router';
import './assets/styles/tokens.css';

const router = createRouter({
  history: createWebHashHistory(),
  routes: [
    {
      path: '/overlay',
      component: () => import('./windows/overlay/OverlayApp.vue'),
    },
    {
      path: '/stats',
      component: () => import('./windows/stats/StatsApp.vue'),
    },
    {
      path: '/settings',
      component: () => import('./windows/settings/SettingsApp.vue'),
    },
    {
      path: '/',
      redirect: '/stats',
    },
  ],
});

const app = createApp({ template: '<router-view />' });
app.use(router);
app.mount('#app');
