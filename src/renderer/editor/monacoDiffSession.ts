import * as monaco from 'monaco-editor';
import type { editor } from 'monaco-editor';
import type { DiffBlock, EditorSettings } from '../generated/bindings';
import { toFontFamily } from './editorTheme';
import { resolveInitializedLanguage } from './monacoEnvironment';
import {
  diffBlocksSignature,
  normalizeMonacoDiffBlocks,
  type MonacoDiffBlocksPayload
} from './monacoDiffBlocks';

export interface MonacoDiffSession {
  diffEditor: editor.IStandaloneDiffEditor;
  originalEditor: editor.ICodeEditor;
  modifiedEditor: editor.ICodeEditor;
  setLanguage(languageId: string): void;
  setTheme(themeName: 'revier-light' | 'revier-dark'): void;
  setBlocks(blocks: DiffBlock[]): void;
  setSelectedBlock(block?: DiffBlock): void;
  layout(): void;
  dispose(): void;
}

export interface MonacoDiffSessionOptions {
  container: HTMLElement;
  path: string;
  oldContent: string;
  newContent: string;
  languageId: string;
  settings: EditorSettings;
  themeName: 'revier-light' | 'revier-dark';
  blocks: DiffBlock[];
  hideUnchangedRegions?: boolean;
  generation?: number;
  contextKey?: string | number;
  onDraftChange(value: true): void;
  onBlockSelected(block: DiffBlock): void;
  onDiffBlocksChange?(payload: MonacoDiffBlocksPayload): void;
  onCursorChange(line: number, column: number): void;
  onEditorsReady(original: editor.ICodeEditor, modified: editor.ICodeEditor): void;
}

type DiffSide = 'old' | 'new';
let nextSessionId = 0;

function modelUri(sessionId: number, side: 'original' | 'modified', path: string): monaco.Uri {
  const normalizedPath = path.replaceAll('\\', '/').replace(/^\/+/, '');
  return monaco.Uri.from({
    scheme: 'inmemory',
    authority: 'revier',
    path: `/session-${sessionId}/${side}/${normalizedPath}`
  });
}

function blockAtLine(blocks: DiffBlock[], side: DiffSide, line: number): DiffBlock | undefined {
  return blocks.find((block) => {
    const start = side === 'old' ? block.oldStart : block.newStart;
    const end = side === 'old' ? block.oldEnd : block.newEnd;
    return start > 0 && end > 0 && line >= start && line <= end;
  });
}

function blockDecorations(
  block: DiffBlock,
  side: DiffSide
): editor.IModelDeltaDecoration[] {
  const start = side === 'old' ? block.oldStart : block.newStart;
  const end = side === 'old' ? block.oldEnd : block.newEnd;
  if (start <= 0 || end <= 0) {
    return [];
  }

  return [
    {
      range: new monaco.Range(start, 1, end, 1),
      options: {
        isWholeLine: true,
        className: `revier-selected-block revier-selected-block--${side}`
      }
    },
    {
      range: new monaco.Range(start, 1, start, 1),
      options: {
        isWholeLine: true,
        className: 'revier-selected-block--top'
      }
    },
    {
      range: new monaco.Range(end, 1, end, 1),
      options: {
        isWholeLine: true,
        className: 'revier-selected-block--bottom'
      }
    }
  ];
}

export function createMonacoDiffSession(
  options: MonacoDiffSessionOptions
): MonacoDiffSession {
  let originalModel: editor.ITextModel | undefined;
  let modifiedModel: editor.ITextModel | undefined;
  let diffEditor: editor.IStandaloneDiffEditor | undefined;
  let originalDecorations: editor.IEditorDecorationsCollection | undefined;
  let modifiedDecorations: editor.IEditorDecorationsCollection | undefined;
  const listeners: monaco.IDisposable[] = [];
  let draft = false;
  let disposed = false;
  let currentBlocks = options.blocks;
  let lastPublishedSignature: string | undefined;

  const cleanupResources = (): unknown[] => {
    if (disposed) {
      return [];
    }
    disposed = true;
    const errors: unknown[] = [];
    const cleanup = (action: (() => void) | undefined): void => {
      if (!action) {
        return;
      }
      try {
        action();
      } catch (error) {
        errors.push(error);
      }
    };
    for (const listener of listeners.splice(0)) {
      cleanup(() => listener.dispose());
    }
    const originalDecorationsToClean = originalDecorations;
    const modifiedDecorationsToClean = modifiedDecorations;
    const diffEditorToClean = diffEditor;
    const originalModelToClean = originalModel;
    const modifiedModelToClean = modifiedModel;
    cleanup(originalDecorationsToClean ? () => originalDecorationsToClean.clear() : undefined);
    cleanup(modifiedDecorationsToClean ? () => modifiedDecorationsToClean.clear() : undefined);
    cleanup(diffEditorToClean ? () => diffEditorToClean.dispose() : undefined);
    cleanup(originalModelToClean ? () => originalModelToClean.dispose() : undefined);
    cleanup(modifiedModelToClean ? () => modifiedModelToClean.dispose() : undefined);
    return errors;
  };

  const reportCleanupErrors = (errors: unknown[], message: string): void => {
    if (errors.length > 0) {
      console.error(message, new AggregateError(errors, message));
    }
  };

  const disposeResources = (): void => {
    reportCleanupErrors(cleanupResources(), 'Monaco Diff 会话资源清理失败');
  };

  try {
    const sessionId = ++nextSessionId;
    const languageId = resolveInitializedLanguage(options.languageId);
    originalModel = monaco.editor.createModel(
      options.oldContent,
      languageId,
      modelUri(sessionId, 'original', options.path)
    );
    modifiedModel = monaco.editor.createModel(
      options.newContent,
      languageId,
      modelUri(sessionId, 'modified', options.path)
    );
    const editorOptions: editor.IStandaloneDiffEditorConstructionOptions & {
      largeFileOptimizations: true;
    } = {
      originalEditable: true,
      readOnly: false,
      renderSideBySide: true,
      theme: options.themeName,
      fontFamily: toFontFamily(options.settings.editor.fontFamilies),
      fontSize: options.settings.editor.fontSize,
      lineHeight: options.settings.editor.lineHeight,
      minimap: { enabled: options.settings.editor.minimap, showSlider: 'always' },
      diffAlgorithm: 'advanced',
      ignoreTrimWhitespace: false,
      folding: true,
      matchBrackets: 'always',
      largeFileOptimizations: true,
      contextmenu: true,
      links: true,
      find: { addExtraSpaceOnTop: false },
      automaticLayout: false
    };
    if (options.hideUnchangedRegions) {
      editorOptions.hideUnchangedRegions = {
        enabled: true,
        contextLineCount: 3
      };
    }
    diffEditor = monaco.editor.createDiffEditor(options.container, editorOptions);
    diffEditor.setModel({ original: originalModel, modified: modifiedModel });

    const originalEditor = diffEditor.getOriginalEditor();
    const modifiedEditor = diffEditor.getModifiedEditor();
    originalDecorations = originalEditor.createDecorationsCollection();
    modifiedDecorations = modifiedEditor.createDecorationsCollection();

    const enterDraft = (): void => {
      if (draft) {
        return;
      }
      draft = true;
      originalDecorations?.clear();
      modifiedDecorations?.clear();
      currentBlocks = [];
      options.onDraftChange(true);
    };
    listeners.push(originalModel.onDidChangeContent(enterDraft));
    listeners.push(modifiedModel.onDidChangeContent(enterDraft));

    const listenForBlock = (codeEditor: editor.ICodeEditor, side: DiffSide): void => {
      listeners.push(codeEditor.onMouseDown((event) => {
        if (draft || !event.target.position) {
          return;
        }
        const block = blockAtLine(currentBlocks, side, event.target.position.lineNumber);
        if (block) {
          options.onBlockSelected(block);
        }
      }));
    };
    listenForBlock(originalEditor, 'old');
    listenForBlock(modifiedEditor, 'new');

    const listenForCursor = (codeEditor: editor.ICodeEditor): void => {
      listeners.push(codeEditor.onDidChangeCursorPosition(({ position }) => {
        options.onCursorChange(position.lineNumber, position.column);
      }));
    };
    listenForCursor(originalEditor);
    listenForCursor(modifiedEditor);

    const publishDiffBlocks = (): void => {
      if (disposed || draft || !options.onDiffBlocksChange || !originalModel || !modifiedModel) {
        return;
      }
      const changes = diffEditor?.getLineChanges() ?? null;
      const blocks = normalizeMonacoDiffBlocks(changes, originalModel, modifiedModel);
      if (blocks === null) {
        return;
      }
      const signature = diffBlocksSignature(blocks);
      if (signature === lastPublishedSignature) {
        return;
      }
      lastPublishedSignature = signature;
      currentBlocks = blocks;
      options.onDiffBlocksChange({
        generation: options.generation ?? 0,
        contextKey: options.contextKey ?? options.path,
        signature,
        blocks
      });
    };
    if (typeof diffEditor.onDidUpdateDiff === 'function') {
      listeners.push(diffEditor.onDidUpdateDiff(publishDiffBlocks));
    }
    publishDiffBlocks();

    options.onEditorsReady(originalEditor, modifiedEditor);

    return {
      diffEditor,
      originalEditor,
      modifiedEditor,
      setLanguage(nextLanguageId) {
        if (disposed) {
          return;
        }
        const resolvedLanguage = resolveInitializedLanguage(nextLanguageId);
        monaco.editor.setModelLanguage(originalModel!, resolvedLanguage);
        monaco.editor.setModelLanguage(modifiedModel!, resolvedLanguage);
      },
      setTheme(themeName) {
        if (disposed) {
          return;
        }
        monaco.editor.setTheme(themeName);
      },
      setBlocks(blocks) {
        currentBlocks = blocks;
      },
      setSelectedBlock(block) {
        if (disposed) {
          return;
        }
        if (draft || !block) {
          originalDecorations?.clear();
          modifiedDecorations?.clear();
          return;
        }
        originalDecorations?.set(blockDecorations(block, 'old'));
        modifiedDecorations?.set(blockDecorations(block, 'new'));
      },
      layout() {
        if (!disposed) {
          diffEditor?.layout();
        }
      },
      dispose: disposeResources
    };
  } catch (error) {
    reportCleanupErrors(cleanupResources(), 'Monaco Diff 会话创建失败后的资源清理失败');
    throw error;
  }
}
