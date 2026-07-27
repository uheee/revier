import { computed, readonly, ref, type Ref } from 'vue';
import { useEventListener } from '@vueuse/core';
import { editor as monacoEditor } from 'monaco-editor';
import { revierClient } from '../api/revierClient';
import type {
  EditorSettingsSnapshot,
  EditorThemeColors
} from '../generated/bindings';
import {
  resolveThemeMode,
  toCssVariables,
  toFontFamily,
  toMonacoTheme
} from '../editor/editorTheme';
import { addNotification } from './useNotifications';

function createLightColors(): EditorThemeColors {
  return {
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
}

function createDarkColors(): EditorThemeColors {
  return {
    workspaceBackground: '#111821',
    panelBackground: '#161F2A',
    editorBackground: '#101720',
    border: '#293645',
    foreground: '#DCE3EC',
    muted: '#94A1B3',
    accent: '#2DD4BF',
    selection: '#173C3A',
    diffRemoved: '#3E262C',
    diffRemovedStrong: '#E06B63',
    diffRemovedWord: '#743A43',
    diffAdded: '#1D3A30',
    diffAddedStrong: '#5EC58A',
    diffAddedWord: '#30664C',
    syntax: {
      comment: '#94A1B3',
      keyword: '#D7A0F2',
      string: '#8BD7AE',
      number: '#EFB875',
      type: '#82B7FF',
      function: '#82B7FF',
      variable: '#DCE3EC'
    }
  };
}

function createDefaultSnapshot(warning?: string): EditorSettingsSnapshot {
  return {
    configPath: '<app_config_dir>/editor.toml',
    warning,
    settings: {
      version: 1,
      theme: 'system',
      defaultEncoding: 'auto',
      editor: {
        fontFamilies: ['JetBrainsMono Nerd Font Mono', 'Microsoft YaHei', 'monospace'],
        fontSize: 13,
        lineHeight: 22,
        minimap: true
      },
      largeFile: {
        maxBytes: 1_048_576,
        maxLines: 5_000
      },
      themes: {
        light: createLightColors(),
        dark: createDarkColors()
      }
    }
  };
}

const snapshot = ref<EditorSettingsSnapshot>(createDefaultSnapshot());
const systemDark = ref(false);
const effectiveTheme = computed(() =>
  resolveThemeMode(snapshot.value.settings.theme, systemDark.value)
);
const activeColors = computed(() => snapshot.value.settings.themes[effectiveTheme.value]);
const fontFamily = computed(() => toFontFamily(snapshot.value.settings.editor.fontFamilies));
const readonlySnapshot = computed<EditorSettingsSnapshot>(() => snapshot.value);

let initialization: Promise<void> | undefined;
let colorSchemeQuery: MediaQueryList | undefined;
let stopColorSchemeListener: (() => void) | undefined;

function errorDetail(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

function reportRuntimeFailure(title: string, error: unknown): void {
  addNotification({
    type: 'error',
    title,
    message: errorDetail(error),
    source: '编辑器运行时'
  });
}

function reportSettingsWarning(): void {
  if (!snapshot.value.warning) {
    return;
  }
  addNotification({
    type: 'warning',
    title: '编辑器配置未能加载',
    message: `${snapshot.value.warning}（配置：${snapshot.value.configPath}）`,
    source: 'editor.toml'
  });
}

function applyCssTheme(): void {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(toCssVariables(activeColors.value))) {
    root.style.setProperty(name, value);
  }
  root.style.setProperty('--editor-font-family', fontFamily.value);
  root.style.setProperty('--editor-font-size', `${snapshot.value.settings.editor.fontSize}px`);
  root.style.setProperty('--editor-line-height', `${snapshot.value.settings.editor.lineHeight}px`);
  root.dataset.theme = effectiveTheme.value;
}

async function applyMonacoTheme(): Promise<void> {
  monacoEditor.defineTheme('revier-light', toMonacoTheme(snapshot.value.settings.themes.light));
  monacoEditor.defineTheme('revier-dark', toMonacoTheme(snapshot.value.settings.themes.dark));
  monacoEditor.setTheme(`revier-${effectiveTheme.value}`);
}

async function applyTheme(): Promise<void> {
  try {
    applyCssTheme();
  } catch (error) {
    reportRuntimeFailure('应用页面主题失败', error);
  }

  try {
    await applyMonacoTheme();
  } catch (error) {
    reportRuntimeFailure('应用 Monaco 主题失败', error);
  }
}

function handleSystemThemeChange(event: MediaQueryListEvent): void {
  if (snapshot.value.settings.theme !== 'system') {
    return;
  }
  systemDark.value = event.matches;
  void applyTheme().catch((error) => {
    reportRuntimeFailure('响应系统主题变化失败', error);
  });
}

async function initialize(): Promise<void> {
  try {
    snapshot.value = await revierClient.settings.getEditorSettings();
  } catch (error) {
    snapshot.value = createDefaultSnapshot(
      `读取编辑器配置失败，已使用内置默认值：${errorDetail(error)}`
    );
  }
  reportSettingsWarning();

  if (snapshot.value.settings.theme === 'system') {
    try {
      colorSchemeQuery = window.matchMedia('(prefers-color-scheme: dark)');
      systemDark.value = colorSchemeQuery.matches;
      stopColorSchemeListener = useEventListener(
        colorSchemeQuery,
        'change',
        handleSystemThemeChange
      );
    } catch (error) {
      colorSchemeQuery = undefined;
      systemDark.value = false;
      addNotification({
        type: 'warning',
        title: '监听系统主题失败',
        message: errorDetail(error),
        source: '编辑器运行时'
      });
    }
  }

  await applyTheme();
}

export async function initializeEditorSettings(): Promise<void> {
  initialization ??= initialize().catch((error) => {
    reportRuntimeFailure('初始化编辑器设置失败', error);
  });
  await initialization;
}

export function useEditorSettings(): {
  snapshot: Readonly<Ref<EditorSettingsSnapshot>>;
  effectiveTheme: Readonly<Ref<'light' | 'dark'>>;
  activeColors: Readonly<Ref<EditorThemeColors>>;
  fontFamily: Readonly<Ref<string>>;
} {
  return {
    snapshot: readonlySnapshot,
    effectiveTheme: readonly(effectiveTheme),
    activeColors: readonly(activeColors),
    fontFamily: readonly(fontFamily)
  };
}

if (import.meta.hot) {
  import.meta.hot.dispose(() => {
    stopColorSchemeListener?.();
  });
}
