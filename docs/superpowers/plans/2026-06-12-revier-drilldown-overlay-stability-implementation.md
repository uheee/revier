# Revier 下钻窗口稳定覆盖 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 取消提交下钻窗口的过渡动画，并让下钻窗口稳定覆盖中间 diff 面板，不受文件级 diff 外层滚动影响。

**Architecture:** 保持现有 `ReviewWorkspace -> DiffDrilldownOverlay -> DiffViewer` 组件边界。下钻窗口改为即时渲染的覆盖层，滚动职责从 `review-diff-pane` 下沉到各自的 `DiffViewer`，避免覆盖层跟随文件级滚动内容移动。

**Tech Stack:** Vue 3、TypeScript、Pinia、Naive UI、Vitest、jsdom、pnpm、fnm Node 24。

---

## 文件结构

- 创建：`tests/unit/diffDrilldownOverlay.test.ts`
  - 验证 loading 到提交 overlay 切换时，下钻根容器不被替换。
- 创建：`tests/unit/reviewDiffPaneLayout.test.ts`
  - 验证中间栏和下钻窗口的滚动与覆盖 CSS 约束。
- 修改：`src/renderer/components/review/DiffDrilldownOverlay.vue`
  - 移除 `Transition`、根节点动态 key 和过渡依赖。
- 修改：`src/renderer/styles.css`
  - `review-diff-pane` 改为裁剪容器。
  - `diff-viewer` 改为内部滚动容器。
  - `diff-drilldown` 改为完整覆盖的非滚动 flex 容器。
  - 移除 `.drilldown-cover-*` 动画样式。

## 任务 1：用测试固定下钻容器不重建

**Files:**
- Create: `tests/unit/diffDrilldownOverlay.test.ts`

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/diffDrilldownOverlay.test.ts`：

```ts
// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import DiffDrilldownOverlay from '../../src/renderer/components/review/DiffDrilldownOverlay.vue';
import type { FileOverlay } from '../../src/shared/reviewTypes';

const commitOverlay: FileOverlay = {
  mode: 'commit',
  file: {
    path: 'src/app.ts',
    status: 'modified',
    additions: 1,
    deletions: 1,
    isBinary: false,
    isPreviewable: true
  },
  range: {
    branch: 'main',
    baseCommit: 'base',
    headCommit: 'head'
  },
  rows: [],
  blocks: [],
  warnings: [],
  commit: {
    hash: 'abc123',
    shortHash: 'abc123',
    authorName: 'Alice',
    authorEmail: 'alice@example.com',
    committedAt: '2026-06-12T00:00:00.000Z',
    subject: 'fix: 修复下钻闪烁',
    matchedByFilter: false,
    touchedRanges: []
  },
  parentHash: 'parent'
};

describe('DiffDrilldownOverlay', () => {
  it('keeps the drilldown container mounted when loading resolves to commit content', async () => {
    const wrapper = mount(DiffDrilldownOverlay, {
      props: {
        loading: true,
        overlay: undefined
      },
      global: {
        stubs: {
          'n-button': { template: '<button type="button"><slot /></button>' },
          'n-spin': { template: '<div><slot /></div>' },
          'n-empty': true,
          'n-alert': true,
          'n-tag': { template: '<span><slot /></span>' },
          'n-tooltip': { template: '<span><slot /></span>' }
        }
      }
    });

    const initialContainer = wrapper.get('.diff-drilldown').element;

    await wrapper.setProps({
      loading: false,
      overlay: commitOverlay
    });

    expect(wrapper.get('.diff-drilldown').element).toBe(initialContainer);
    expect(wrapper.text()).toContain('fix: 修复下钻闪烁');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/diffDrilldownOverlay.test.ts
```

预期：失败，原因是当前 `DiffDrilldownOverlay` 通过 keyed `Transition` 从 `loading` 切到提交 hash 时会替换 `.diff-drilldown` 根元素。

## 任务 2：用测试固定覆盖层和滚动边界

**Files:**
- Create: `tests/unit/reviewDiffPaneLayout.test.ts`

- [ ] **Step 1: 写失败测试**

创建 `tests/unit/reviewDiffPaneLayout.test.ts`：

```ts
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function readStyles(): string {
  return readFileSync(resolve(process.cwd(), 'src/renderer/styles.css'), 'utf8');
}

function expectRule(styles: string, selector: string, declarations: string[]): void {
  const escapedSelector = selector.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
  const ruleMatch = styles.match(new RegExp(`${escapedSelector}\\s*\\{(?<body>[^}]*)\\}`, 's'));
  expect(ruleMatch?.groups?.body, `缺少 ${selector} 样式规则`).toBeDefined();

  const body = ruleMatch?.groups?.body ?? '';
  for (const declaration of declarations) {
    expect(body).toContain(declaration);
  }
}

describe('review diff pane layout', () => {
  it('keeps the outer diff pane fixed while individual viewers scroll internally', () => {
    const styles = readStyles();

    expectRule(styles, '.review-diff-pane', ['overflow: hidden;']);
    expectRule(styles, '.diff-viewer', ['height: 100%;', 'overflow: auto;']);
    expectRule(styles, '.diff-drilldown', [
      'overflow: hidden;',
      'display: flex;',
      'flex-direction: column;'
    ]);
    expect(styles).not.toContain('.drilldown-cover-enter-active');
    expect(styles).not.toContain('.drilldown-cover-leave-active');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/reviewDiffPaneLayout.test.ts
```

预期：失败，原因是当前 `.review-diff-pane` 使用 `overflow: auto`，`.diff-viewer` 不承担完整内部滚动，`.diff-drilldown` 自己滚动且仍存在下钻过渡动画样式。

## 任务 3：实现即时覆盖的下钻窗口

**Files:**
- Modify: `src/renderer/components/review/DiffDrilldownOverlay.vue`

- [ ] **Step 1: 移除 Transition 和根节点 key**

把模板改为：

```vue
<template>
  <section v-if="overlay || loading" class="diff-drilldown">
    <header class="diff-drilldown__bar">
      <n-button text type="primary" @click="emit('close')">
        <ArrowLeft :size="16" aria-hidden="true" />
        <span>返回</span>
      </n-button>
      <div class="diff-drilldown__title">
        <code>{{ overlay?.commit?.shortHash ?? 'loading' }}</code>
        <span>{{ overlay?.commit?.subject ?? '加载提交变更' }}</span>
      </div>
    </header>
    <DiffViewer :overlay="overlay" :loading="loading" />
  </section>
</template>
```

- [ ] **Step 2: 运行下钻组件测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/diffDrilldownOverlay.test.ts
```

预期：通过。

## 任务 4：实现外层裁剪、内层滚动布局

**Files:**
- Modify: `src/renderer/styles.css`

- [ ] **Step 1: 调整中间栏与 DiffViewer 滚动职责**

将关键样式调整为：

```css
.review-diff-pane {
  position: relative;
  min-width: 0;
  min-height: 0;
  overflow: hidden;
  background: #f2f5f8;
}

.diff-viewer {
  height: 100%;
  min-height: 0;
  overflow: auto;
  padding: 18px;
}
```

- [ ] **Step 2: 调整下钻覆盖层**

将 `.diff-drilldown` 和内部滚动区域调整为：

```css
.diff-drilldown {
  position: absolute;
  inset: 0;
  z-index: 3;
  display: flex;
  flex-direction: column;
  min-height: 0;
  overflow: hidden;
  background: #f2f5f8;
  border-left: 1px solid var(--rv-border);
}

.diff-drilldown > .diff-viewer {
  flex: 1 1 auto;
}
```

- [ ] **Step 3: 移除下钻动画样式**

删除以下选择器对应规则：

```css
.drilldown-cover-enter-active
.drilldown-cover-enter-from
.drilldown-cover-enter-to
.drilldown-cover-leave-active
.drilldown-cover-leave-to
@media (prefers-reduced-motion: reduce) 中的 .drilldown-cover-* 引用
```

- [ ] **Step 4: 运行布局测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/reviewDiffPaneLayout.test.ts
```

预期：通过。

## 任务 5：完整验证和提交

**Files:**
- Verify: all changed files

- [ ] **Step 1: 运行目标测试**

运行：

```bash
fnm exec --using 24 pnpm vitest run tests/unit/diffDrilldownOverlay.test.ts tests/unit/reviewDiffPaneLayout.test.ts
```

预期：通过。

- [ ] **Step 2: 运行完整单元与集成测试**

运行：

```bash
fnm exec --using 24 pnpm test
```

预期：通过。

- [ ] **Step 3: 运行类型检查**

运行：

```bash
fnm exec --using 24 pnpm typecheck
```

预期：通过。

- [ ] **Step 4: 自我代码审阅**

检查：

- `DiffDrilldownOverlay.vue` 中没有 `Transition` 和根节点动态 key。
- `review-diff-pane` 没有外层滚动。
- 文件级和提交级 `DiffViewer` 都有独立滚动能力。
- 删除动画后没有残留 `.drilldown-cover-*` 样式。
- 关闭下钻仍通过 `close` 事件调用 store 的 `closeCommitDrilldown()`。

- [ ] **Step 5: 提交**

运行：

```bash
git add docs/superpowers/specs/2026-06-12-revier-drilldown-overlay-stability-design.md docs/superpowers/plans/2026-06-12-revier-drilldown-overlay-stability-implementation.md tests/unit/diffDrilldownOverlay.test.ts tests/unit/reviewDiffPaneLayout.test.ts src/renderer/components/review/DiffDrilldownOverlay.vue src/renderer/styles.css
git commit -m "fix: stabilize review drilldown overlay"
```
