# Revier 首页项目入口调整实施计划

> **面向执行代理：** 必须使用子技能 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 按任务逐项执行。本计划使用复选框 `- [ ]` 追踪进度。

**目标：** 将启动首页从左右两栏改为最近项目或 welcome 信息，并通过“打开项目”模式对话框添加项目后直接进入 review 工作台。

**架构：** 改动限定在渲染层首页入口。`ProjectHome.vue` 负责新布局、弹窗状态和添加后跳转；`ProjectEditor.vue` 继续负责项目表单，但增加可配置文案；`ProjectList.vue`、Pinia store、main/preload IPC 不改变公开接口。

**技术栈：** Vue 3、TypeScript、Pinia、Vue Router、Naive UI、lucide-vue-next、Playwright、Vitest、pnpm。

---

## 文件结构

- 修改：`tests/e2e/review-workflow.spec.ts`
  - 负责验证启动首页点击“打开项目”后出现弹窗、提交新项目后直接进入 review 工作台。
- 修改：`src/renderer/components/projects/ProjectEditor.vue`
  - 保持仓库路径和项目名称表单职责。
  - 增加 `title` 和 `submitLabel` 可选 props，默认值保持现有文案。
  - 给提交按钮增加稳定测试标识。
- 修改：`src/renderer/pages/ProjectHome.vue`
  - 移除左右两栏布局。
  - 增加“打开项目”按钮、welcome 空状态、模式对话框。
  - 添加项目成功后根据仓库路径定位项目并跳转 review。
- 修改：`src/renderer/styles.css`
  - 移除首页两栏样式的使用路径。
  - 增加单列内容区、空状态、弹窗表单和顶部操作区样式。

## 任务 1：先更新 E2E 测试，锁定新入口行为

**文件：**
- 修改：`tests/e2e/review-workflow.spec.ts`

- [ ] **步骤 1：替换 E2E 测试中的项目添加入口**

将 `tests/e2e/review-workflow.spec.ts` 中 `try` 块内从 `const page = await app.firstWindow();` 到进入筛选页断言前的内容替换为：

```ts
    const page = await app.firstWindow();
    await expect(page.getByRole('heading', { name: 'Revier' })).toBeVisible();
    await expect(page.getByPlaceholder('E:/Projects/revier')).toHaveCount(0);

    await page.locator('[data-test="open-project-dialog"]').first().click();
    const projectDialog = page.locator('[data-test="project-dialog"]');
    await expect(projectDialog).toBeVisible();

    const repoPathInput = projectDialog.getByPlaceholder('E:/Projects/revier');
    await expect(repoPathInput).toBeVisible();
    await repoPathInput.evaluate((element, value) => {
      const input = element as HTMLInputElement;
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }, process.cwd());
    await projectDialog.getByRole('textbox', { name: '项目名称' }).fill('Revier');
    await projectDialog.locator('[data-test="project-editor-submit"]').click();

    await expect(page.getByRole('heading', { name: '筛选' })).toBeVisible();
    await expect(page.locator('[data-test="branch-select"]')).toBeVisible();
    await expect(page.locator('[data-test="date-range"]')).toBeVisible();
```

替换后完整测试文件应为：

```ts
import { mkdtemp, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { _electron as electron, expect, test } from '@playwright/test';

test('project entry UI is interactive', async () => {
  const userDataDir = await mkdtemp(join(tmpdir(), 'revier-e2e-'));
  const env: Record<string, string> = {
    ...stringEnv(),
    ELECTRON_DISABLE_SECURITY_WARNINGS: 'true',
    REVIER_USER_DATA_DIR: userDataDir
  };
  delete env.ELECTRON_RUN_AS_NODE;

  const app = await electron.launch({
    args: [join(process.cwd(), 'dist/main/index.js')],
    env
  });

  try {
    const page = await app.firstWindow();
    await expect(page.getByRole('heading', { name: 'Revier' })).toBeVisible();
    await expect(page.getByPlaceholder('E:/Projects/revier')).toHaveCount(0);

    await page.locator('[data-test="open-project-dialog"]').first().click();
    const projectDialog = page.locator('[data-test="project-dialog"]');
    await expect(projectDialog).toBeVisible();

    const repoPathInput = projectDialog.getByPlaceholder('E:/Projects/revier');
    await expect(repoPathInput).toBeVisible();
    await repoPathInput.evaluate((element, value) => {
      const input = element as HTMLInputElement;
      input.value = value;
      input.dispatchEvent(new Event('input', { bubbles: true }));
    }, process.cwd());
    await projectDialog.getByRole('textbox', { name: '项目名称' }).fill('Revier');
    await projectDialog.locator('[data-test="project-editor-submit"]').click();

    await expect(page.getByRole('heading', { name: '筛选' })).toBeVisible();
    await expect(page.locator('[data-test="branch-select"]')).toBeVisible();
    await expect(page.locator('[data-test="date-range"]')).toBeVisible();

    await page.getByRole('button', { name: /分析/ }).click();
    await expect(page.locator('.diff-table--full')).toBeVisible({ timeout: 20000 });
  } finally {
    await app.close();
    await rm(userDataDir, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 });
  }
});

function stringEnv(): Record<string, string> {
  return Object.fromEntries(
    Object.entries(process.env).filter((entry): entry is [string, string] => typeof entry[1] === 'string')
  );
}
```

- [ ] **步骤 2：运行 E2E，确认当前实现不满足新行为**

运行：

```bash
pnpm test:e2e
```

预期：失败。失败点应指向 `[data-test="open-project-dialog"]` 不存在，或首页仍能直接找到 `E:/Projects/revier` 输入框。若失败原因是构建或环境问题，先记录错误输出，不改实现代码。

## 任务 2：调整 `ProjectEditor` 为弹窗复用提供文案配置

**文件：**
- 修改：`src/renderer/components/projects/ProjectEditor.vue`
- 验证：`tests/unit/projectEditor.test.ts`

- [ ] **步骤 1：用可配置文案替换硬编码标题和提交按钮文案**

将 `src/renderer/components/projects/ProjectEditor.vue` 替换为：

```vue
<script setup lang="ts">
import { FolderOpen } from 'lucide-vue-next';
import { computed, reactive } from 'vue';

const props = withDefaults(
  defineProps<{
    loading?: boolean;
    title?: string;
    submitLabel?: string;
  }>(),
  {
    title: '新增项目',
    submitLabel: '添加项目'
  }
);

const emit = defineEmits<{
  submit: [payload: { repoPath: string; name?: string }];
}>();

const form = reactive({
  repoPath: '',
  name: ''
});

const canSubmit = computed(() => form.repoPath.trim().length > 0 && !props.loading);

function submit(): void {
  if (!canSubmit.value) {
    return;
  }

  emit('submit', {
    repoPath: form.repoPath.trim(),
    name: form.name.trim() || undefined
  });
  form.repoPath = '';
  form.name = '';
}

async function selectDirectory(): Promise<void> {
  const selection = await window.revier.projects.selectDirectory();
  if (!selection) {
    return;
  }

  form.repoPath = selection.path;
  form.name = selection.name;
}
</script>

<template>
  <form class="project-editor" @submit.prevent="submit">
    <header class="panel-heading">
      <h2>{{ title }}</h2>
    </header>

    <label class="field">
      <span>仓库路径</span>
      <div class="path-picker">
        <n-input v-model:value="form.repoPath" placeholder="E:/Projects/revier" clearable @click="selectDirectory" />
        <n-button
          data-test="select-repo-directory"
          aria-label="选择仓库目录"
          @click="selectDirectory"
        >
          <FolderOpen :size="16" aria-hidden="true" />
        </n-button>
      </div>
    </label>

    <label class="field">
      <span>项目名称</span>
      <n-input v-model:value="form.name" placeholder="Revier" clearable />
    </label>

    <n-button
      class="project-editor__submit"
      data-test="project-editor-submit"
      type="primary"
      attr-type="submit"
      :disabled="!canSubmit"
    >
      <FolderOpen :size="16" aria-hidden="true" />
      <span>{{ submitLabel }}</span>
    </n-button>
  </form>
</template>
```

- [ ] **步骤 2：运行 ProjectEditor 单元测试**

运行：

```bash
pnpm test -- projectEditor
```

预期：通过。该测试验证目录选择仍能填充仓库路径和项目名称。

## 任务 3：重构 `ProjectHome` 为单列首页和打开项目弹窗

**文件：**
- 修改：`src/renderer/pages/ProjectHome.vue`

- [ ] **步骤 1：替换 `ProjectHome.vue` 脚本和模板**

将 `src/renderer/pages/ProjectHome.vue` 替换为：

```vue
<script setup lang="ts">
import { FolderOpen } from 'lucide-vue-next';
import { storeToRefs } from 'pinia';
import { computed, onMounted, ref } from 'vue';
import { useRouter } from 'vue-router';
import ProjectEditor from '../components/projects/ProjectEditor.vue';
import ProjectList from '../components/projects/ProjectList.vue';
import { useProjectStore } from '../stores/projectStore';

const router = useRouter();
const projectStore = useProjectStore();
const { projects, loading, error } = storeToRefs(projectStore);
const projectDialogVisible = ref(false);
const projectOpenError = ref<string>();
const dialogError = computed(() => projectOpenError.value ?? error.value);

onMounted(() => {
  void projectStore.loadProjects();
});

function openProjectDialog(): void {
  projectOpenError.value = undefined;
  projectDialogVisible.value = true;
}

async function addProjectAndOpen(payload: { repoPath: string; name?: string }): Promise<void> {
  projectOpenError.value = undefined;
  await projectStore.addProject(payload.repoPath, payload.name);

  if (projectStore.error) {
    return;
  }

  const project = projects.value.find(
    (candidate) => normalizeRepoPath(candidate.repoPath) === normalizeRepoPath(payload.repoPath)
  );
  if (!project) {
    projectOpenError.value = '项目已添加，但未能定位项目记录，请刷新后重试';
    return;
  }

  projectDialogVisible.value = false;
  await router.push({ name: 'review', params: { projectId: project.id } });
}

function removeProject(projectId: string): void {
  void projectStore.removeProject(projectId);
}

function openProject(projectId: string): void {
  void router.push({ name: 'review', params: { projectId } });
}

function normalizeRepoPath(repoPath: string): string {
  return repoPath.replaceAll('\\', '/').replace(/\/+$/, '').toLowerCase();
}
</script>

<template>
  <main class="project-home">
    <header class="project-topbar">
      <div class="project-brand">
        <h1>Revier</h1>
        <span>{{ projects.length }} 个项目</span>
      </div>
      <div class="project-topbar__actions">
        <n-button :loading="loading" @click="projectStore.loadProjects()">刷新</n-button>
        <n-button
          data-test="open-project-dialog"
          type="primary"
          :disabled="loading"
          @click="openProjectDialog"
        >
          <FolderOpen :size="16" aria-hidden="true" />
          <span>打开项目</span>
        </n-button>
      </div>
    </header>

    <n-alert
      v-if="error"
      class="project-alert"
      :title="error"
      type="error"
      show-icon
      :closable="false"
    />

    <section class="project-content">
      <section v-if="projects.length > 0 || loading" class="project-main-panel">
        <ProjectList
          :projects="projects"
          :loading="loading"
          @open="openProject"
          @remove="removeProject"
        />
      </section>

      <section v-else class="project-empty" aria-label="欢迎">
        <div class="project-empty__content">
          <h2>还没有项目</h2>
          <p>打开一个本地 Git 仓库，开始查看提交范围和代码差异。</p>
          <n-button data-test="open-project-dialog" type="primary" @click="openProjectDialog">
            <FolderOpen :size="16" aria-hidden="true" />
            <span>打开项目</span>
          </n-button>
        </div>
      </section>
    </section>

    <n-modal
      v-model:show="projectDialogVisible"
      class="project-dialog"
      preset="card"
      title="打开项目"
      :bordered="false"
      :mask-closable="!loading"
    >
      <div data-test="project-dialog">
        <n-alert
          v-if="dialogError"
          class="project-dialog__alert"
          :title="dialogError"
          type="error"
          show-icon
          :closable="false"
        />
        <ProjectEditor
          :loading="loading"
          title="仓库信息"
          submit-label="打开项目"
          @submit="addProjectAndOpen"
        />
      </div>
    </n-modal>
  </main>
</template>
```

- [ ] **步骤 2：运行类型检查，确认组件代码类型正确**

运行：

```bash
pnpm typecheck
```

预期：通过。若 TypeScript 报告 `replaceAll`、Naive UI attr 或路由参数相关错误，按报错位置修正当前文件，不改 store 公开接口。

## 任务 4：调整首页样式为单列列表和弹窗表单

**文件：**
- 修改：`src/renderer/styles.css`

- [ ] **步骤 1：替换首页相关样式块**

在 `src/renderer/styles.css` 中，将 `.project-alert` 后到 `.project-actions` 前的首页项目区样式替换为：

```css
.project-alert {
  margin-bottom: 16px;
}

.project-topbar__actions {
  display: inline-flex;
  align-items: center;
  gap: 10px;
  flex: 0 0 auto;
}

.project-content {
  display: flex;
  height: calc(100vh - 92px);
  min-height: 0;
}

.project-main-panel,
.project-empty {
  min-width: 0;
  width: 100%;
  background: var(--rv-surface);
  border: 1px solid var(--rv-border);
  border-radius: 8px;
  box-shadow: var(--rv-shadow);
}

.project-main-panel {
  overflow: hidden;
}

.project-empty {
  display: grid;
  place-items: center;
  min-height: 360px;
  padding: 48px 24px;
}

.project-empty__content {
  display: grid;
  justify-items: center;
  gap: 12px;
  max-width: 420px;
  text-align: center;
}

.project-empty h2 {
  margin: 0;
  color: var(--rv-text);
  font-size: 18px;
  font-weight: 600;
}

.project-empty p {
  margin: 0 0 4px;
  color: var(--rv-muted);
  font-size: 13px;
  line-height: 20px;
}

.project-dialog {
  width: min(520px, calc(100vw - 32px));
}

.project-dialog__alert {
  margin-bottom: 16px;
}

.panel-heading {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  margin-bottom: 18px;
}

.panel-heading h2 {
  margin: 0;
  color: var(--rv-text);
  font-size: 12px;
  font-weight: 600;
  letter-spacing: 0;
}

.project-editor {
  display: flex;
  flex-direction: column;
  gap: 18px;
}

.field {
  display: grid;
  gap: 8px;
  color: var(--rv-text);
  font-size: 12px;
  font-weight: 500;
}

.path-picker {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 36px;
  gap: 8px;
}

.path-picker .n-button {
  width: 36px;
  padding: 0;
}

.project-editor__submit {
  width: 100%;
  gap: 8px;
}

.project-list {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.project-list__heading {
  flex: 0 0 auto;
  margin: 0;
  padding: 18px 20px;
  background: var(--rv-surface-subtle);
  border-bottom: 1px solid var(--rv-border-soft);
}

.project-table {
  flex: 1 1 auto;
  min-height: 0;
  font-size: 13px;
}

.project-table .n-data-table-base-table-header {
  font-size: 12px;
}

.project-name-cell {
  display: flex;
  align-items: center;
  gap: 8px;
  min-width: 0;
}

.pin-dot {
  width: 7px;
  height: 7px;
  flex: 0 0 auto;
  border-radius: 999px;
  background: #0f766e;
}

.path-cell {
  display: block;
  overflow: hidden;
  color: var(--rv-muted);
  font-family: "JetBrains Mono", "SFMono-Regular", Consolas, "Liberation Mono", monospace;
  font-size: 12px;
  text-overflow: ellipsis;
  white-space: nowrap;
}
```

保留 `.project-actions` 及其后的 review 工作台样式。

- [ ] **步骤 2：替换窄屏首页样式**

在 `@media (max-width: 860px)` 内，将当前首页相关规则替换为：

```css
  .project-home {
    height: auto;
    overflow: auto;
    padding: 16px;
  }

  .project-topbar {
    align-items: flex-start;
    flex-direction: column;
  }

  .project-topbar__actions {
    width: 100%;
  }

  .project-topbar__actions .n-button {
    flex: 1 1 0;
  }

  .project-content {
    height: auto;
    min-height: 420px;
  }

  .project-main-panel {
    min-height: 420px;
  }

  .project-empty {
    min-height: 360px;
    padding: 32px 20px;
  }
```

删除该媒体查询中旧的 `.project-layout` 规则。

- [ ] **步骤 3：运行类型检查和 ProjectEditor 单元测试**

运行：

```bash
pnpm typecheck
pnpm test -- projectEditor
```

预期：两条命令均通过。

## 任务 5：跑完整验证并提交实现

**文件：**
- 验证：`src/renderer/pages/ProjectHome.vue`
- 验证：`src/renderer/components/projects/ProjectEditor.vue`
- 验证：`src/renderer/styles.css`
- 验证：`tests/e2e/review-workflow.spec.ts`

- [ ] **步骤 1：运行关键验证命令**

运行：

```bash
pnpm typecheck
pnpm test -- projectEditor
pnpm test:e2e
```

预期：

- `pnpm typecheck` 通过。
- `pnpm test -- projectEditor` 通过。
- `pnpm test:e2e` 通过，并验证新项目通过弹窗提交后直接进入筛选页。

- [ ] **步骤 2：检查工作区变更**

运行：

```bash
git status --short
```

预期只包含以下文件：

```text
M  src/renderer/components/projects/ProjectEditor.vue
M  src/renderer/pages/ProjectHome.vue
M  src/renderer/styles.css
M  tests/e2e/review-workflow.spec.ts
```

若包含构建产物、测试结果或无关文件，不提交这些文件。

- [ ] **步骤 3：提交实现**

运行：

```bash
git add src/renderer/components/projects/ProjectEditor.vue src/renderer/pages/ProjectHome.vue src/renderer/styles.css tests/e2e/review-workflow.spec.ts
git commit -m "feat: 调整首页项目打开入口"
```

预期：生成一个语义化提交，提交范围只包含本计划列出的实现和测试文件。

## 计划自检

- 设计文档中“启动后不再使用左右两栏”由任务 3 和任务 4 覆盖。
- 设计文档中“最近项目或 welcome 信息”由任务 3 和任务 4 覆盖。
- 设计文档中“点击打开项目后弹出模式对话框”由任务 1、任务 3 和任务 4 覆盖。
- 设计文档中“添加项目成功后直接进入 review 工作台”由任务 1 和任务 3 覆盖。
- 设计文档中“不修改 store 公开接口”由任务 3 的实现约束覆盖。
- 计划没有未完成占位词。
- 计划中的组件 props、事件名、测试标识和路径在各任务之间保持一致。
