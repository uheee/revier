import { computed, readonly, ref, type Ref } from 'vue';
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

const lightColors: EditorThemeColors = {
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

const darkColors: EditorThemeColors = {
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

const defaultSnapshot: EditorSettingsSnapshot = {
  configPath: '<app_config_dir>/editor.toml',
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
      light: lightColors,
      dark: darkColors
    }
  }
};

const snapshot = ref<EditorSettingsSnapshot>(defaultSnapshot);
const systemDark = ref(false);
const effectiveTheme = computed(() =>
  resolveThemeMode(snapshot.value.settings.theme, systemDark.value)
);
const activeColors = computed(() => snapshot.value.settings.themes[effectiveTheme.value]);
const fontFamily = computed(() => toFontFamily(snapshot.value.settings.editor.fontFamilies));
const readonlySnapshot = computed<EditorSettingsSnapshot>(() => snapshot.value);

let initialization: Promise<void> | undefined;
let colorSchemeQuery: MediaQueryList | undefined;

async function applyTheme(): Promise<void> {
  const root = document.documentElement;
  for (const [name, value] of Object.entries(toCssVariables(activeColors.value))) {
    root.style.setProperty(name, value);
  }
  root.style.setProperty('--editor-font-family', fontFamily.value);
  root.style.setProperty('--editor-font-size', `${snapshot.value.settings.editor.fontSize}px`);
  root.style.setProperty('--editor-line-height', `${snapshot.value.settings.editor.lineHeight}px`);
  root.dataset.theme = effectiveTheme.value;

  const { editor } = await import('monaco-editor');
  editor.defineTheme('revier-light', toMonacoTheme(snapshot.value.settings.themes.light));
  editor.defineTheme('revier-dark', toMonacoTheme(snapshot.value.settings.themes.dark));
  editor.setTheme(`revier-${effectiveTheme.value}`);
}

function handleSystemThemeChange(event: MediaQueryListEvent): void {
  if (snapshot.value.settings.theme !== 'system') {
    return;
  }
  systemDark.value = event.matches;
  void applyTheme();
}

function initialize(): Promise<void> {
  return (async () => {
    colorSchemeQuery = window.matchMedia('(prefers-color-scheme: dark)');
    systemDark.value = colorSchemeQuery.matches;

    try {
      snapshot.value = await revierClient.settings.getEditorSettings();
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error);
      snapshot.value = {
        ...defaultSnapshot,
        warning: `读取编辑器配置失败，已使用内置默认值：${detail}`
      };
    }

    if (snapshot.value.settings.theme === 'system') {
      colorSchemeQuery.addEventListener('change', handleSystemThemeChange);
    }
    await applyTheme();
  })();
}

export async function initializeEditorSettings(): Promise<void> {
  initialization ??= initialize();
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
    colorSchemeQuery?.removeEventListener('change', handleSystemThemeChange);
  });
}
