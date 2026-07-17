import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function readStyles(): string {
  return readFileSync(resolve(process.cwd(), 'src/renderer/styles.css'), 'utf8');
}

function expectRule(styles: string, selector: string, declarations: string[]): void {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const ruleMatch = styles.match(new RegExp(`${escapedSelector}\\s*\\{(?<body>[^}]*)\\}`, 's'));
  expect(ruleMatch?.groups?.body, `缺少 ${selector} 样式规则`).toBeDefined();

  const body = ruleMatch?.groups?.body ?? '';
  for (const declaration of declarations) {
    expect(body).toContain(declaration);
  }
}

function expectSelectorNeverDeclares(styles: string, selector: string, declaration: string): void {
  const rules = styles.matchAll(/(?<selectors>[^{}]+)\{(?<body>[^}]*)\}/g);
  for (const rule of rules) {
    const selectors = rule.groups?.selectors.split(',').map((item) => item.trim()) ?? [];
    if (selectors.includes(selector)) {
      expect(rule.groups?.body ?? '').not.toContain(declaration);
    }
  }
}

describe('review diff pane layout', () => {
  it('keeps the outer diff pane fixed while individual viewers scroll internally', () => {
    const styles = readStyles();

    expectRule(styles, '.review-diff-pane', ['overflow: hidden;']);
    expectSelectorNeverDeclares(styles, '.review-diff-pane', 'overflow: visible;');
    expectRule(styles, '.diff-viewer', ['height: 100%;', 'overflow: auto;']);
    expectRule(styles, '.diff-drilldown', [
      'overflow: hidden;',
      'display: flex;',
      'flex-direction: column;'
    ]);
    expect(styles).not.toContain('.drilldown-cover-enter-active');
    expect(styles).not.toContain('.drilldown-cover-leave-active');
  });

  it('使用固定主题变量统一工作区、面板、编辑器与浮层视觉', () => {
    const styles = readStyles();
    const variables = [
      '--workspace-background',
      '--panel-background',
      '--editor-background',
      '--border-color',
      '--foreground-color',
      '--muted-color',
      '--accent-color',
      '--selection-color',
      '--diff-removed',
      '--diff-removed-strong',
      '--diff-removed-word',
      '--diff-added',
      '--diff-added-strong',
      '--diff-added-word',
      '--editor-font-family',
      '--editor-font-size',
      '--editor-line-height'
    ];

    for (const variable of variables) {
      expect(styles).toContain(`${variable}:`);
    }

    expectRule(styles, '.app-theme-root', [
      'color: var(--foreground-color);',
      'background: var(--workspace-background);'
    ]);
    expectRule(styles, '.review-workspace', ['background: var(--workspace-background);']);
    expectRule(styles, '.review-sidebar', ['background: var(--panel-background);']);
    expectRule(styles, '.review-detail-pane', ['background: var(--panel-background);']);
    expectRule(styles, '.review-diff-pane', ['background: var(--editor-background);']);
    expectRule(styles, '.diff-drilldown', ['background: var(--editor-background);']);
  });

  it('为 Monaco 与固定宽度 AuthorRail 使用双列布局，并让草稿恢复单列', () => {
    const styles = readStyles();

    expectRule(styles, '.diff-viewer__editor-layout', [
      'display: grid;',
      'grid-template-columns: minmax(0, 1fr) 112px;'
    ]);
    expectRule(styles, '.diff-viewer__editor-layout:not(:has(.diff-author-rail))', [
      'grid-template-columns: minmax(0, 1fr);'
    ]);
    expectRule(styles, '.diff-author-rail', ['width: 112px;', 'min-width: 112px;']);
    expectRule(styles, '.diff-author-rail__author', [
      'height: 20px;',
      'white-space: nowrap;'
    ]);
    expectRule(styles, '.diff-author-rail__block', ['gap: 2px;']);
    expectRule(styles, '.diff-author-rail__popover', [
      'max-height: 320px;',
      'overflow-y: auto;'
    ]);
    expectRule(styles, '.editor-status-menu--upward', ['bottom: calc(100% + 4px);']);
  });

  it('通知入口与可滚动面板沿用主题变量', () => {
    const styles = readStyles();

    expectRule(styles, '.notification-center__trigger', [
      'color: var(--foreground-color);',
      'background: var(--panel-background);',
      'border: 1px solid var(--border-color);'
    ]);
    expectRule(styles, '.notification-center__badge .n-badge-sup', [
      'border-color: var(--panel-background);'
    ]);
    expectRule(styles, '.notification-center__panel', [
      'color: var(--foreground-color);',
      'background: var(--panel-background);'
    ]);
    expectRule(styles, '.notification-center__list--scrollable', ['overflow-y: auto;']);
  });

  it('不再保留旧 Diff 表格与行内作者运行时样式', () => {
    const styles = readStyles();

    expect(styles).not.toContain('.diff-table--full');
    expect(styles).not.toContain('.diff-row__authors');
  });
});
