import { mount } from '@vue/test-utils';
import { ref } from 'vue';
import App from '../../src/renderer/App.vue';

vi.mock('../../src/renderer/composables/useEditorSettings', () => ({
  useEditorSettings: () => ({
    snapshot: ref({ warning: '不应显示在横幅中' }),
    effectiveTheme: ref('light'),
    activeColors: ref({}),
    fontFamily: ref('monospace')
  })
}));
vi.mock('../../src/renderer/editor/editorTheme', () => ({
  toNaiveThemeOverrides: () => ({})
}));

describe('应用通知中心挂载', () => {
  it('真实渲染通知中心入口且不再渲染配置 warning 横幅', () => {
    const wrapper = mount(App, {
      global: {
        stubs: {
          NConfigProvider: { template: '<div><slot /></div>' },
          NMessageProvider: { template: '<div><slot /></div>' },
          RouterView: { template: '<main data-test="router-view" />' },
          NotificationCenter: {
            name: 'NotificationCenter',
            template: '<button data-test="notification-center">通知</button>'
          }
        }
      }
    });

    expect(wrapper.find('[data-test="notification-center"]').exists()).toBe(true);
    expect(wrapper.find('.app-notification-center').exists()).toBe(true);
    expect(wrapper.find('.editor-settings-warning').exists()).toBe(false);
    expect(wrapper.text()).not.toContain('不应显示在横幅中');
  });
});
