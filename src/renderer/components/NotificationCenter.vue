<template>
  <n-popover
    v-model:show="show"
    trigger="click"
    placement="bottom-end"
    :show-arrow="false"
    @update:show="handleVisibilityChange"
  >
    <template #trigger>
      <n-badge
        class="notification-center__badge"
        :value="unreadBadge"
        :show="Boolean(unreadBadge)"
        :aria-label="unreadBadge ? `${unreadCount} 条未读通知` : undefined"
      >
        <button class="notification-center__trigger" type="button" aria-label="通知中心">
          <svg aria-hidden="true" viewBox="0 0 24 24">
            <path d="M18 8a6 6 0 0 0-12 0c0 7-3 7-3 9h18c0-2-3-2-3-9M10 21h4" />
          </svg>
        </button>
      </n-badge>
    </template>

    <section class="notification-center__panel" aria-label="通知列表">
      <header class="notification-center__header">
        <strong>通知</strong>
        <button
          v-if="notifications.length"
          type="button"
          aria-label="清空全部通知"
          @click="clear"
        >清空全部</button>
      </header>
      <div v-if="!notifications.length" class="notification-center__empty">暂无通知</div>
      <n-scrollbar
        v-else
        class="notification-center__list notification-center__list--scrollable"
        :style="{ maxHeight: listMaxHeight }"
      >
        <article
          v-for="notification in notifications"
          :key="notification.id"
          class="notification-center__item"
          :data-type="notification.type"
        >
          <span class="notification-center__kind" :aria-label="notification.type"></span>
          <div class="notification-center__content">
            <strong>{{ notification.title }}</strong>
            <p v-if="notification.message">{{ notification.message }}</p>
            <small>{{ notification.source || 'Revier' }} · {{ formatTime(notification.createdAt) }}</small>
          </div>
          <button
            type="button"
            :aria-label="`删除通知：${notification.title}`"
            @click="remove(notification.id)"
          >×</button>
        </article>
      </n-scrollbar>
    </section>
  </n-popover>
</template>

<script setup lang="ts">
import { ref } from 'vue';
import { useNotifications } from '../composables/useNotifications';

const show = ref(false);
const { notifications, unreadCount, unreadBadge, markAllRead, remove, clear } = useNotifications();
const listMaxHeight = `${Math.max(160, Math.min(460, window.innerHeight - 96))}px`;

function handleVisibilityChange(visible: boolean): void {
  if (visible) {
    markAllRead();
  }
}

function formatTime(value: string): string {
  return new Intl.DateTimeFormat('zh-CN', {
    hour: '2-digit',
    minute: '2-digit',
    second: '2-digit'
  }).format(new Date(value));
}
</script>

<style scoped>
.notification-center__trigger {
  position: relative;
  display: grid;
  width: 34px;
  height: 34px;
  padding: 7px;
  color: var(--foreground-color);
  background: var(--panel-background);
  border: 1px solid var(--border-color);
  border-radius: 6px;
  cursor: pointer;
}

.notification-center__trigger:hover {
  border-color: var(--accent-color);
  background: var(--selection-color);
}

.notification-center__trigger svg {
  width: 18px;
  fill: none;
  stroke: currentColor;
  stroke-linecap: round;
  stroke-linejoin: round;
  stroke-width: 2;
}

.notification-center__badge :deep(.n-badge-sup) {
  right: -7px;
  top: auto;
  bottom: -6px;
  height: 16px;
  font-size: 10px;
  line-height: 16px;
  border: 2px solid var(--panel-background);
  transform: none;
}

.notification-center__panel {
  width: min(380px, calc(100vw - 32px));
  color: var(--foreground-color);
}

.notification-center__header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 36px;
  padding: 0 4px 8px;
  border-bottom: 1px solid var(--border-color);
}

.notification-center__header button,
.notification-center__item button {
  color: var(--muted-color);
  background: transparent;
  border: 0;
  cursor: pointer;
}

.notification-center__empty {
  padding: 40px 16px;
  color: var(--muted-color);
  text-align: center;
}

:deep(.notification-center__list) {
  max-height: 460px;
  max-height: min(460px, calc(100vh - 96px));
}

.notification-center__item {
  display: grid;
  grid-template-columns: 10px 1fr 24px;
  gap: 10px;
  padding: 12px 4px;
  border-bottom: 1px solid var(--border-color);
}

.notification-center__kind {
  width: 8px;
  height: 8px;
  margin-top: 6px;
  background: var(--accent-color);
  border-radius: 50%;
}

.notification-center__item[data-type='warning'] .notification-center__kind {
  background: #f0a020;
}

.notification-center__item[data-type='error'] .notification-center__kind {
  background: #d03050;
}

.notification-center__content {
  min-width: 0;
}

.notification-center__content p {
  margin: 4px 0;
  color: var(--foreground-color);
  overflow-wrap: anywhere;
}

.notification-center__content small {
  color: var(--muted-color);
}
</style>
