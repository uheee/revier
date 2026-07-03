import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

type CssRule = {
  selectors: string[];
  declarations: Map<string, string>;
};

function readStyles(): string {
  return readFileSync(resolve(process.cwd(), 'src/renderer/styles.css'), 'utf8');
}

function parseRules(css: string): CssRule[] {
  const rules: CssRule[] = [];
  const cleanCss = css.replace(/\/\*[\s\S]*?\*\//g, '');

  for (const match of cleanCss.matchAll(/(?<selectors>[^{}]+)\{(?<body>[^{}]*)\}/g)) {
    const selectors =
      match.groups?.selectors
        .split(',')
        .map((selector) => selector.trim())
        .filter((selector) => selector.length > 0) ?? [];
    const body = match.groups?.body ?? '';
    const declarations = parseDeclarations(body);

    if (selectors.length > 0 && declarations.size > 0) {
      rules.push({ selectors, declarations });
    }
  }

  return rules;
}

function parseDeclarations(body: string): Map<string, string> {
  const declarations = new Map<string, string>();

  for (const rawDeclaration of body.split(';')) {
    const declaration = rawDeclaration.trim();
    if (declaration.length === 0) {
      continue;
    }

    const separatorIndex = declaration.indexOf(':');
    if (separatorIndex === -1) {
      continue;
    }

    const property = declaration.slice(0, separatorIndex).trim();
    const value = declaration.slice(separatorIndex + 1).trim();
    declarations.set(property, value);
  }

  return declarations;
}

function parseDeclaration(declaration: string): [string, string] {
  const normalizedDeclaration = declaration.trim().replace(/;$/, '');
  const separatorIndex = normalizedDeclaration.indexOf(':');

  expect(separatorIndex, `声明格式错误：${declaration}`).toBeGreaterThan(0);

  const property = normalizedDeclaration.slice(0, separatorIndex).trim();
  const value = normalizedDeclaration.slice(separatorIndex + 1).trim();
  return [property, value];
}

function expectRule(styles: string, selector: string, declarations: string[]): void {
  const rule = parseRules(styles).find((candidate) => candidate.selectors.includes(selector));
  expect(rule, `缺少 ${selector} 样式规则`).toBeDefined();

  for (const declaration of declarations) {
    const [property, value] = parseDeclaration(declaration);
    expect(
      rule?.declarations.get(property),
      `${selector} 缺少声明 ${declaration}`
    ).toBe(value);
  }
}

describe('diff 选中态红绿侧样式', () => {
  it.each([
    {
      selector: '.diff-row.is-selected',
      declarations: ['box-shadow: none;']
    },
    {
      selector: '.diff-row.is-selected.diff-row--modified .code-cell--old',
      declarations: ['position: relative;']
    },
    {
      selector: '.diff-row.is-selected.diff-row--modified .code-cell--new',
      declarations: ['position: relative;']
    },
    {
      selector: '.diff-row.is-selected.diff-row--modified .code-cell--old::before',
      declarations: ['background: #9f1d18;', 'left: 0;', 'width: 6px;']
    },
    {
      selector: '.diff-row.is-selected.diff-row--deleted .code-cell--old::before',
      declarations: ['background: #9f1d18;', 'left: 0;', 'width: 6px;']
    },
    {
      selector: '.diff-row.is-selected.diff-row--modified .code-cell--new::after',
      declarations: ['background: #155f34;', 'right: 0;', 'width: 6px;']
    },
    {
      selector: '.diff-row.is-selected.diff-row--added .code-cell--new::after',
      declarations: ['background: #155f34;', 'right: 0;', 'width: 6px;']
    },
    {
      selector: '.diff-row.is-selected.is-block-start.diff-row--modified .code-cell--old::before',
      declarations: ['border-top-left-radius: 4px;', 'border-top-right-radius: 4px;']
    },
    {
      selector: '.diff-row.is-selected.is-block-end.diff-row--modified .code-cell--new::after',
      declarations: ['border-bottom-left-radius: 4px;', 'border-bottom-right-radius: 4px;']
    }
  ])('要求 $selector 包含指定声明', ({ selector, declarations }) => {
    expectRule(readStyles(), selector, declarations);
  });
});
