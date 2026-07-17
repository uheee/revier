<script setup lang="ts">
import { computed, ref } from 'vue';
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
</script>

<template>
  <footer class="editor-status-bar">
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
          type="button"
          data-testid="encoding-trigger"
          :aria-expanded="openMenu === 'encoding'"
          @click="toggleMenu('encoding')"
        >{{ resolvedEncoding }}</button>
        <div
          v-if="openMenu === 'encoding'"
          class="editor-status-menu editor-status-menu--upward"
          data-testid="encoding-menu"
          role="menu"
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
          type="button"
          data-testid="language-trigger"
          :aria-expanded="openMenu === 'language'"
          @click="toggleMenu('language')"
        >{{ languageLabel }}</button>
        <div
          v-if="openMenu === 'language'"
          class="editor-status-menu editor-status-menu--upward"
          data-testid="language-menu"
          role="menu"
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
