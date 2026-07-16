import { detectEditorLanguage, EDITOR_LANGUAGES } from '../../src/renderer/editor/editorLanguages';

const syntaxMocks = vi.hoisted(() => ({
  createHighlighter: vi.fn(),
  loadLanguage: vi.fn(),
  shikiToMonaco: vi.fn(),
  createJavaScriptRegexEngine: vi.fn(() => 'javascript-regex-engine'),
  workerKinds: [] as string[]
}));

vi.mock('shiki/core', () => ({
  createHighlighterCore: syntaxMocks.createHighlighter
}));
vi.mock('shiki/engine/javascript', () => ({
  createJavaScriptRegexEngine: syntaxMocks.createJavaScriptRegexEngine
}));

vi.mock('@shikijs/monaco', () => ({ shikiToMonaco: syntaxMocks.shikiToMonaco }));
vi.mock('monaco-editor', () => ({ languages: {}, editor: {} }));

vi.mock('monaco-editor/esm/vs/editor/editor.worker?worker', () => ({
  default: class { constructor() { syntaxMocks.workerKinds.push('editor'); } }
}));
vi.mock('monaco-editor/esm/vs/language/typescript/ts.worker?worker', () => ({
  default: class { constructor() { syntaxMocks.workerKinds.push('typescript'); } }
}));
vi.mock('monaco-editor/esm/vs/language/json/json.worker?worker', () => ({
  default: class { constructor() { syntaxMocks.workerKinds.push('json'); } }
}));
vi.mock('monaco-editor/esm/vs/language/css/css.worker?worker', () => ({
  default: class { constructor() { syntaxMocks.workerKinds.push('css'); } }
}));
vi.mock('monaco-editor/esm/vs/language/html/html.worker?worker', () => ({
  default: class { constructor() { syntaxMocks.workerKinds.push('html'); } }
}));

describe('编辑器语言注册表', () => {
  it('固定暴露已确认的 20 种语言及顺序', () => {
    expect(EDITOR_LANGUAGES.map((item) => item.id)).toEqual([
      'plaintext', 'javascript', 'typescript', 'vue', 'html', 'css', 'rust',
      'python', 'go', 'java', 'c', 'cpp', 'csharp', 'sql', 'markdown', 'json',
      'yaml', 'toml', 'shell', 'powershell'
    ]);
  });

  it.each([
    ['src/App.vue', 'vue'],
    ['Cargo.toml', 'toml'],
    ['MAKEFILE', 'shell'],
    ['scripts/build.PS1', 'powershell'],
    ['unknown.data', 'plaintext']
  ])('将 %s 识别为 %s', (path, language) => {
    expect(detectEditorLanguage(path)).toBe(language);
  });

  it('扩展名按最长项匹配', () => {
    expect(detectEditorLanguage('src/env.d.ts')).toBe('typescript');
  });
});

describe('Monaco 语法与 Worker 环境', () => {
  beforeEach(() => {
    vi.resetModules();
    syntaxMocks.createHighlighter.mockReset();
    syntaxMocks.loadLanguage.mockReset();
    syntaxMocks.shikiToMonaco.mockReset();
    syntaxMocks.workerKinds.length = 0;
    syntaxMocks.createHighlighter.mockResolvedValue({ loadLanguage: syntaxMocks.loadLanguage });
    syntaxMocks.loadLanguage.mockResolvedValue(undefined);
  });

  it('按 label 分派五类 Worker，未知 label 使用 editor Worker', async () => {
    await import('../../src/renderer/editor/monacoEnvironment');
    const getWorker = globalThis.MonacoEnvironment?.getWorker;

    getWorker?.('', 'typescript');
    getWorker?.('', 'javascript');
    getWorker?.('', 'json');
    getWorker?.('', 'scss');
    getWorker?.('', 'html');
    getWorker?.('', 'rust');

    expect(syntaxMocks.workerKinds).toEqual([
      'typescript', 'typescript', 'json', 'css', 'html', 'editor'
    ]);
  });

  it('只初始化一次 highlighter 和 Monaco 集成，并隔离单语言失败', async () => {
    syntaxMocks.loadLanguage.mockImplementation(async () => {
      if (syntaxMocks.loadLanguage.mock.calls.length === 3) {
        throw new Error('Vue grammar unavailable');
      }
    });
    const environment = await import('../../src/renderer/editor/monacoEnvironment');
    const notifications = await import('../../src/renderer/composables/useNotifications');
    const colors = {
      workspaceBackground: '#111111', panelBackground: '#222222', editorBackground: '#111111',
      border: '#333333', foreground: '#eeeeee', muted: '#999999', accent: '#00aaaa',
      selection: '#004444', diffRemoved: '#440000', diffRemovedStrong: '#aa0000',
      diffRemovedWord: '#660000', diffAdded: '#003300', diffAddedStrong: '#00aa00',
      diffAddedWord: '#006600', syntax: {
        comment: '#999999', keyword: '#cc99ff', string: '#99ddaa', number: '#ffbb77',
        type: '#88bbff', function: '#88bbff', variable: '#eeeeee'
      }
    };

    await Promise.all([
      environment.initializeMonacoSyntax({ light: colors, dark: colors }),
      environment.initializeMonacoSyntax({ light: colors, dark: colors })
    ]);

    expect(syntaxMocks.createHighlighter).toHaveBeenCalledTimes(1);
    expect(syntaxMocks.createHighlighter).toHaveBeenCalledWith(expect.objectContaining({
      langs: [],
      engine: 'javascript-regex-engine',
      themes: [expect.objectContaining({ name: 'revier-light' }), expect.objectContaining({ name: 'revier-dark' })]
    }));
    expect(syntaxMocks.loadLanguage).toHaveBeenCalledTimes(19);
    expect(syntaxMocks.shikiToMonaco).toHaveBeenCalledTimes(1);
    expect(environment.resolveInitializedLanguage('vue')).toBe('plaintext');
    expect(environment.resolveInitializedLanguage('rust')).toBe('rust');
    expect(notifications.useNotifications().notifications.value[0]).toMatchObject({
      type: 'warning',
      title: 'Vue 语法高亮加载失败',
      source: 'Shiki'
    });
  });
});
