import type { EditorSettingsSnapshot } from '../../src/renderer/generated/bindings';

const invoke = vi.fn();
const defineTheme = vi.fn();
const setTheme = vi.fn();

vi.mock('@tauri-apps/api/core', () => ({ invoke }));
vi.mock('monaco-editor', () => ({
  editor: { defineTheme, setTheme }
}));

function settingsSnapshot(theme: 'system' | 'light' | 'dark' = 'system'): EditorSettingsSnapshot {
  const colors = {
    workspaceBackground: '#F4F6F8',
    panelBackground: '#FFFFFF',
    editorBackground: '#FCFDFE',
    border: '#DFE5EC',
    foreground: '#273448',
    muted: '#768296',
    accent: '#0F766E',
    selection: '#DCEFEB',
    diffRemoved: '#FBE7E5',
    diffRemovedStrong: '#BC3D35',
    diffRemovedWord: '#F1B9B3',
    diffAdded: '#E2F3E8',
    diffAddedStrong: '#26804A',
    diffAddedWord: '#A9DBBB',
    syntax: {
      comment: '#768296',
      keyword: '#893CAD',
      string: '#0B7952',
      number: '#A05B00',
      type: '#0969DA',
      function: '#1C63A5',
      variable: '#273448'
    }
  };
  return {
    configPath: 'C:/config/editor.toml',
    settings: {
      version: 1,
      theme,
      defaultEncoding: 'auto',
      editor: {
        fontFamilies: ['JetBrainsMono Nerd Font Mono', 'Microsoft YaHei', 'monospace'],
        fontSize: 13,
        lineHeight: 22,
        minimap: true
      },
      largeFile: { maxBytes: 1_048_576, maxLines: 5_000 },
      themes: { light: colors, dark: { ...colors, workspaceBackground: '#111821' } }
    }
  };
}

type MatchMediaHarness = {
  query: MediaQueryList;
  listener?: (event: MediaQueryListEvent) => void;
  add: ReturnType<typeof vi.fn>;
  remove: ReturnType<typeof vi.fn>;
};

function installMatchMedia(matches = false): MatchMediaHarness {
  const harness: MatchMediaHarness = {
    add: vi.fn(),
    remove: vi.fn(),
    query: undefined as unknown as MediaQueryList
  };
  harness.add.mockImplementation((_type, listener) => {
    harness.listener = listener;
  });
  harness.query = {
    matches,
    media: '(prefers-color-scheme: dark)',
    onchange: null,
    addEventListener: harness.add,
    removeEventListener: harness.remove,
    addListener: vi.fn(),
    removeListener: vi.fn(),
    dispatchEvent: vi.fn()
  };
  vi.stubGlobal('matchMedia', vi.fn(() => harness.query));
  return harness;
}

async function loadComposable() {
  return import('../../src/renderer/composables/useEditorSettings');
}

describe('编辑器设置启动初始化', () => {
  beforeEach(() => {
    vi.resetModules();
    invoke.mockReset();
    defineTheme.mockReset();
    setTheme.mockReset();
    document.documentElement.removeAttribute('style');
    document.documentElement.removeAttribute('data-theme');
  });

  afterEach(() => vi.unstubAllGlobals());

  it('并发初始化只读取一次 IPC，并在设置完成后应用主题', async () => {
    installMatchMedia(true);
    invoke.mockResolvedValue(settingsSnapshot());
    const module = await loadComposable();

    await Promise.all([module.initializeEditorSettings(), module.initializeEditorSettings()]);

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(module.useEditorSettings().effectiveTheme.value).toBe('dark');
    expect(document.documentElement.dataset.theme).toBe('dark');
    expect(setTheme).toHaveBeenCalledWith('revier-dark');
  });

  it('IPC 失败仍完成启动并返回完整默认值和 warning', async () => {
    installMatchMedia(false);
    invoke.mockRejectedValue(new Error('IPC unavailable'));
    const module = await loadComposable();

    await expect(module.initializeEditorSettings()).resolves.toBeUndefined();

    const state = module.useEditorSettings();
    expect(state.snapshot.value.settings.editor.fontFamilies).toEqual([
      'JetBrainsMono Nerd Font Mono',
      'Microsoft YaHei',
      'monospace'
    ]);
    expect(state.snapshot.value.warning).toContain('IPC unavailable');
    const notifications = await import('../../src/renderer/composables/useNotifications');
    expect(notifications.useNotifications().notifications.value[0]).toMatchObject({
      type: 'warning',
      title: '编辑器配置未能加载',
      source: 'editor.toml'
    });
  });

  it('设置快照中的 warning 会连同配置路径发布到通知中心', async () => {
    installMatchMedia(false);
    invoke.mockResolvedValue({ ...settingsSnapshot(), warning: '颜色字段无效' });
    const module = await loadComposable();
    await module.initializeEditorSettings();

    const notifications = await import('../../src/renderer/composables/useNotifications');
    expect(notifications.useNotifications().notifications.value[0]).toMatchObject({
      type: 'warning',
      title: '编辑器配置未能加载',
      message: '颜色字段无效（配置：C:/config/editor.toml）'
    });
  });

  it('system 变化只重新应用主题而不重读配置', async () => {
    const media = installMatchMedia(false);
    invoke.mockResolvedValue(settingsSnapshot('system'));
    const module = await loadComposable();
    await module.initializeEditorSettings();

    media.listener?.({ matches: true } as MediaQueryListEvent);
    await vi.waitFor(() => expect(setTheme).toHaveBeenLastCalledWith('revier-dark'));

    expect(invoke).toHaveBeenCalledTimes(1);
    expect(module.useEditorSettings().effectiveTheme.value).toBe('dark');
  });

  it.each(['light', 'dark'] as const)('%s 模式不注册也不响应系统主题变化', async (theme) => {
    const media = installMatchMedia(theme === 'light');
    invoke.mockResolvedValue(settingsSnapshot(theme));
    const module = await loadComposable();
    await module.initializeEditorSettings();

    expect(media.add).not.toHaveBeenCalled();
    expect(module.useEditorSettings().effectiveTheme.value).toBe(theme);
  });

  it('Monaco 主题应用失败不会拒绝启动并发布错误通知', async () => {
    installMatchMedia(false);
    invoke.mockResolvedValue(settingsSnapshot());
    defineTheme.mockImplementationOnce(() => {
      throw new Error('Monaco unavailable');
    });
    const module = await loadComposable();

    await expect(module.initializeEditorSettings()).resolves.toBeUndefined();

    const notifications = await import('../../src/renderer/composables/useNotifications');
    expect(notifications.useNotifications().notifications.value[0]).toMatchObject({
      type: 'error',
      title: '应用 Monaco 主题失败'
    });
    expect(notifications.useNotifications().notifications.value[0]?.message).toContain('Monaco unavailable');
    expect(document.documentElement.dataset.theme).toBe('light');
  });

  it('matchMedia 失败仍完成启动并发布非阻断警告', async () => {
    vi.stubGlobal('matchMedia', vi.fn(() => {
      throw new Error('media unavailable');
    }));
    invoke.mockResolvedValue(settingsSnapshot());
    const module = await loadComposable();

    await expect(module.initializeEditorSettings()).resolves.toBeUndefined();

    const notifications = await import('../../src/renderer/composables/useNotifications');
    expect(notifications.useNotifications().notifications.value[0]?.message).toContain('media unavailable');
  });

  it('CSS 应用失败仍完成启动并发布错误通知', async () => {
    installMatchMedia(false);
    invoke.mockResolvedValue(settingsSnapshot());
    const setProperty = vi
      .spyOn(document.documentElement.style, 'setProperty')
      .mockImplementationOnce(() => {
        throw new Error('CSS unavailable');
      });
    const module = await loadComposable();

    await expect(module.initializeEditorSettings()).resolves.toBeUndefined();

    const notifications = await import('../../src/renderer/composables/useNotifications');
    expect(notifications.useNotifications().notifications.value[0]).toMatchObject({
      type: 'error',
      title: '应用页面主题失败'
    });
    expect(notifications.useNotifications().notifications.value[0]?.message).toContain('CSS unavailable');
    setProperty.mockRestore();
  });

});
