// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import EditorStatusBar from '../../src/renderer/components/review/EditorStatusBar.vue';

function mountStatus(overrides: Record<string, unknown> = {}) {
  return mount(EditorStatusBar, {
    props: {
      mode: 'original',
      line: 12,
      column: 8,
      resolvedEncoding: 'utf-8',
      requestedEncoding: 'auto',
      languageId: 'typescript',
      binary: false,
      ...overrides
    }
  });
}

describe('EditorStatusBar', () => {
  it('显示编辑器状态、光标、实际编码和语言名称', () => {
    const original = mountStatus();
    expect(original.text()).toContain('原始 Diff');
    expect(original.text()).toContain('Ln 12, Col 8');
    expect(original.text()).toContain('utf-8');
    expect(original.text()).toContain('TypeScript');

    const draft = mountStatus({ mode: 'draft' });
    expect(draft.text()).toContain('临时草稿');
  });

  it('在状态栏内从点击位置向上展开编码菜单，并按请求编码勾选', async () => {
    const wrapper = mountStatus();
    await wrapper.get('[data-testid="encoding-trigger"]').trigger('click');

    const menu = wrapper.get('[data-testid="encoding-menu"]');
    expect(wrapper.get('.editor-status-bar').element.contains(menu.element)).toBe(true);
    expect(menu.classes()).toContain('editor-status-menu--upward');
    expect(menu.findAll('button').map((item) => item.text())).toEqual([
      'auto', 'utf-8', 'gb18030', 'utf-16le', 'utf-16be'
    ]);
    expect(menu.get('[aria-checked="true"]').text()).toBe('auto');

    await menu.findAll('button')[2].trigger('click');
    expect(wrapper.emitted('encodingChange')).toEqual([['gb18030']]);
  });

  it('在状态栏内向上展开语言菜单并发出语言选择', async () => {
    const wrapper = mountStatus();
    await wrapper.get('[data-testid="language-trigger"]').trigger('click');

    const menu = wrapper.get('[data-testid="language-menu"]');
    expect(wrapper.get('.editor-status-bar').element.contains(menu.element)).toBe(true);
    expect(menu.classes()).toContain('editor-status-menu--upward');
    await menu.findAll('button').find((item) => item.text() === 'Rust')!.trigger('click');
    expect(wrapper.emitted('languageChange')).toEqual([['rust']]);
  });

  it('二进制状态不显示编码按钮', () => {
    expect(mountStatus({ binary: true }).find('[data-testid="encoding-trigger"]').exists()).toBe(false);
  });
});
