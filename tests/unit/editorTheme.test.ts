import type { EditorThemeColors } from '../../src/renderer/generated/bindings';
import {
  resolveThemeMode,
  toCssVariables,
  toFontFamily,
  toMonacoTheme,
  toNaiveThemeOverrides,
  toShikiTheme
} from '../../src/renderer/editor/editorTheme';

const colors: EditorThemeColors = {
  workspaceBackground: '#010101',
  panelBackground: '#020202',
  editorBackground: '#030303',
  border: '#040404',
  foreground: '#050505',
  muted: '#060606',
  accent: '#070707',
  selection: '#080808',
  diffRemoved: '#090909',
  diffRemovedStrong: '#101010',
  diffRemovedWord: '#111111',
  diffAdded: '#121212',
  diffAddedStrong: '#131313',
  diffAddedWord: '#141414',
  syntax: {
    comment: '#151515',
    keyword: '#161616',
    string: '#171717',
    number: '#181818',
    type: '#191919',
    function: '#202020',
    variable: '#212121'
  }
};

describe('编辑器主题转换', () => {
  it('按顺序保留字体回落并正确引用带空格的字体名', () => {
    expect(
      toFontFamily(['JetBrainsMono Nerd Font Mono', 'Microsoft YaHei', 'monospace'])
    ).toBe('"JetBrainsMono Nerd Font Mono", "Microsoft YaHei", monospace');
    expect(toFontFamily(['Fira Code', 'sans-serif'])).toBe('"Fira Code", sans-serif');
    expect(toFontFamily([' ', '', 'monospace'])).toBe('monospace');
    expect(toFontFamily([])).toBe('monospace');
  });

  it('只在 system 模式解析系统深色偏好', () => {
    expect(resolveThemeMode('system', true)).toBe('dark');
    expect(resolveThemeMode('system', false)).toBe('light');
    expect(resolveThemeMode('light', true)).toBe('light');
    expect(resolveThemeMode('dark', false)).toBe('dark');
  });

  it('从同一颜色对象生成应用 CSS 变量', () => {
    const variables = toCssVariables(colors);

    expect(variables['--workspace-background']).toBe(colors.workspaceBackground);
    expect(variables['--panel-background']).toBe(colors.panelBackground);
    expect(variables['--editor-background']).toBe(colors.editorBackground);
    expect(variables['--border-color']).toBe(colors.border);
    expect(variables['--foreground-color']).toBe(colors.foreground);
    expect(variables['--accent-color']).toBe(colors.accent);
    expect(variables['--diff-removed']).toBe(colors.diffRemoved);
    expect(variables['--diff-added']).toBe(colors.diffAdded);
  });

  it('让 Naive UI、Monaco 与 Shiki 共用背景、前景、边框、强调色和 Diff 红绿', () => {
    const naive = toNaiveThemeOverrides(colors, '"Fira Code", monospace');
    const monaco = toMonacoTheme(colors);
    const shiki = toShikiTheme('revier-test', colors);

    expect(naive.common).toMatchObject({
      bodyColor: colors.workspaceBackground,
      cardColor: colors.panelBackground,
      codeColor: colors.editorBackground,
      textColorBase: colors.foreground,
      borderColor: colors.border,
      primaryColor: colors.accent,
      errorColor: colors.diffRemovedStrong,
      successColor: colors.diffAddedStrong,
      fontFamily: '"Fira Code", monospace',
      fontFamilyMono: '"Fira Code", monospace'
    });
    expect(monaco.colors).toMatchObject({
      'editor.background': colors.editorBackground,
      'editor.foreground': colors.foreground,
      'contrastBorder': colors.border,
      'focusBorder': colors.accent,
      'diffEditor.removedLineBackground': `${colors.diffRemoved}99`,
      'diffEditor.removedTextBackground': `${colors.diffRemovedWord}CC`,
      'diffEditor.insertedLineBackground': `${colors.diffAdded}99`,
      'diffEditor.insertedTextBackground': `${colors.diffAddedWord}CC`,
      'editor.inactiveSelectionBackground': `${colors.selection}80`
    });
    expect(shiki).toMatchObject({
      name: 'revier-test',
      bg: colors.editorBackground,
      fg: colors.foreground,
      colors: {
        'editor.background': colors.editorBackground,
        'editor.foreground': colors.foreground,
        'contrastBorder': colors.border,
        'focusBorder': colors.accent,
        'diffEditor.removedLineBackground': colors.diffRemoved,
        'diffEditor.insertedLineBackground': colors.diffAdded
      }
    });

    const shikiSettings = shiki.settings ?? [];
    expect(shikiSettings).toEqual(
      expect.arrayContaining([
        expect.objectContaining({
          scope: expect.arrayContaining(['constant.numeric']),
          settings: { foreground: colors.syntax.number }
        }),
        expect.objectContaining({
          scope: expect.arrayContaining(['entity.name.function', 'support.function']),
          settings: { foreground: colors.syntax.function }
        }),
        expect.objectContaining({
          scope: expect.arrayContaining(['entity.name.type', 'support.type']),
          settings: { foreground: colors.syntax.type }
        })
      ])
    );
  });
});
