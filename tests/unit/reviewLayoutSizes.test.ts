import {
  clampReviewLayout,
  defaultReviewLayout,
  reviewLayoutStorageKey,
  useReviewLayoutSizes
} from '../../src/renderer/composables/useReviewLayoutSizes';
import { mount } from '@vue/test-utils';
import { defineComponent, nextTick } from 'vue';

describe('review layout sizes', () => {
  beforeEach(() => {
    window.localStorage.clear();
    Object.defineProperty(window, 'innerWidth', { configurable: true, value: 1200 });
  });

  it('clamps side panes while preserving center minimum width', () => {
    const layout = clampReviewLayout(
      { left: 900, right: 900 },
      { containerWidth: 1200, centerMin: 520 }
    );

    expect(layout.left).toBeLessThanOrEqual(460);
    expect(layout.right).toBeLessThanOrEqual(520);
    expect(layout.left + layout.right).toBeLessThanOrEqual(680);
  });

  it('uses a versioned storage key', () => {
    expect(reviewLayoutStorageKey).toBe('revier.reviewLayout.v1');
    expect(defaultReviewLayout).toEqual({ left: 320, right: 320 });
  });

  it('从非法存储值回退默认布局，并在 resize 后写入同一存储键', async () => {
    window.localStorage.setItem(reviewLayoutStorageKey, '{');
    const wrapper = mount(layoutHarness());

    expect(wrapper.vm.sizes).toEqual(defaultReviewLayout);

    wrapper.vm.resize('left', 40);
    await nextTick();

    expect(JSON.parse(window.localStorage.getItem(reviewLayoutStorageKey) ?? 'null')).toEqual({
      left: 360,
      right: 320
    });
  });

  it('挂载和卸载时由 VueUse 管理窗口 resize 监听', () => {
    const add = vi.spyOn(window, 'addEventListener');
    const remove = vi.spyOn(window, 'removeEventListener');
    const wrapper = mount(layoutHarness());
    const handler = add.mock.calls.find(([type]) => type === 'resize')?.[1];

    expect(handler).toBeDefined();
    wrapper.unmount();
    expect(remove).toHaveBeenCalledWith('resize', handler, undefined);
    add.mockRestore();
    remove.mockRestore();
  });
});

function layoutHarness() {
  return defineComponent({
    setup() {
      return useReviewLayoutSizes();
    },
    template: '<div />'
  });
}
