import { toErrorMessage } from '../../src/renderer/api/errors';

describe('toErrorMessage', () => {
  it('优先展示标准 Tauri 错误消息', () => {
    expect(
      toErrorMessage({
        code: 'REPOSITORY_INVALID',
        message: '请选择一个 Git 仓库目录',
        detail: 'E:/Projects/revier'
      })
    ).toBe('请选择一个 Git 仓库目录');
  });

  it('非标准结构化错误不会显示成 [object Object]', () => {
    expect(
      toErrorMessage({
        code: 'PLUGIN_ERROR',
        detail: 'dialog open failed'
      })
    ).toBe('PLUGIN_ERROR: dialog open failed');
  });
});
