import { computed, ref } from 'vue';
import { useEventListener, useStorage } from '@vueuse/core';

export const reviewLayoutStorageKey = 'revier.reviewLayout.v1';
export const defaultReviewLayout = { left: 320, right: 320 };
const leftMin = 240;
const leftMax = 460;
const rightMin = 180;
const rightMax = 640;

export interface ReviewLayoutSizes {
  left: number;
  right: number;
}

export interface ClampReviewLayoutOptions {
  containerWidth: number;
  centerMin: number;
}

export function clampReviewLayout(
  sizes: ReviewLayoutSizes,
  options: ClampReviewLayoutOptions
): ReviewLayoutSizes {
  const maxSideTotal = Math.max(0, options.containerWidth - options.centerMin);
  let left = Math.min(Math.max(sizes.left, leftMin), leftMax);
  let right = Math.min(Math.max(sizes.right, rightMin), rightMax);
  const total = left + right;

  if (total > maxSideTotal) {
    let overflow = total - maxSideTotal;
    const rightReduction = Math.min(overflow, Math.max(0, right - rightMin));
    right -= rightReduction;
    overflow -= rightReduction;
    left = Math.max(leftMin, left - overflow);
  }

  return { left, right };
}

export function useReviewLayoutSizes() {
  const container = ref<HTMLElement>();
  const sizes = useStorage<ReviewLayoutSizes>(
    reviewLayoutStorageKey,
    defaultReviewLayout,
    window.localStorage,
    {
      serializer: reviewLayoutSerializer,
      writeDefaults: false,
      listenToStorageChanges: false
    }
  );
  const gridTemplateColumns = computed(
    () => `${sizes.value.left}px 6px minmax(0, 1fr) 6px ${sizes.value.right}px`
  );

  function setContainer(element?: HTMLElement): void {
    container.value = element;
    applyClamp();
  }

  function resize(side: 'left' | 'right', delta: number): void {
    sizes.value =
      side === 'left'
        ? { ...sizes.value, left: sizes.value.left + delta }
        : { ...sizes.value, right: Math.min(Math.max(sizes.value.right - delta, rightMin), rightMax) };
    applyClamp();
  }

  function applyClamp(): void {
    const width = container.value?.clientWidth ?? window.innerWidth;
    sizes.value = clampReviewLayout(sizes.value, { containerWidth: width, centerMin: 360 });
  }

  useEventListener(window, 'resize', applyClamp);

  return { sizes, gridTemplateColumns, setContainer, resize };
}

const reviewLayoutSerializer = {
  read(value: string): ReviewLayoutSizes {
    try {
      const parsed = JSON.parse(value) as ReviewLayoutSizes | null;
      return isReviewLayoutSizes(parsed) ? parsed : defaultReviewLayout;
    } catch {
      return defaultReviewLayout;
    }
  },
  write(value: ReviewLayoutSizes): string {
    return JSON.stringify(value);
  }
};

function isReviewLayoutSizes(value: ReviewLayoutSizes | null): value is ReviewLayoutSizes {
  return Boolean(value && Number.isFinite(value.left) && Number.isFinite(value.right));
}
