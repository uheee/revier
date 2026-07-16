// @vitest-environment jsdom
import type { DiffBlock, EditorSettings } from '../../src/renderer/generated/bindings';

const mocks = vi.hoisted(() => {
  const createModel = vi.fn();
  const createDiffEditor = vi.fn();
  const setModelLanguage = vi.fn();
  const setTheme = vi.fn();
  return { createModel, createDiffEditor, setModelLanguage, setTheme };
});

vi.mock('monaco-editor', () => ({
  editor: {
    createModel: mocks.createModel,
    createDiffEditor: mocks.createDiffEditor,
    setModelLanguage: mocks.setModelLanguage,
    setTheme: mocks.setTheme
  },
  Uri: {
    from: (parts: { scheme: string; authority: string; path: string }) => ({
      ...parts,
      toString: () => `${parts.scheme}://${parts.authority}${parts.path}`
    })
  },
  Range: class {
    constructor(
      public startLineNumber: number,
      public startColumn: number,
      public endLineNumber: number,
      public endColumn: number
    ) {}
  }
}));

vi.mock('../../src/renderer/editor/monacoEnvironment', () => ({
  resolveInitializedLanguage: (languageId: string) =>
    languageId === 'failed-language' ? 'plaintext' : languageId
}));

import { createMonacoDiffSession } from '../../src/renderer/editor/monacoDiffSession';

type Listener<T = void> = (event: T) => void;

function disposable() {
  return { dispose: vi.fn() };
}

function createModelMock() {
  const listeners: Listener[] = [];
  const listenerDisposables: ReturnType<typeof disposable>[] = [];
  const model = {
    dispose: vi.fn(),
    onDidChangeContent: vi.fn((listener: Listener) => {
      listeners.push(listener);
      const value = disposable();
      listenerDisposables.push(value);
      return value;
    })
  };
  return {
    model,
    listenerDisposables,
    change: () => listeners.forEach((listener) => listener())
  };
}

function createCodeEditorMock() {
  const mouseListeners: Listener<{ target: { position: { lineNumber: number } | null } }>[] = [];
  const cursorListeners: Listener<{ position: { lineNumber: number; column: number } }>[] = [];
  const listenerDisposables: ReturnType<typeof disposable>[] = [];
  const collections: Array<{
    set: ReturnType<typeof vi.fn>;
    clear: ReturnType<typeof vi.fn>;
  }> = [];
  const editor = {
    onMouseDown: vi.fn((listener: (typeof mouseListeners)[number]) => {
      mouseListeners.push(listener);
      const value = disposable();
      listenerDisposables.push(value);
      return value;
    }),
    onDidChangeCursorPosition: vi.fn((listener: (typeof cursorListeners)[number]) => {
      cursorListeners.push(listener);
      const value = disposable();
      listenerDisposables.push(value);
      return value;
    }),
    createDecorationsCollection: vi.fn(() => {
      const collection = { set: vi.fn(), clear: vi.fn() };
      collections.push(collection);
      return collection;
    })
  };
  return {
    editor,
    collections,
    listenerDisposables,
    mouse: (lineNumber: number) =>
      mouseListeners.forEach((listener) => listener({ target: { position: { lineNumber } } })),
    cursor: (lineNumber: number, column: number) =>
      cursorListeners.forEach((listener) => listener({ position: { lineNumber, column } }))
  };
}

const settings: EditorSettings = {
  version: 1,
  theme: 'system',
  defaultEncoding: 'auto',
  editor: {
    fontFamilies: ['JetBrainsMono Nerd Font Mono', 'Microsoft YaHei', 'monospace'],
    fontSize: 14,
    lineHeight: 24,
    minimap: true
  },
  largeFile: { maxBytes: 1_048_576, maxLines: 5_000 },
  themes: {} as EditorSettings['themes']
};

const blocks: DiffBlock[] = [
  {
    id: 'modified', oldStart: 2, oldEnd: 4, newStart: 3, newEnd: 5,
    changeType: 'modified', authors: [], rows: [], relatedCommits: []
  },
  {
    id: 'added', oldStart: 0, oldEnd: 0, newStart: 8, newEnd: 9,
    changeType: 'added', authors: [], rows: [], relatedCommits: []
  },
  {
    id: 'deleted', oldStart: 11, oldEnd: 12, newStart: 0, newEnd: 0,
    changeType: 'deleted', authors: [], rows: [], relatedCommits: []
  }
];

function setup() {
  const originalModel = createModelMock();
  const modifiedModel = createModelMock();
  const original = createCodeEditorMock();
  const modified = createCodeEditorMock();
  const diffEditor = {
    setModel: vi.fn(),
    getOriginalEditor: vi.fn(() => original.editor),
    getModifiedEditor: vi.fn(() => modified.editor),
    layout: vi.fn(),
    dispose: vi.fn()
  };
  mocks.createModel
    .mockReturnValueOnce(originalModel.model)
    .mockReturnValueOnce(modifiedModel.model);
  mocks.createDiffEditor.mockReturnValue(diffEditor);
  const callbacks = {
    onDraftChange: vi.fn(),
    onBlockSelected: vi.fn(),
    onCursorChange: vi.fn(),
    onEditorsReady: vi.fn()
  };
  const session = createMonacoDiffSession({
    container: document.createElement('div'),
    path: 'src/组件/app.test.ts',
    oldContent: 'old',
    newContent: 'new',
    languageId: 'typescript',
    settings,
    themeName: 'revier-dark',
    blocks,
    ...callbacks
  });
  return { session, originalModel, modifiedModel, original, modified, diffEditor, callbacks };
}

beforeEach(() => vi.clearAllMocks());

describe('createMonacoDiffSession', () => {
  it('用唯一且保留扩展名的 URI 创建并完整绑定可编辑 Diff', () => {
    const result = setup();
    const modelCalls = mocks.createModel.mock.calls;
    expect(modelCalls).toHaveLength(2);
    expect(modelCalls[0][0]).toBe('old');
    expect(modelCalls[1][0]).toBe('new');
    expect(modelCalls[0][1]).toBe('typescript');
    expect(modelCalls[0][2].toString()).toMatch(/original\/src\/.*app\.test\.ts$/);
    expect(modelCalls[1][2].toString()).toMatch(/modified\/src\/.*app\.test\.ts$/);
    expect(modelCalls[0][2].toString()).not.toBe(modelCalls[1][2].toString());
    expect(mocks.createDiffEditor).toHaveBeenCalledWith(expect.any(HTMLElement), expect.objectContaining({
      originalEditable: true,
      readOnly: false,
      renderSideBySide: true,
      fontFamily: '"JetBrainsMono Nerd Font Mono", "Microsoft YaHei", monospace',
      fontSize: 14,
      lineHeight: 24,
      minimap: { enabled: true },
      folding: true,
      matchBrackets: 'always',
      largeFileOptimizations: true,
      contextmenu: true,
      find: expect.objectContaining({ addExtraSpaceOnTop: false })
    }));
    expect(result.diffEditor.setModel).toHaveBeenCalledWith({
      original: result.originalModel.model,
      modified: result.modifiedModel.model
    });
    expect(result.callbacks.onEditorsReady).toHaveBeenCalledWith(
      result.original.editor,
      result.modified.editor
    );
  });

  it('相同路径的并存会话仍为四个 Model 生成全局唯一 URI', () => {
    const first = setup();
    const firstUris = mocks.createModel.mock.calls.map((call) => call[2].toString());
    const second = setup();
    const allUris = mocks.createModel.mock.calls.map((call) => call[2].toString());

    expect(new Set(allUris).size).toBe(4);
    expect(firstUris.every((uri) => uri.endsWith('/src/组件/app.test.ts'))).toBe(true);
    expect(allUris.slice(2).every((uri) => uri.endsWith('/src/组件/app.test.ts'))).toBe(true);
    first.session.dispose();
    second.session.dispose();
  });

  it('首次编辑只进入一次草稿并清除装饰、禁用块命中', () => {
    const { originalModel, modifiedModel, original, modified, callbacks } = setup();
    original.mouse(2);
    expect(callbacks.onBlockSelected).toHaveBeenLastCalledWith(blocks[0]);

    modifiedModel.change();
    originalModel.change();
    modifiedModel.change();

    expect(callbacks.onDraftChange).toHaveBeenCalledTimes(1);
    expect(callbacks.onDraftChange).toHaveBeenCalledWith(true);
    expect(original.collections[0].clear).toHaveBeenCalledTimes(1);
    expect(modified.collections[0].clear).toHaveBeenCalledTimes(1);
    callbacks.onBlockSelected.mockClear();
    original.mouse(2);
    modified.mouse(3);
    expect(callbacks.onBlockSelected).not.toHaveBeenCalled();
  });

  it('按左右真实非零范围命中块，并精确转发光标', () => {
    const { original, modified, callbacks } = setup();
    original.mouse(3);
    modified.mouse(8);
    original.mouse(8);
    modified.mouse(11);
    expect(callbacks.onBlockSelected.mock.calls.map(([block]) => block.id)).toEqual([
      'modified', 'added'
    ]);
    modified.cursor(12, 7);
    expect(callbacks.onCursorChange).toHaveBeenCalledWith(12, 7);
  });

  it('更新语言、主题和选中块时不重建模型', () => {
    const { session, originalModel, modifiedModel, original, modified } = setup();
    mocks.createModel.mockClear();
    session.setLanguage('failed-language');
    expect(mocks.setModelLanguage).toHaveBeenNthCalledWith(1, originalModel.model, 'plaintext');
    expect(mocks.setModelLanguage).toHaveBeenNthCalledWith(2, modifiedModel.model, 'plaintext');
    session.setTheme('revier-light');
    expect(mocks.setTheme).toHaveBeenCalledWith('revier-light');
    expect(mocks.createModel).not.toHaveBeenCalled();

    session.setSelectedBlock(blocks[0]);
    const oldDecorations = original.collections[0].set.mock.calls[0][0];
    const newDecorations = modified.collections[0].set.mock.calls[0][0];
    expect(oldDecorations).toHaveLength(3);
    expect(oldDecorations[0].options.className).toContain('old');
    expect(oldDecorations[1].options.className).toContain('top');
    expect(oldDecorations[2].options.className).toContain('bottom');
    expect(newDecorations).toHaveLength(3);
    expect(newDecorations[0].options.className).toContain('new');
    session.setSelectedBlock(undefined);
    expect(original.collections[0].clear).toHaveBeenCalled();
    expect(modified.collections[0].clear).toHaveBeenCalled();
  });

  it('layout 可用且 dispose 幂等释放全部资源各一次', () => {
    const result = setup();
    result.session.layout();
    expect(result.diffEditor.layout).toHaveBeenCalledTimes(1);
    result.session.dispose();
    result.session.dispose();
    expect(result.diffEditor.dispose).toHaveBeenCalledTimes(1);
    expect(result.originalModel.model.dispose).toHaveBeenCalledTimes(1);
    expect(result.modifiedModel.model.dispose).toHaveBeenCalledTimes(1);
    expect(result.original.collections[0].clear).toHaveBeenCalledTimes(1);
    expect(result.modified.collections[0].clear).toHaveBeenCalledTimes(1);
    for (const listener of [
      ...result.originalModel.listenerDisposables,
      ...result.modifiedModel.listenerDisposables,
      ...result.original.listenerDisposables,
      ...result.modified.listenerDisposables
    ]) {
      expect(listener.dispose).toHaveBeenCalledTimes(1);
    }
  });

  it('清理项抛错时仍继续释放后续资源并保持幂等', () => {
    const result = setup();
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    result.originalModel.listenerDisposables[0].dispose.mockImplementation(() => {
      throw new Error('监听器清理失败');
    });
    result.original.collections[0].clear.mockImplementation(() => {
      throw new Error('装饰清理失败');
    });
    result.diffEditor.dispose.mockImplementation(() => {
      throw new Error('编辑器清理失败');
    });

    expect(() => result.session.dispose()).not.toThrow();
    result.session.dispose();

    expect(result.modified.collections[0].clear).toHaveBeenCalledTimes(1);
    expect(result.originalModel.model.dispose).toHaveBeenCalledTimes(1);
    expect(result.modifiedModel.model.dispose).toHaveBeenCalledTimes(1);
    expect(consoleError).toHaveBeenCalledTimes(1);
    expect(consoleError.mock.calls[0][1]).toBeInstanceOf(AggregateError);
    consoleError.mockRestore();
  });

  it('销毁后所有公开操作都安全 no-op', () => {
    const result = setup();
    result.session.dispose();
    vi.clearAllMocks();

    expect(() => {
      result.session.setLanguage('rust');
      result.session.setTheme('revier-light');
      result.session.setSelectedBlock(blocks[0]);
      result.session.layout();
    }).not.toThrow();
    expect(mocks.setModelLanguage).not.toHaveBeenCalled();
    expect(mocks.setTheme).not.toHaveBeenCalled();
    expect(result.original.collections[0].set).not.toHaveBeenCalled();
    expect(result.original.collections[0].clear).not.toHaveBeenCalled();
    expect(result.diffEditor.layout).not.toHaveBeenCalled();
  });

  it('创建中异常时释放已经创建的资源', () => {
    const originalModel = createModelMock();
    const modifiedModel = createModelMock();
    const diffEditor = {
      setModel: vi.fn(),
      dispose: vi.fn(),
      getOriginalEditor: vi.fn(() => { throw new Error('失败'); })
    };
    mocks.createModel
      .mockReturnValueOnce(originalModel.model)
      .mockReturnValueOnce(modifiedModel.model);
    mocks.createDiffEditor.mockReturnValue(diffEditor);
    expect(() => createMonacoDiffSession({
      container: document.createElement('div'), path: 'x.ts', oldContent: '', newContent: '',
      languageId: 'typescript', settings, themeName: 'revier-light', blocks,
      onDraftChange: vi.fn(), onBlockSelected: vi.fn(), onCursorChange: vi.fn(),
      onEditorsReady: vi.fn()
    })).toThrow('失败');
    expect(diffEditor.dispose).toHaveBeenCalledTimes(1);
    expect(originalModel.model.dispose).toHaveBeenCalledTimes(1);
    expect(modifiedModel.model.dispose).toHaveBeenCalledTimes(1);
  });

  it('创建错误不被清理错误覆盖，且继续清理所有 Model', () => {
    const originalModel = createModelMock();
    const modifiedModel = createModelMock();
    const createError = new Error('原始创建错误');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    originalModel.model.dispose.mockImplementation(() => {
      throw new Error('原始 Model 清理失败');
    });
    const diffEditor = {
      setModel: vi.fn(),
      dispose: vi.fn(() => { throw new Error('编辑器清理失败'); }),
      getOriginalEditor: vi.fn(() => { throw createError; })
    };
    mocks.createModel
      .mockReturnValueOnce(originalModel.model)
      .mockReturnValueOnce(modifiedModel.model);
    mocks.createDiffEditor.mockReturnValue(diffEditor);

    let thrown: unknown;
    try {
      createMonacoDiffSession({
        container: document.createElement('div'), path: 'x.ts', oldContent: '', newContent: '',
        languageId: 'typescript', settings, themeName: 'revier-light', blocks,
        onDraftChange: vi.fn(), onBlockSelected: vi.fn(), onCursorChange: vi.fn(),
        onEditorsReady: vi.fn()
      });
    } catch (error) {
      thrown = error;
    }
    expect(thrown).toBe(createError);
    expect(modifiedModel.model.dispose).toHaveBeenCalledTimes(1);
    expect(consoleError).toHaveBeenCalledTimes(1);
    consoleError.mockRestore();
  });
});
