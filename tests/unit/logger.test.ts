const harness = vi.hoisted(() => ({
  attachConsole: vi.fn(),
  trace: vi.fn(),
  debug: vi.fn(),
  info: vi.fn(),
  warn: vi.fn(),
  error: vi.fn()
}));

vi.mock('@tauri-apps/plugin-log', () => harness);

describe('渲染器日志边界', () => {
  beforeEach(() => {
    vi.resetModules();
    for (const writer of Object.values(harness)) {
      writer.mockReset().mockResolvedValue(undefined);
    }
    Object.defineProperty(window, '__TAURI_INTERNALS__', {
      configurable: true,
      value: {}
    });
  });

  afterEach(() => {
    Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
  });

  it('写入插件前隐藏路径和邮箱并丢弃未批准的上下文字段', async () => {
    const { logError } = await import('../../src/renderer/api/logger');

    logError(
      '读取 E:/Program Files/secret/repo 失败，联系 dev@example.com',
      new Error('联系 dev@example.com 查看 src/private/main.ts 和 /home/user/repo'),
      {
        source: 'Review',
        fileCount: 3,
        ...{ queryKey: 'projects:E:/secret/repo' }
      }
    );

    await vi.waitFor(() => expect(harness.error).toHaveBeenCalled());
    expect(harness.error).toHaveBeenCalledWith(
      '读取 [路径已脱敏] [路径已脱敏] 失败，联系 [邮箱已脱敏]：Error',
      {
        keyValues: {
          source: 'Review',
          fileCount: '3'
        }
      }
    );
  });

  it('非 Tauri 环境只把脱敏后的错误写入 console', async () => {
    Reflect.deleteProperty(window, '__TAURI_INTERNALS__');
    const consoleError = vi.spyOn(console, 'error').mockImplementation(() => undefined);
    const { logError } = await import('../../src/renderer/api/logger');

    logError('读取 E:/private/repo 失败', new Error('owner@example.com'));

    await vi.waitFor(() => expect(consoleError).toHaveBeenCalled());
    expect(consoleError).toHaveBeenCalledWith(
      '读取 [路径已脱敏] 失败：Error'
    );
    expect(harness.error).not.toHaveBeenCalled();
  });

  it('未知对象错误不会序列化完整 payload', async () => {
    const { logError } = await import('../../src/renderer/api/logger');

    logError('请求失败', {
      payload: {
        source: 'const secret = true;',
        path: 'E:/private/repo'
      }
    });

    await vi.waitFor(() => expect(harness.error).toHaveBeenCalled());
    expect(harness.error).toHaveBeenCalledWith('请求失败：错误详情已省略', undefined);
  });

  it('字符串和 Error 中的正文型 payload 不会进入日志', async () => {
    const { logError } = await import('../../src/renderer/api/logger');

    logError('对象请求失败', new Error('{"content":"const secret = true;"}'));
    logError('多行请求失败', '第一行源码\n第二行源码');

    await vi.waitFor(() => expect(harness.error).toHaveBeenCalledTimes(2));
    expect(harness.error).toHaveBeenNthCalledWith(
      1,
      '对象请求失败：Error',
      undefined
    );
    expect(harness.error).toHaveBeenNthCalledWith(
      2,
      '多行请求失败：错误详情已省略',
      undefined
    );
  });

  it('重复初始化只建立一个 WebView 日志桥接', async () => {
    harness.attachConsole.mockResolvedValue(() => undefined);
    const { initializeRendererLogger } = await import('../../src/renderer/api/logger');

    await initializeRendererLogger();
    await initializeRendererLogger();

    expect(harness.attachConsole).toHaveBeenCalledTimes(1);
  });
});
