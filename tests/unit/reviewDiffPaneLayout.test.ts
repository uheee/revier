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
});
