import { ref } from 'vue';

const harness = vi.hoisted(() => ({
  order: [] as string[],
  initializeEditorSettings: vi.fn(),
  initializeMonacoSyntax: vi.fn(),
  addNotification: vi.fn(),
  mount: vi.fn(),
  resolveSettings: undefined as (() => void) | undefined,
  themes: { light: {}, dark: {} }
}));

vi.mock('../../src/renderer/App.vue', () => ({ default: {} }));
vi.mock('../../src/renderer/router', () => ({ default: {} }));
vi.mock('pinia', () => ({ createPinia: vi.fn(() => ({})) }));
vi.mock('naive-ui', () => ({ default: {} }));
vi.mock('../../src/renderer/composables/useEditorSettings', () => ({
  initializeEditorSettings: harness.initializeEditorSettings,
  useEditorSettings: () => ({ snapshot: ref({ settings: { themes: harness.themes } }) })
}));
vi.mock('../../src/renderer/editor/monacoEnvironment', () => ({
  initializeMonacoSyntax: harness.initializeMonacoSyntax
}));
vi.mock('../../src/renderer/composables/useNotifications', () => ({
  addNotification: harness.addNotification
}));
vi.mock('vue', async (importOriginal) => {
  const actual = await importOriginal<typeof import('vue')>();
  return {
    ...actual,
    createApp: vi.fn(() => {
      const app = {
        use: vi.fn(() => app),
        mount: harness.mount
      };
      return app;
    })
  };
});

describe('渲染器启动流程', () => {
  beforeEach(() => {
    vi.resetModules();
    harness.order.length = 0;
    harness.resolveSettings = undefined;
    harness.initializeEditorSettings.mockReset().mockImplementation(() => {
      harness.order.push('settings:start');
      return new Promise<void>((resolve) => {
        harness.resolveSettings = () => {
          harness.order.push('settings:done');
          resolve();
        };
      });
    });
    harness.initializeMonacoSyntax.mockReset().mockImplementation(async () => {
      harness.order.push('syntax');
    });
    harness.addNotification.mockReset();
    harness.mount.mockReset().mockImplementation(() => {
      harness.order.push('mount');
    });
  });

  it('严格按设置、语法、挂载顺序启动', async () => {
    await import('../../src/renderer/main');
    expect(harness.order).toEqual(['settings:start']);
    expect(harness.initializeMonacoSyntax).not.toHaveBeenCalled();
    expect(harness.mount).not.toHaveBeenCalled();

    harness.resolveSettings?.();
    await vi.waitFor(() => expect(harness.mount).toHaveBeenCalledWith('#app'));

    expect(harness.order).toEqual(['settings:start', 'settings:done', 'syntax', 'mount']);
    expect(harness.initializeMonacoSyntax).toHaveBeenCalledWith(harness.themes);
  });

  it('语法初始化失败时发布错误通知并继续挂载', async () => {
    harness.initializeMonacoSyntax.mockImplementationOnce(async () => {
      harness.order.push('syntax');
      throw new Error('syntax unavailable');
    });

    await import('../../src/renderer/main');
    expect(harness.initializeMonacoSyntax).not.toHaveBeenCalled();
    harness.resolveSettings?.();
    await vi.waitFor(() => expect(harness.mount).toHaveBeenCalledWith('#app'));

    expect(harness.order).toEqual(['settings:start', 'settings:done', 'syntax', 'mount']);
    expect(harness.addNotification).toHaveBeenCalledWith({
      type: 'error',
      title: 'Monaco 语法高亮初始化失败',
      message: 'syntax unavailable',
      source: 'Shiki'
    });
  });
});
