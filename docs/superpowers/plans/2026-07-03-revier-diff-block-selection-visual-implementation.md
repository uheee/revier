# Revier diff 代码块选中态视觉强化 Implementation Plan

> **给 agentic workers：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 按任务执行本计划。所有步骤使用 checkbox（`- [ ]`）语法跟踪。

**目标：** 让选中的 diff 变更块在左侧旧代码区域显示深红连续标记、在右侧新代码区域显示深绿连续标记，并按整个变更块高度连续呈现。

**架构：** 保持现有 `DiffViewer.vue` 行级渲染结构不变，通过新增块首/块尾 class 暴露选中块边界。CSS 基于行类型、选中态和首尾 class 在左右代码单元格边缘绘制深色伪元素，不修改 diff 数据结构和 IPC 接口。

**技术栈：** Vue 3 `<script setup>`、TypeScript、全局 CSS、Vitest、Vue Test Utils、`pnpm`。

---

## 范围检查

本计划只覆盖一个前端渲染子系统：Review 工作台中间栏 diff 代码块选中态。已确认设计文档为：

- `docs/superpowers/specs/2026-07-03-revier-diff-block-selection-visual-design.md`

本计划不拆分为多个子计划，因为本次变更只涉及一个组件、一个样式文件和对应单元测试。

## 文件结构

- 修改：`tests/unit/diffViewer.test.ts`
  - 职责：验证 `DiffViewer.vue` 在选中多行变更块时，为块首、块中、块尾行输出正确 class，并保留点击 emit 行为。
- 创建：`tests/unit/diffViewerSelectionStyles.test.ts`
  - 职责：以样式契约测试锁定选中态 CSS 规则，避免回退成单侧绿色细线。
- 修改：`src/renderer/components/review/DiffViewer.vue`
  - 职责：新增 `isBlockEnd` 辅助函数，并在选中行 class 中暴露 `is-block-start`、`is-block-end`。
- 修改：`src/renderer/styles.css`
  - 职责：移除选中行单条绿色左线，改为红/绿侧代码单元格边缘的深色连续标记。

## 执行前检查

- [ ] **Step 1: 确认工作区干净**

运行：

```powershell
git status --short
```

预期：没有输出。若出现与本计划无关的用户改动，记录文件名并避免触碰。

- [ ] **Step 2: 激活 Node 24**

运行：

```powershell
fnm use 24
```

预期：命令成功，当前 shell 可以调用 `node`、`pnpm`。如果当前环境找不到 `fnm`，停止实现并让用户先修复 Node 环境。

---

### Task 1: 为选中块边界 class 写失败测试

**Files:**
- Modify: `tests/unit/diffViewer.test.ts`

- [ ] **Step 1: 编写失败测试**

将 `tests/unit/diffViewer.test.ts` 中的 `overlay` 调整为三行同一变更块，并新增选中态 class 断言。完整文件内容如下：

```ts
// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import DiffViewer from '../../src/renderer/components/review/DiffViewer.vue';
import type { FileOverlay } from '../../src/shared/reviewTypes';

const overlay: FileOverlay = {
  mode: 'range',
  file: {
    path: 'src/app.ts',
    status: 'modified',
    additions: 3,
    deletions: 3,
    isBinary: false,
    isPreviewable: true
  },
  range: {
    branch: 'main',
    baseCommit: 'base',
    headCommit: 'head'
  },
  rows: [
    {
      oldLineNumber: 1,
      newLineNumber: 1,
      oldText: 'same',
      newText: 'same',
      type: 'context'
    },
    {
      oldLineNumber: 2,
      newLineNumber: 2,
      oldText: 'old one',
      newText: 'new one',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 3,
      newLineNumber: 3,
      oldText: 'old two',
      newText: 'new two',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 4,
      newLineNumber: 4,
      oldText: 'old three',
      newText: 'new three',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 5,
      newLineNumber: 5,
      oldText: 'after',
      newText: 'after',
      type: 'context'
    }
  ],
  blocks: [
    {
      id: 'block-1',
      oldStart: 2,
      oldEnd: 4,
      newStart: 2,
      newEnd: 4,
      rowStartIndex: 1,
      rowEndIndex: 3,
      changeType: 'modified',
      authors: [{ name: 'Alice', email: 'alice@example.com' }],
      rows: [],
      relatedCommits: []
    }
  ],
  warnings: []
};

function mountDiffViewer(props: { selectedBlockId?: string } = {}) {
  return mount(DiffViewer, {
    props: { overlay, ...props },
    global: {
      directives: {
        loading: {}
      },
      stubs: {
        'n-spin': { template: '<div><slot /></div>' },
        'n-empty': true,
        'n-alert': true,
        'n-tag': { template: '<span><slot /></span>' },
        'n-tooltip': { template: '<span><slot /></span>' }
      }
    }
  });
}

describe('DiffViewer', () => {
  it('renders full file rows and emits selected block when a changed row is clicked', async () => {
    const wrapper = mountDiffViewer();

    expect(wrapper.text()).toContain('same');
    expect(wrapper.text()).toContain('after');

    await wrapper.get('[data-block-id="block-1"]').trigger('click');
    expect(wrapper.emitted('selected')?.[0]?.[0]).toEqual(overlay.blocks[0]);
  });

  it('marks selected block rows with continuous block boundary classes', () => {
    const wrapper = mountDiffViewer({ selectedBlockId: 'block-1' });

    const selectedRows = wrapper.findAll('[data-block-id="block-1"]');
    expect(selectedRows).toHaveLength(3);

    expect(selectedRows[0].classes()).toContain('is-selected');
    expect(selectedRows[0].classes()).toContain('is-block-start');
    expect(selectedRows[0].classes()).not.toContain('is-block-end');

    expect(selectedRows[1].classes()).toContain('is-selected');
    expect(selectedRows[1].classes()).not.toContain('is-block-start');
    expect(selectedRows[1].classes()).not.toContain('is-block-end');

    expect(selectedRows[2].classes()).toContain('is-selected');
    expect(selectedRows[2].classes()).not.toContain('is-block-start');
    expect(selectedRows[2].classes()).toContain('is-block-end');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/diffViewer.test.ts
```

预期：测试失败，失败信息包含 `expected [...] to include 'is-block-start'` 或 `expected [...] to include 'is-block-end'`。如果失败原因是 TypeScript、导入路径或挂载错误，先修正测试本身再继续。

- [ ] **Step 3: 提交失败测试**

运行：

```powershell
git add tests/unit/diffViewer.test.ts
git commit -m "test: 覆盖 diff 选中块边界类"
```

预期：提交成功。该提交只包含失败测试。

---

### Task 2: 实现选中块首尾 class

**Files:**
- Modify: `src/renderer/components/review/DiffViewer.vue`
- Test: `tests/unit/diffViewer.test.ts`

- [ ] **Step 1: 编写最小实现**

在 `src/renderer/components/review/DiffViewer.vue` 中新增 `isBlockEnd`，并在行 class 对象中追加选中块首尾 class。完整相关片段如下：

```vue
<script setup lang="ts">
import { computed } from 'vue';
import type { DiffBlock, FileOverlay, SideBySideDiffRow, WordChange } from '../../../shared/reviewTypes';
import DiffBlockAuthors from './DiffBlockAuthors.vue';

const props = defineProps<{
  overlay?: FileOverlay;
  selectedBlockId?: string;
  loading?: boolean;
}>();

const emit = defineEmits<{
  selected: [block: DiffBlock];
}>();

function lineClass(row: SideBySideDiffRow): string {
  return `diff-row diff-row--${row.type}`;
}

const visibleRows = computed(() => props.overlay?.rows ?? props.overlay?.blocks.flatMap((block) => block.rows) ?? []);
const blocksById = computed(() => new Map((props.overlay?.blocks ?? []).map((block) => [block.id, block])));

function selectRow(row: SideBySideDiffRow): void {
  if (!row.blockId) {
    return;
  }

  const block = blocksById.value.get(row.blockId);
  if (block) {
    emit('selected', block);
  }
}

function blockForRow(row: SideBySideDiffRow): DiffBlock | undefined {
  return row.blockId ? blocksById.value.get(row.blockId) : undefined;
}

function isBlockStart(row: SideBySideDiffRow, index: number): boolean {
  const block = blockForRow(row);
  return Boolean(block && block.rowStartIndex === index);
}

function isBlockEnd(row: SideBySideDiffRow, index: number): boolean {
  const block = blockForRow(row);
  return Boolean(block && block.rowEndIndex === index);
}

function visibleWordChanges(row: SideBySideDiffRow, side: 'old' | 'new'): WordChange[] {
  if (!row.wordChanges) {
    return [{ value: side === 'old' ? (row.oldText ?? '') : (row.newText ?? '') }];
  }

  return row.wordChanges.filter((change) => (side === 'old' ? !change.added : !change.removed));
}
</script>
```

将模板中 diff 行的 `:class` 对象改为：

```vue
:class="[
  lineClass(row),
  {
    'is-clickable': row.blockId,
    'is-selected': row.blockId === selectedBlockId,
    'is-block-start': row.blockId === selectedBlockId && isBlockStart(row, index),
    'is-block-end': row.blockId === selectedBlockId && isBlockEnd(row, index)
  }
]"
```

- [ ] **Step 2: 运行测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/diffViewer.test.ts
```

预期：`tests/unit/diffViewer.test.ts` 全部通过。

- [ ] **Step 3: 提交实现**

运行：

```powershell
git add src/renderer/components/review/DiffViewer.vue tests/unit/diffViewer.test.ts
git commit -m "feat: 标记 diff 选中块首尾行"
```

预期：提交成功，包含 `DiffViewer.vue` 与对应测试。

---

### Task 3: 为红绿侧选中样式写失败测试

**Files:**
- Create: `tests/unit/diffViewerSelectionStyles.test.ts`

- [ ] **Step 1: 新增样式契约测试**

创建 `tests/unit/diffViewerSelectionStyles.test.ts`，完整内容如下：

```ts
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';

function readStyles(): string {
  return readFileSync(resolve(process.cwd(), 'src/renderer/styles.css'), 'utf8');
}

function expectRule(styles: string, selector: string, declarations: string[]): void {
  const rules = styles.matchAll(/(?<selectors>[^{}]+)\{(?<body>[^}]*)\}/g);
  const rule = Array.from(rules).find((item) => {
    const selectors = item.groups?.selectors.split(',').map((selectorText) => selectorText.trim()) ?? [];
    return selectors.includes(selector);
  });

  expect(rule?.groups?.body, `缺少 ${selector} 样式规则`).toBeDefined();
  const body = rule?.groups?.body ?? '';

  for (const declaration of declarations) {
    expect(body).toContain(declaration);
  }
}

describe('diff viewer selection styles', () => {
  it('uses side-specific continuous markers for selected diff blocks', () => {
    const styles = readStyles();

    expectRule(styles, '.diff-row.is-selected', ['box-shadow: none;']);
    expectRule(styles, '.diff-row.is-selected.diff-row--modified .code-cell--old', ['position: relative;']);
    expectRule(styles, '.diff-row.is-selected.diff-row--modified .code-cell--new', ['position: relative;']);

    expectRule(styles, '.diff-row.is-selected.diff-row--modified .code-cell--old::before', [
      'background: #9f1d18;',
      'left: 0;',
      'width: 6px;'
    ]);
    expectRule(styles, '.diff-row.is-selected.diff-row--deleted .code-cell--old::before', [
      'background: #9f1d18;',
      'left: 0;',
      'width: 6px;'
    ]);
    expectRule(styles, '.diff-row.is-selected.diff-row--modified .code-cell--new::after', [
      'background: #155f34;',
      'right: 0;',
      'width: 6px;'
    ]);
    expectRule(styles, '.diff-row.is-selected.diff-row--added .code-cell--new::after', [
      'background: #155f34;',
      'right: 0;',
      'width: 6px;'
    ]);

    expectRule(styles, '.diff-row.is-selected.is-block-start.diff-row--modified .code-cell--old::before', [
      'border-top-left-radius: 4px;',
      'border-top-right-radius: 4px;'
    ]);
    expectRule(styles, '.diff-row.is-selected.is-block-end.diff-row--modified .code-cell--new::after', [
      'border-bottom-left-radius: 4px;',
      'border-bottom-right-radius: 4px;'
    ]);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/diffViewerSelectionStyles.test.ts
```

预期：测试失败，失败信息包含 `缺少 .diff-row.is-selected.diff-row--modified .code-cell--old 样式规则` 或 `expected ... to contain box-shadow: none;`。

- [ ] **Step 3: 提交失败测试**

运行：

```powershell
git add tests/unit/diffViewerSelectionStyles.test.ts
git commit -m "test: 覆盖 diff 选中态红绿侧样式"
```

预期：提交成功。该提交只包含失败测试。

---

### Task 4: 实现红绿侧连续选中样式

**Files:**
- Modify: `src/renderer/styles.css`
- Test: `tests/unit/diffViewerSelectionStyles.test.ts`

- [ ] **Step 1: 替换现有选中态样式**

在 `src/renderer/styles.css` 中找到现有规则：

```css
.diff-row.is-selected {
  box-shadow: inset 3px 0 0 var(--rv-accent);
}
```

将其替换为以下完整样式块：

```css
.diff-row.is-selected {
  box-shadow: none;
}

.diff-row.is-selected.diff-row--modified .code-cell--old,
.diff-row.is-selected.diff-row--deleted .code-cell--old,
.diff-row.is-selected.diff-row--modified .code-cell--new,
.diff-row.is-selected.diff-row--added .code-cell--new {
  position: relative;
}

.diff-row.is-selected.diff-row--modified .code-cell--old::before,
.diff-row.is-selected.diff-row--deleted .code-cell--old::before,
.diff-row.is-selected.diff-row--modified .code-cell--new::after,
.diff-row.is-selected.diff-row--added .code-cell--new::after {
  position: absolute;
  top: 0;
  bottom: 0;
  z-index: 1;
  width: 6px;
  pointer-events: none;
  content: "";
}

.diff-row.is-selected.diff-row--modified .code-cell--old::before,
.diff-row.is-selected.diff-row--deleted .code-cell--old::before {
  left: 0;
  background: #9f1d18;
}

.diff-row.is-selected.diff-row--modified .code-cell--new::after,
.diff-row.is-selected.diff-row--added .code-cell--new::after {
  right: 0;
  background: #155f34;
}

.diff-row.is-selected.is-block-start.diff-row--modified .code-cell--old::before,
.diff-row.is-selected.is-block-start.diff-row--deleted .code-cell--old::before,
.diff-row.is-selected.is-block-start.diff-row--modified .code-cell--new::after,
.diff-row.is-selected.is-block-start.diff-row--added .code-cell--new::after {
  border-top-left-radius: 4px;
  border-top-right-radius: 4px;
}

.diff-row.is-selected.is-block-end.diff-row--modified .code-cell--old::before,
.diff-row.is-selected.is-block-end.diff-row--deleted .code-cell--old::before,
.diff-row.is-selected.is-block-end.diff-row--modified .code-cell--new::after,
.diff-row.is-selected.is-block-end.diff-row--added .code-cell--new::after {
  border-bottom-left-radius: 4px;
  border-bottom-right-radius: 4px;
}
```

- [ ] **Step 2: 运行样式测试确认通过**

运行：

```powershell
pnpm test -- tests/unit/diffViewerSelectionStyles.test.ts
```

预期：`tests/unit/diffViewerSelectionStyles.test.ts` 通过。

- [ ] **Step 3: 运行 diff viewer 组件测试确认未破坏行为**

运行：

```powershell
pnpm test -- tests/unit/diffViewer.test.ts
```

预期：`tests/unit/diffViewer.test.ts` 通过。

- [ ] **Step 4: 提交样式实现**

运行：

```powershell
git add src/renderer/styles.css tests/unit/diffViewerSelectionStyles.test.ts
git commit -m "feat: 强化 diff 选中块红绿侧提示"
```

预期：提交成功，包含 CSS 实现与样式测试。

---

### Task 5: 收尾验证

**Files:**
- Verify: `src/renderer/components/review/DiffViewer.vue`
- Verify: `src/renderer/styles.css`
- Verify: `tests/unit/diffViewer.test.ts`
- Verify: `tests/unit/diffViewerSelectionStyles.test.ts`

- [ ] **Step 1: 运行目标单元测试**

运行：

```powershell
pnpm test -- tests/unit/diffViewer.test.ts tests/unit/diffViewerSelectionStyles.test.ts
```

预期：两个测试文件全部通过。

- [ ] **Step 2: 运行类型检查**

运行：

```powershell
pnpm typecheck
```

预期：TypeScript 和 Vue 类型检查通过。

- [ ] **Step 3: 检查最终 diff**

运行：

```powershell
git diff -- src/renderer/components/review/DiffViewer.vue src/renderer/styles.css tests/unit/diffViewer.test.ts tests/unit/diffViewerSelectionStyles.test.ts
```

预期：没有未提交 diff。如果执行计划时选择合并提交而不是逐任务提交，则预期只包含本计划列出的四个文件。

- [ ] **Step 4: 检查工作区状态**

运行：

```powershell
git status --short
```

预期：没有输出。若存在未提交文件，只能是本计划列出的四个文件；处理完成后再交付。

## 视觉验收说明

当前设计阶段记录过环境限制：当前 `pwsh` 会话无法直接访问 `node`/`fnm`，默认 `bash` 指向不可用 WSL，因此视觉 companion 未能启动。执行本计划时如果 Node 环境已恢复，可额外运行：

```powershell
pnpm dev
```

随后在 Review 工作台手动点击新增、删除、修改三类变更块，确认：

- 新增块只在右侧显示深绿色连续标记。
- 删除块只在左侧显示深红色连续标记。
- 修改块左侧显示深红、右侧显示深绿。
- 多行块首尾圆角正确，中间行无断裂感。
- 右侧详情仍跟随当前选中块变化。

## 自检结果

- 设计文档目标均有任务覆盖：块首/块尾 class 由 Task 1 和 Task 2 覆盖；红绿侧样式由 Task 3 和 Task 4 覆盖；验证由 Task 5 覆盖。
- 本计划不修改 IPC、Pinia、共享类型、分析流程和数据结构。
- 类型名称与现有代码一致：`FileOverlay`、`DiffBlock`、`SideBySideDiffRow`、`WordChange`、`selectedBlockId`、`rowStartIndex`、`rowEndIndex`。
- 测试命令使用项目现有 `pnpm test` 与 `pnpm typecheck` 脚本。
