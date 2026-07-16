import * as monaco from 'monaco-editor';
import type { editor } from 'monaco-editor';
import type { DiffBlock, EditorSettings } from '../generated/bindings';
import { toFontFamily } from './editorTheme';
import { resolveInitializedLanguage } from './monacoEnvironment';

export interface MonacoDiffSession {
  diffEditor: editor.IStandaloneDiffEditor;
  originalEditor: editor.ICodeEditor;
  modifiedEditor: editor.ICodeEditor;
  setLanguage(languageId: string): void;
  setTheme(themeName: 'revier-light' | 'revier-dark'): void;
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
  onDraftChange(value: true): void;
  onBlockSelected(block: DiffBlock): void;
  onCursorChange(line: number, column: number): void;
  onEditorsReady(original: editor.ICodeEditor, modified: editor.ICodeEditor): void;
}

type DiffSide = 'old' | 'new';
type ReleasableDecorationsCollection = editor.IEditorDecorationsCollection & {
  dispose?: () => void;
};

function modelUri(side: 'original' | 'modified', path: string): monaco.Uri {
  const normalizedPath = path.replaceAll('\\', '/').replace(/^\/+/, '');
  return monaco.Uri.from({
    scheme: 'inmemory',
    authority: 'revier',
    path: `/${side}/${normalizedPath}`
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
  let originalDecorations: ReleasableDecorationsCollection | undefined;
  let modifiedDecorations: ReleasableDecorationsCollection | undefined;
  const listeners: monaco.IDisposable[] = [];
  let draft = false;
  let disposed = false;

  const disposeResources = (): void => {
    if (disposed) {
      return;
    }
    disposed = true;
    for (const listener of listeners.splice(0)) {
      listener.dispose();
    }
    originalDecorations?.clear();
    modifiedDecorations?.clear();
    originalDecorations?.dispose?.();
    modifiedDecorations?.dispose?.();
    diffEditor?.dispose();
    originalModel?.dispose();
    modifiedModel?.dispose();
  };

  try {
    const languageId = resolveInitializedLanguage(options.languageId);
    originalModel = monaco.editor.createModel(
      options.oldContent,
      languageId,
      modelUri('original', options.path)
    );
    modifiedModel = monaco.editor.createModel(
      options.newContent,
      languageId,
      modelUri('modified', options.path)
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
      minimap: { enabled: options.settings.editor.minimap },
      folding: true,
      matchBrackets: 'always',
      largeFileOptimizations: true,
      contextmenu: true,
      links: true,
      find: { addExtraSpaceOnTop: false },
      automaticLayout: false
    };
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
      options.onDraftChange(true);
    };
    listeners.push(originalModel.onDidChangeContent(enterDraft));
    listeners.push(modifiedModel.onDidChangeContent(enterDraft));

    const listenForBlock = (codeEditor: editor.ICodeEditor, side: DiffSide): void => {
      listeners.push(codeEditor.onMouseDown((event) => {
        if (draft || !event.target.position) {
          return;
        }
        const block = blockAtLine(options.blocks, side, event.target.position.lineNumber);
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
    options.onEditorsReady(originalEditor, modifiedEditor);

    return {
      diffEditor,
      originalEditor,
      modifiedEditor,
      setLanguage(nextLanguageId) {
        const resolvedLanguage = resolveInitializedLanguage(nextLanguageId);
        monaco.editor.setModelLanguage(originalModel!, resolvedLanguage);
        monaco.editor.setModelLanguage(modifiedModel!, resolvedLanguage);
      },
      setTheme(themeName) {
        monaco.editor.setTheme(themeName);
      },
      setSelectedBlock(block) {
        if (draft || !block) {
          originalDecorations?.clear();
          modifiedDecorations?.clear();
          return;
        }
        originalDecorations?.set(blockDecorations(block, 'old'));
        modifiedDecorations?.set(blockDecorations(block, 'new'));
      },
      layout() {
        diffEditor?.layout();
      },
      dispose: disposeResources
    };
  } catch (error) {
    disposeResources();
    throw error;
  }
}
