// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import ReviewLayoutResizer from '../../src/renderer/components/review/ReviewLayoutResizer.vue';

describe('ReviewLayoutResizer', () => {
  beforeEach(() => {
    vi.stubGlobal('PointerEvent', MouseEvent);
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('只在拖拽期间响应窗口 pointermove，并在 pointerup 后停止', async () => {
    const wrapper = mount(ReviewLayoutResizer, {
      props: { side: 'left' }
    });

    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 120 }));
    expect(wrapper.emitted('resize')).toBeUndefined();

    await wrapper.get('[role="separator"]').trigger('pointerdown', { clientX: 100 });
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 116 }));
    expect(wrapper.emitted('resize')).toEqual([['left', 16]]);

    window.dispatchEvent(new PointerEvent('pointerup'));
    window.dispatchEvent(new PointerEvent('pointermove', { clientX: 140 }));
    expect(wrapper.emitted('resize')).toEqual([['left', 16]]);
  });
});
