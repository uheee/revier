import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function componentSource(): string {
  return readFileSync(
    resolve(process.cwd(), 'src/renderer/components/review/DiffViewer.vue'),
    'utf8'
  );
}

function declarationBody(source: string, selector: string): string {
  const escaped = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const match = source.match(new RegExp(`${escaped}\\s*\\{(?<body>[^}]*)\\}`));
  expect(match?.groups?.body, `缺少 ${selector} 样式规则`).toBeDefined();
  return match!.groups!.body;
}

describe('Monaco Diff 真实块选中边界', () => {
  it('旧侧使用深红外缘，新侧使用深绿外缘', () => {
    const source = componentSource();
    expect(declarationBody(source, ':global(.revier-selected-block--old)'))
      .toContain('box-shadow: inset 6px 0 0 var(--diff-removed-strong);');
    expect(declarationBody(source, ':global(.revier-selected-block--new)'))
      .toContain('box-shadow: inset -6px 0 0 var(--diff-added-strong);');
  });

  it('仅首行绘制 teal 上边界、尾行绘制 teal 下边界', () => {
    const source = componentSource();
    expect(declarationBody(source, ':global(.revier-selected-block--top)'))
      .toContain('border-top: 2px solid var(--accent-color);');
    expect(declarationBody(source, ':global(.revier-selected-block--bottom)'))
      .toContain('border-bottom: 2px solid var(--accent-color);');
  });

  it('中间 Decoration class 不绘制横线', () => {
    const source = componentSource();
    const exactMiddleRule = source.match(/:global\(\.revier-selected-block\)\s*\{(?<body>[^}]*)\}/);
    expect(exactMiddleRule?.groups?.body ?? '').not.toMatch(/border-(?:top|bottom)/);
  });
});
