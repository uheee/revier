import {
  clampReviewLayout,
  defaultReviewLayout,
  reviewLayoutStorageKey
} from '../../src/renderer/composables/useReviewLayoutSizes';

describe('review layout sizes', () => {
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
});
