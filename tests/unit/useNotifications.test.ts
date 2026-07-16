import { nextTick } from 'vue';

async function loadNotifications() {
  vi.resetModules();
  return import('../../src/renderer/composables/useNotifications');
}

describe('会话通知队列', () => {
  it('支持三类通知并按新到旧排列', async () => {
    const module = await loadNotifications();
    const center = module.useNotifications();

    module.addNotification({ type: 'info', title: '信息' });
    module.addNotification({ type: 'warning', title: '警告' });
    module.addNotification({ type: 'error', title: '错误' });

    expect(center.notifications.value.map((item) => [item.type, item.title])).toEqual([
      ['error', '错误'],
      ['warning', '警告'],
      ['info', '信息']
    ]);
    expect(center.unreadCount.value).toBe(3);
    expect(center.unreadBadge.value).toBe('3');
  });

  it('最多保留 100 条并将超过 99 的未读数显示为 99+', async () => {
    const module = await loadNotifications();
    const center = module.useNotifications();

    for (let index = 0; index < 99; index += 1) {
      module.addNotification({ type: 'info', title: `消息 ${index}` });
    }

    expect(center.unreadBadge.value).toBe('99');
    module.addNotification({ type: 'info', title: '消息 99' });
    module.addNotification({ type: 'info', title: '消息 100' });

    expect(center.notifications.value).toHaveLength(100);
    expect(center.notifications.value[0]?.title).toBe('消息 100');
    expect(center.notifications.value.at(-1)?.title).toBe('消息 1');
    expect(center.unreadBadge.value).toBe('99+');
  });

  it('全部已读保留历史，并支持删除单条和清空全部', async () => {
    const module = await loadNotifications();
    const center = module.useNotifications();
    const first = module.addNotification({ type: 'info', title: '第一条' });
    module.addNotification({ type: 'warning', title: '第二条' });

    center.markAllRead();
    expect(center.unreadCount.value).toBe(0);
    expect(center.unreadBadge.value).toBeUndefined();
    expect(center.notifications.value).toHaveLength(2);

    center.remove(first.id);
    expect(center.notifications.value.map((item) => item.title)).toEqual(['第二条']);
    center.clear();
    expect(center.notifications.value).toEqual([]);
  });

  it('对外通知列表为只读状态', async () => {
    const module = await loadNotifications();
    const center = module.useNotifications();
    module.addNotification({ type: 'info', title: '保留' });
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => undefined);

    (center.notifications.value as unknown as Array<unknown>).push({});
    await nextTick();

    expect(center.notifications.value).toHaveLength(1);
    expect(warn).toHaveBeenCalled();
    warn.mockRestore();
  });
});
