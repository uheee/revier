import type { editor } from 'monaco-editor';
import type { GlobalThemeOverrides } from 'naive-ui';
import type { ThemeRegistration } from 'shiki';
import type {
  EditorThemeColors,
  EditorThemeMode
} from '../generated/bindings';

const genericFamilies = new Set([
  'serif',
  'sans-serif',
  'monospace',
  'cursive',
  'fantasy',
  'system-ui',
  'ui-serif',
  'ui-sans-serif',
  'ui-monospace',
  'ui-rounded',
  'emoji',
  'math',
  'fangsong'
]);

function isDarkColor(color: string): boolean {
  const red = Number.parseInt(color.slice(1, 3), 16);
  const green = Number.parseInt(color.slice(3, 5), 16);
  const blue = Number.parseInt(color.slice(5, 7), 16);
  return red * 0.299 + green * 0.587 + blue * 0.114 < 128;
}

function tokenRules(theme: EditorThemeColors): editor.ITokenThemeRule[] {
  return [
    { token: 'comment', foreground: theme.syntax.comment.slice(1) },
    { token: 'keyword', foreground: theme.syntax.keyword.slice(1) },
    { token: 'string', foreground: theme.syntax.string.slice(1) },
    { token: 'number', foreground: theme.syntax.number.slice(1) },
    { token: 'type', foreground: theme.syntax.type.slice(1) },
    { token: 'function', foreground: theme.syntax.function.slice(1) },
    { token: 'variable', foreground: theme.syntax.variable.slice(1) }
  ];
}

function sharedEditorColors(theme: EditorThemeColors): Record<string, string> {
  return {
    'editor.background': theme.editorBackground,
    'editor.foreground': theme.foreground,
    'editorLineNumber.foreground': theme.muted,
    'editorCursor.foreground': theme.accent,
    'editor.selectionBackground': theme.selection,
    'editor.inactiveSelectionBackground': theme.selection,
    'editorGutter.background': theme.editorBackground,
    'editorWidget.background': theme.panelBackground,
    'editorWidget.border': theme.border,
    'minimap.background': theme.editorBackground,
    'scrollbarSlider.background': theme.border,
    'contrastBorder': theme.border,
    'focusBorder': theme.accent,
    'diffEditor.removedLineBackground': theme.diffRemoved,
    'diffEditor.removedTextBackground': theme.diffRemovedWord,
    'diffEditor.insertedLineBackground': theme.diffAdded,
    'diffEditor.insertedTextBackground': theme.diffAddedWord,
    'diffEditorGutter.removedLineBackground': theme.diffRemovedStrong,
    'diffEditorGutter.insertedLineBackground': theme.diffAddedStrong,
    'diffEditor.border': theme.border
  };
}

export function toFontFamily(families: string[]): string {
  return families
    .map((family) => {
      const trimmed = family.trim();
      if (genericFamilies.has(trimmed.toLowerCase())) {
        return trimmed;
      }
      return `"${trimmed.replaceAll('\\', '\\\\').replaceAll('"', '\\"')}"`;
    })
    .join(', ');
}

export function resolveThemeMode(
  mode: EditorThemeMode,
  systemDark: boolean
): 'light' | 'dark' {
  return mode === 'system' ? (systemDark ? 'dark' : 'light') : mode;
}

export function toCssVariables(theme: EditorThemeColors): Record<string, string> {
  return {
    '--workspace-background': theme.workspaceBackground,
    '--panel-background': theme.panelBackground,
    '--editor-background': theme.editorBackground,
    '--border-color': theme.border,
    '--foreground-color': theme.foreground,
    '--muted-color': theme.muted,
    '--accent-color': theme.accent,
    '--selection-color': theme.selection,
    '--diff-removed': theme.diffRemoved,
    '--diff-removed-strong': theme.diffRemovedStrong,
    '--diff-removed-word': theme.diffRemovedWord,
    '--diff-added': theme.diffAdded,
    '--diff-added-strong': theme.diffAddedStrong,
    '--diff-added-word': theme.diffAddedWord,
    '--syntax-comment': theme.syntax.comment,
    '--syntax-keyword': theme.syntax.keyword,
    '--syntax-string': theme.syntax.string,
    '--syntax-number': theme.syntax.number,
    '--syntax-type': theme.syntax.type,
    '--syntax-function': theme.syntax.function,
    '--syntax-variable': theme.syntax.variable
  };
}

export function toNaiveThemeOverrides(
  theme: EditorThemeColors,
  fontFamily: string
): GlobalThemeOverrides {
  return {
    common: {
      primaryColor: theme.accent,
      primaryColorHover: theme.accent,
      primaryColorPressed: theme.accent,
      primaryColorSuppl: theme.accent,
      successColor: theme.diffAddedStrong,
      successColorHover: theme.diffAddedStrong,
      successColorPressed: theme.diffAddedStrong,
      successColorSuppl: theme.diffAddedStrong,
      errorColor: theme.diffRemovedStrong,
      errorColorHover: theme.diffRemovedStrong,
      errorColorPressed: theme.diffRemovedStrong,
      errorColorSuppl: theme.diffRemovedStrong,
      bodyColor: theme.workspaceBackground,
      cardColor: theme.panelBackground,
      tableColor: theme.panelBackground,
      tableHeaderColor: theme.editorBackground,
      popoverColor: theme.panelBackground,
      modalColor: theme.panelBackground,
      inputColor: theme.editorBackground,
      codeColor: theme.editorBackground,
      hoverColor: theme.selection,
      pressedColor: theme.selection,
      borderColor: theme.border,
      dividerColor: theme.border,
      textColorBase: theme.foreground,
      textColor1: theme.foreground,
      textColor2: theme.muted,
      textColor3: theme.muted,
      placeholderColor: theme.muted,
      fontFamily,
      fontFamilyMono: fontFamily
    }
  };
}

export function toMonacoTheme(theme: EditorThemeColors): editor.IStandaloneThemeData {
  return {
    base: isDarkColor(theme.workspaceBackground) ? 'vs-dark' : 'vs',
    inherit: true,
    rules: tokenRules(theme),
    colors: sharedEditorColors(theme)
  };
}

export function toShikiTheme(name: string, theme: EditorThemeColors): ThemeRegistration {
  const settings = tokenRules(theme).map((rule) => ({
    scope: rule.token,
    settings: { foreground: `#${rule.foreground}` }
  }));

  return {
    name,
    type: isDarkColor(theme.workspaceBackground) ? 'dark' : 'light',
    bg: theme.editorBackground,
    fg: theme.foreground,
    colors: sharedEditorColors(theme),
    settings
  };
}
