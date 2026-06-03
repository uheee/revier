import { computed, onMounted, onUnmounted, ref } from 'vue';

export const reviewLayoutStorageKey = 'revier.reviewLayout.v1';
export const defaultReviewLayout = { left: 320, right: 320 };

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
  let left = Math.min(Math.max(sizes.left, 260), 460);
  let right = Math.min(Math.max(sizes.right, 280), 520);
  const total = left + right;

  if (total > maxSideTotal) {
    const overflow = total - maxSideTotal;
    const leftShare = left / total;
    left = Math.max(260, Math.floor(left - overflow * leftShare));
    right = Math.max(280, Math.floor(right - overflow * (1 - leftShare)));
  }

  return { left, right };
}

export function useReviewLayoutSizes() {
  const container = ref<HTMLElement>();
  const sizes = ref<ReviewLayoutSizes>(loadStoredLayout());
  const gridTemplateColumns = computed(
    () => `${sizes.value.left}px 6px minmax(520px, 1fr) 6px ${sizes.value.right}px`
  );

  function setContainer(element?: HTMLElement): void {
    container.value = element;
    applyClamp();
  }

  function resize(side: 'left' | 'right', delta: number): void {
    sizes.value =
      side === 'left'
        ? { ...sizes.value, left: sizes.value.left + delta }
        : { ...sizes.value, right: sizes.value.right - delta };
    applyClamp();
    window.localStorage.setItem(reviewLayoutStorageKey, JSON.stringify(sizes.value));
  }

  function applyClamp(): void {
    const width = container.value?.clientWidth ?? window.innerWidth;
    sizes.value = clampReviewLayout(sizes.value, { containerWidth: width, centerMin: 520 });
  }

  onMounted(() => window.addEventListener('resize', applyClamp));
  onUnmounted(() => window.removeEventListener('resize', applyClamp));

  return { sizes, gridTemplateColumns, setContainer, resize };
}

function loadStoredLayout(): ReviewLayoutSizes {
  try {
    const parsed = JSON.parse(window.localStorage.getItem(reviewLayoutStorageKey) ?? 'null') as
      | ReviewLayoutSizes
      | null;
    return parsed && Number.isFinite(parsed.left) && Number.isFinite(parsed.right)
      ? parsed
      : defaultReviewLayout;
  } catch {
    return defaultReviewLayout;
  }
}
