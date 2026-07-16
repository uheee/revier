import { computed, readonly, ref, type DeepReadonly, type Ref } from 'vue';

export type NotificationType = 'info' | 'warning' | 'error';

export interface AppNotification {
  id: number;
  type: NotificationType;
  title: string;
  message?: string;
  source?: string;
  createdAt: string;
  read: boolean;
}

export type NewNotification = Pick<AppNotification, 'type' | 'title'> &
  Partial<Pick<AppNotification, 'message' | 'source'>>;

const MAX_NOTIFICATIONS = 100;
const notifications = ref<AppNotification[]>([]);
const unreadCount = computed(() => notifications.value.filter((item) => !item.read).length);
const unreadBadge = computed(() => {
  if (unreadCount.value === 0) {
    return undefined;
  }
  return unreadCount.value > 99 ? '99+' : String(unreadCount.value);
});
let nextId = 1;

export function addNotification(input: NewNotification): void {
  const notification: AppNotification = {
    ...input,
    id: nextId,
    createdAt: new Date().toISOString(),
    read: false
  };
  nextId += 1;
  notifications.value = [notification, ...notifications.value].slice(0, MAX_NOTIFICATIONS);
}

function markAllRead(): void {
  notifications.value = notifications.value.map((item) => ({ ...item, read: true }));
}

function remove(id: number): void {
  notifications.value = notifications.value.filter((item) => item.id !== id);
}

function clear(): void {
  notifications.value = [];
}

export function useNotifications(): {
  notifications: DeepReadonly<Ref<AppNotification[]>>;
  unreadCount: Readonly<Ref<number>>;
  unreadBadge: Readonly<Ref<string | undefined>>;
  markAllRead: () => void;
  remove: (id: number) => void;
  clear: () => void;
} {
  return {
    notifications: readonly(notifications),
    unreadCount: readonly(unreadCount),
    unreadBadge: readonly(unreadBadge),
    markAllRead,
    remove,
    clear
  };
}
