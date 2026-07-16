import { mount } from '@vue/test-utils';
import { nextTick } from 'vue';
import naive from 'naive-ui';
import NotificationCenter from '../../src/renderer/components/NotificationCenter.vue';
import {
  addNotification,
  useNotifications
} from '../../src/renderer/composables/useNotifications';

async function openCenter(wrapper: ReturnType<typeof mount>) {
  await wrapper.get('[aria-label="通知中心"]').trigger('click');
  await nextTick();
  await new Promise((resolve) => setTimeout(resolve, 0));
}

describe('通知中心', () => {
  beforeEach(() => useNotifications().clear());

  it('铃铛始终显示且空列表可以打开', async () => {
    const wrapper = mount(NotificationCenter, { attachTo: document.body, global: { plugins: [naive] } });

    expect(wrapper.find('[aria-label="通知中心"]').exists()).toBe(true);
    expect(wrapper.find('.notification-center__badge .n-badge-sup').exists()).toBe(false);
    await openCenter(wrapper);
    expect(document.body.textContent).toContain('暂无通知');

    wrapper.unmount();
  });

  it('在按钮右下角显示红色未读数量并在打开后标记已读', async () => {
    for (let index = 0; index < 100; index += 1) {
      addNotification({ type: 'info', title: `消息 ${index}` });
    }
    const wrapper = mount(NotificationCenter, { attachTo: document.body, global: { plugins: [naive] } });

    const badge = wrapper.get('.notification-center__badge');
    expect(badge.classes()).toContain('n-badge');
    expect(badge.text()).toContain('99+');
    expect(badge.attributes('aria-label')).toBe('100 条未读通知');
    await openCenter(wrapper);
    expect(useNotifications().unreadCount.value).toBe(0);
    expect(wrapper.find('.notification-center__badge .n-badge-sup').exists()).toBe(false);

    wrapper.unmount();
  });

  it('列表可滚动，并支持删除单条与清空全部', async () => {
    addNotification({ type: 'warning', title: '配置警告', message: '字段无效' });
    addNotification({ type: 'error', title: '运行错误', source: 'Shiki' });
    const wrapper = mount(NotificationCenter, { attachTo: document.body, global: { plugins: [naive] } });
    await openCenter(wrapper);

    const panel = document.body.querySelector('.notification-center__list');
    expect(panel).not.toBeNull();
    expect(panel?.classList.contains('notification-center__list--scrollable')).toBe(true);
    const removeButton = document.body.querySelector<HTMLButtonElement>('[aria-label="删除通知：运行错误"]');
    removeButton?.click();
    await nextTick();
    expect(useNotifications().notifications.value.map((item) => item.title)).toEqual(['配置警告']);

    document.body.querySelector<HTMLButtonElement>('[aria-label="清空全部通知"]')?.click();
    await nextTick();
    expect(useNotifications().notifications.value).toEqual([]);

    wrapper.unmount();
  });
});
