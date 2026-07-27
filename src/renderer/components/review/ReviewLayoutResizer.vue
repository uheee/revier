<script setup lang="ts">
import { ref } from 'vue';
import { useEventListener } from '@vueuse/core';

const props = defineProps<{
  side: 'left' | 'right';
}>();

const emit = defineEmits<{
  resize: [side: 'left' | 'right', delta: number];
}>();

let startX = 0;
const dragging = ref(false);

function onPointerDown(event: PointerEvent): void {
  startX = event.clientX;
  dragging.value = true;
}

function onPointerMove(event: PointerEvent): void {
  if (!dragging.value) {
    return;
  }
  const delta = event.clientX - startX;
  startX = event.clientX;
  emit('resize', props.side, delta);
}

function onPointerUp(): void {
  dragging.value = false;
}

useEventListener(window, 'pointermove', onPointerMove);
useEventListener(window, 'pointerup', onPointerUp);
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
