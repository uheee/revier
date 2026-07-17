<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue';
import type { ResolvedTextEncoding, TextEncoding } from '../../generated/bindings';
import { EDITOR_LANGUAGES } from '../../editor/editorLanguages';

const ENCODINGS: readonly TextEncoding[] = [
  'auto', 'utf-8', 'gb18030', 'utf-16le', 'utf-16be'
];

const props = withDefaults(defineProps<{
  mode: 'original' | 'draft';
  line?: number;
  column?: number;
  resolvedEncoding: ResolvedTextEncoding;
  requestedEncoding: TextEncoding;
  languageId: string;
  binary?: boolean;
  selectedBlockIndex?: number;
  blockCount?: number;
}>(), {
  line: 1,
  column: 1,
  binary: false,
  selectedBlockIndex: undefined,
  blockCount: 0
});

const emit = defineEmits<{
  encodingChange: [encoding: TextEncoding];
  languageChange: [languageId: string];
}>();

const openMenu = ref<'encoding' | 'language'>();
const statusBar = ref<HTMLElement>();
const encodingTrigger = ref<HTMLButtonElement>();
const languageTrigger = ref<HTMLButtonElement>();
const languageLabel = computed(() =>
  EDITOR_LANGUAGES.find((language) => language.id === props.languageId)?.label ?? props.languageId
);

function toggleMenu(menu: 'encoding' | 'language'): void {
  openMenu.value = openMenu.value === menu ? undefined : menu;
}

function selectEncoding(encoding: TextEncoding): void {
  openMenu.value = undefined;
  emit('encodingChange', encoding);
}

function selectLanguage(languageId: string): void {
  openMenu.value = undefined;
  emit('languageChange', languageId);
}

function closeMenu(restoreFocus = false): void {
  const menu = openMenu.value;
  if (!menu) {
    return;
  }
  openMenu.value = undefined;
  if (restoreFocus) {
    void nextTick(() => {
      (menu === 'encoding' ? encodingTrigger.value : languageTrigger.value)?.focus();
    });
  }
}

function handleDocumentPointerDown(event: PointerEvent): void {
  if (event.target instanceof Node && !statusBar.value?.contains(event.target)) {
    closeMenu();
  }
}

function handleDocumentKeyDown(event: KeyboardEvent): void {
  if (event.key === 'Escape' && openMenu.value) {
    event.preventDefault();
    closeMenu(true);
  }
}

function handleMenuKeyDown(event: KeyboardEvent): void {
  if (!['ArrowDown', 'ArrowUp', 'Home', 'End'].includes(event.key)) {
    return;
  }
  const menu = event.currentTarget as HTMLElement;
  const items = Array.from(menu.querySelectorAll<HTMLButtonElement>('[role="menuitemradio"]'));
  if (items.length === 0) {
    return;
  }
  const current = items.indexOf(document.activeElement as HTMLButtonElement);
  let next = 0;
  if (event.key === 'ArrowDown') {
    next = current < 0 ? 0 : (current + 1) % items.length;
  } else if (event.key === 'ArrowUp') {
    next = current < 0 ? items.length - 1 : (current - 1 + items.length) % items.length;
  } else if (event.key === 'End') {
    next = items.length - 1;
  }
  event.preventDefault();
  items[next].focus();
}

onMounted(() => {
  document.addEventListener('pointerdown', handleDocumentPointerDown);
  document.addEventListener('keydown', handleDocumentKeyDown);
});

onBeforeUnmount(() => {
  document.removeEventListener('pointerdown', handleDocumentPointerDown);
  document.removeEventListener('keydown', handleDocumentKeyDown);
});
</script>

<template>
  <footer ref="statusBar" class="editor-status-bar">
    <div class="editor-status-bar__left">
      <span>{{ mode === 'draft' ? '临时草稿' : '原始 Diff' }}</span>
      <span v-if="mode === 'original' && selectedBlockIndex !== undefined">
        变更块 {{ selectedBlockIndex + 1 }}/{{ blockCount }}
      </span>
    </div>
    <div class="editor-status-bar__right">
      <span>Ln {{ line }}, Col {{ column }}</span>
      <div v-if="!binary" class="editor-status-control">
        <button
          ref="encodingTrigger"
          type="button"
          data-testid="encoding-trigger"
          aria-haspopup="menu"
          :aria-expanded="openMenu === 'encoding'"
          @click="toggleMenu('encoding')"
        >{{ resolvedEncoding }}</button>
        <div
          v-if="openMenu === 'encoding'"
          class="editor-status-menu editor-status-menu--upward"
          data-testid="encoding-menu"
          role="menu"
          @keydown="handleMenuKeyDown"
        >
          <button
            v-for="encoding in ENCODINGS"
            :key="encoding"
            type="button"
            role="menuitemradio"
            :aria-checked="encoding === requestedEncoding"
            @click="selectEncoding(encoding)"
          >{{ encoding }}</button>
        </div>
      </div>
      <div class="editor-status-control">
        <button
          ref="languageTrigger"
          type="button"
          data-testid="language-trigger"
          aria-haspopup="menu"
          :aria-expanded="openMenu === 'language'"
          @click="toggleMenu('language')"
        >{{ languageLabel }}</button>
        <div
          v-if="openMenu === 'language'"
          class="editor-status-menu editor-status-menu--upward"
          data-testid="language-menu"
          role="menu"
          @keydown="handleMenuKeyDown"
        >
          <button
            v-for="language in EDITOR_LANGUAGES"
            :key="language.id"
            type="button"
            role="menuitemradio"
            :aria-checked="language.id === languageId"
            @click="selectLanguage(language.id)"
          >{{ language.label }}</button>
        </div>
      </div>
    </div>
  </footer>
</template>

<style scoped>
.editor-status-bar {
  display: flex;
  flex: 0 0 26px;
  align-items: center;
  justify-content: space-between;
  min-width: 0;
  padding-inline: 10px;
  color: var(--muted-color);
  background: var(--panel-background);
  border-top: 1px solid var(--border-color);
  font-size: 12px;
}

.editor-status-bar__left,
.editor-status-bar__right {
  display: flex;
  align-items: center;
  gap: 12px;
}

.editor-status-control {
  position: relative;
  height: 100%;
}

.editor-status-control > button,
.editor-status-menu button {
  border: 0;
  color: inherit;
  background: transparent;
  font: inherit;
  cursor: pointer;
}

.editor-status-control > button {
  height: 100%;
  padding-inline: 6px;
}

.editor-status-control > button:hover,
.editor-status-menu button:hover,
.editor-status-menu button[aria-checked="true"] {
  color: var(--foreground-color);
  background: var(--selection-color);
}

.editor-status-menu {
  position: absolute;
  right: 0;
  z-index: 20;
  display: grid;
  min-width: 150px;
  max-height: 320px;
  overflow-y: auto;
  padding-block: 4px;
  background: var(--panel-background);
  border: 1px solid var(--border-color);
  box-shadow: 0 8px 24px rgb(0 0 0 / 18%);
}

.editor-status-menu--upward {
  bottom: calc(100% + 4px);
}

.editor-status-menu button {
  padding: 6px 12px;
  text-align: left;
  white-space: nowrap;
}
</style>
