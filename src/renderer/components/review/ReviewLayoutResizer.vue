<script setup lang="ts">
const props = defineProps<{
  side: 'left' | 'right';
}>();

const emit = defineEmits<{
  resize: [side: 'left' | 'right', delta: number];
}>();

let startX = 0;

function onPointerDown(event: PointerEvent): void {
  startX = event.clientX;
  window.addEventListener('pointermove', onPointerMove);
  window.addEventListener('pointerup', onPointerUp, { once: true });
}

function onPointerMove(event: PointerEvent): void {
  const delta = event.clientX - startX;
  startX = event.clientX;
  emit('resize', props.side, delta);
}

function onPointerUp(): void {
  window.removeEventListener('pointermove', onPointerMove);
}
</script>

<template>
  <div
    class="review-resizer"
    role="separator"
    aria-orientation="vertical"
    tabindex="0"
    @pointerdown="onPointerDown"
  />
</template>
