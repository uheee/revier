import * as monaco from 'monaco-editor';
import { shikiToMonaco } from '@shikijs/monaco';
import {
  createHighlighterCore as createHighlighter,
  type LanguageRegistration
} from 'shiki/core';
import { createJavaScriptRegexEngine } from 'shiki/engine/javascript';
import EditorWorker from 'monaco-editor/esm/vs/editor/editor.worker?worker';
import TypeScriptWorker from 'monaco-editor/esm/vs/language/typescript/ts.worker?worker';
import JsonWorker from 'monaco-editor/esm/vs/language/json/json.worker?worker';
import CssWorker from 'monaco-editor/esm/vs/language/css/css.worker?worker';
import HtmlWorker from 'monaco-editor/esm/vs/language/html/html.worker?worker';
import type { EditorThemeColors } from '../generated/bindings';
import { addNotification } from '../composables/useNotifications';
import { EDITOR_LANGUAGES } from './editorLanguages';
import { toShikiTheme } from './editorTheme';

globalThis.MonacoEnvironment = {
  getWorker(_moduleId: string, label: string): Worker {
    if (label === 'typescript' || label === 'javascript') {
      return new TypeScriptWorker();
    }
    if (label === 'json') {
      return new JsonWorker();
    }
    if (label === 'css' || label === 'scss' || label === 'less') {
      return new CssWorker();
    }
    if (label === 'html' || label === 'handlebars' || label === 'razor') {
      return new HtmlWorker();
    }
    return new EditorWorker();
  }
};

const failedLanguages = new Set<string>();
let syntaxInitialization: Promise<void> | undefined;

const languageLoaders: Record<string, () => Promise<{ default: LanguageRegistration[] }>> = {
  javascript: () => import('shiki/langs/javascript.mjs'),
  typescript: () => import('shiki/langs/typescript.mjs'),
  vue: () => import('shiki/langs/vue.mjs'),
  html: () => import('shiki/langs/html.mjs'),
  css: () => import('shiki/langs/css.mjs'),
  rust: () => import('shiki/langs/rust.mjs'),
  python: () => import('shiki/langs/python.mjs'),
  go: () => import('shiki/langs/go.mjs'),
  java: () => import('shiki/langs/java.mjs'),
  c: () => import('shiki/langs/c.mjs'),
  cpp: () => import('shiki/langs/cpp.mjs'),
  csharp: () => import('shiki/langs/csharp.mjs'),
  sql: () => import('shiki/langs/sql.mjs'),
  markdown: () => import('shiki/langs/markdown.mjs'),
  json: () => import('shiki/langs/json.mjs'),
  yaml: () => import('shiki/langs/yaml.mjs'),
  toml: () => import('shiki/langs/toml.mjs'),
  shellscript: () => import('shiki/langs/shellscript.mjs'),
  powershell: () => import('shiki/langs/powershell.mjs')
};

function errorDetail(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

async function initialize(themes: {
  light: EditorThemeColors;
  dark: EditorThemeColors;
}): Promise<void> {
  const highlighter = await createHighlighter({
    engine: createJavaScriptRegexEngine(),
    themes: [
      toShikiTheme('revier-light', themes.light),
      toShikiTheme('revier-dark', themes.dark)
    ],
    langs: []
  });

  for (const language of EDITOR_LANGUAGES) {
    if (!language.shikiLanguage) {
      continue;
    }
    try {
      const loader = languageLoaders[language.shikiLanguage];
      if (!loader) {
        throw new Error(`Shiki 不包含 ${language.shikiLanguage} 语言资源`);
      }
      const registration = await loader();
      await highlighter.loadLanguage(registration.default);
    } catch (error) {
      failedLanguages.add(language.id);
      addNotification({
        type: 'warning',
        title: `${language.label} 语法高亮加载失败`,
        message: `${errorDetail(error)}；已降级为纯文本。`,
        source: 'Shiki'
      });
    }
  }

  shikiToMonaco(highlighter, monaco);
}

export function initializeMonacoSyntax(themes: {
  light: EditorThemeColors;
  dark: EditorThemeColors;
}): Promise<void> {
  syntaxInitialization ??= initialize(themes);
  return syntaxInitialization;
}

export function resolveInitializedLanguage(languageId: string): string {
  return failedLanguages.has(languageId) ? 'plaintext' : languageId;
}
