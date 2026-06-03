# Revier Review 工作台交互改进 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.
>
> **给自动化执行者：** 实施本计划时必须按任务逐项执行并在每个任务后验证。若执行中出现“先实现简单版本，之后再完善”的取舍，代码中必须写明 `TODO:`，并说明触发条件。

**Goal:** 按已确认 spec 补齐项目目录选择与校验、结构化筛选、完整文件 side-by-side diff、三栏宽度调整、提交下钻交互。

**Architecture:** 主进程继续负责本地目录、Git 仓库、提交和 diff 数据；preload 暴露受控 typed IPC；renderer 通过 Pinia 管理任务、筛选元数据、完整文件 overlay 和下钻状态。中间栏统一使用同一套 diff 行渲染组件，默认展示筛选范围 overlay，下钻时以覆盖层展示单提交 overlay。

**Tech Stack:** Vue 3、TypeScript、Electron、electron-vite、Pinia、Element Plus、lucide-vue-next、simple-git、diff、Vitest、Playwright、pnpm、fnm Node 24。

---

## 文件结构

本轮改动涉及以下文件：

- 修改：`src/shared/projectTypes.ts`
  - 新增目录选择返回类型。
  - 明确 `RepositoryValidation.repoPath` 在成功时为 Git root。
- 修改：`src/shared/reviewTypes.ts`
  - 将作者筛选改为结构化 `authorKeys`。
  - 新增作者候选、完整 overlay 行序列、提交下钻 request/response 类型。
- 修改：`src/shared/ipcTypes.ts`
  - 新增 `projects.selectDirectory`、`review.listAuthors`、`review.getCommitOverlay` typed API。
- 修改：`src/preload/index.ts`
  - 暴露新增 IPC 方法。
- 修改：`src/main/git/gitService.ts`
  - 新增 Git root 解析、作者列表、第一父提交、单提交 patch 检测方法。
- 修改：`src/main/projects/projectStore.ts`
  - 保存规范化 Git root。
  - 拒绝重复仓库。
- 修改：`src/main/ipc/projectsIpc.ts`
  - 新增目录选择 dialog。
  - 新增项目时先验证 Git 仓库并使用 root 路径入库。
- 修改：`src/main/ipc/reviewIpc.ts`
  - 支持结构化作者筛选。
  - 新增作者候选 IPC。
  - 新增单提交 overlay 构建与 IPC。
- 修改：`src/main/analysis/overlayEngine.ts`
  - 生成完整 `rows`，并给每个 `DiffBlock` 标记 `rowStartIndex` 和 `rowEndIndex`。
- 修改：`src/renderer/stores/projectStore.ts`
  - 新增目录选择 action。
  - 保持新增项目错误信息。
- 修改：`src/renderer/stores/reviewStore.ts`
  - 新增作者候选、提交下钻 overlay、选中提交状态。
- 修改：`src/renderer/components/projects/ProjectEditor.vue`
  - 仓库路径输入支持目录选择按钮。
  - 选择目录后自动填项目名称。
- 修改：`src/renderer/components/review/FilterPanel.vue`
  - 分支下拉、作者多选、日期范围选择器。
- 修改：`src/renderer/components/review/DiffViewer.vue`
  - 从按块渲染改为完整行渲染。
  - 高亮行映射到块，点击行选中块。
- 修改：`src/renderer/components/review/BlockDetailPanel.vue`
  - 提交列表可点击并发出下钻事件。
- 创建：`src/renderer/components/review/ReviewLayoutResizer.vue`
  - 两个拖拽分隔条。
- 创建：`src/renderer/composables/useReviewLayoutSizes.ts`
  - 三栏宽度状态、拖拽、localStorage 持久化。
- 创建：`src/renderer/components/review/DiffDrilldownOverlay.vue`
  - 提交下钻覆盖层、返回按钮、右向左动画。
- 修改：`src/renderer/pages/ReviewWorkspace.vue`
  - 接入结构化筛选元数据、三栏宽度、提交下钻。
- 修改：`src/renderer/styles.css`
  - 新增完整 diff 表格、拖拽分隔条、下钻覆盖层动画样式。
- 修改：`tests/unit/sharedTypes.test.ts`
- 修改：`tests/unit/projectStore.test.ts`
- 修改：`tests/integration/gitService.test.ts`
- 修改：`tests/unit/reviewIpc.test.ts`
- 修改：`tests/unit/overlayEngine.test.ts`
- 修改：`tests/unit/rendererProjectStore.test.ts`
- 修改：`tests/unit/rendererReviewStore.test.ts`
- 创建：`tests/unit/projectEditor.test.ts`
- 创建：`tests/unit/filterPanel.test.ts`
- 创建：`tests/unit/diffViewer.test.ts`
- 创建：`tests/unit/blockDetailPanel.test.ts`
- 创建：`tests/unit/reviewLayoutSizes.test.ts`
- 修改：`tests/e2e/review-workflow.spec.ts`

## 任务 1：共享类型与 IPC 合约

**Files:**
- Modify: `src/shared/projectTypes.ts`
- Modify: `src/shared/reviewTypes.ts`
- Modify: `src/shared/ipcTypes.ts`
- Modify: `src/preload/index.ts`
- Modify: `tests/unit/sharedTypes.test.ts`

- [ ] **Step 1: 写失败测试**

在 `tests/unit/sharedTypes.test.ts` 中把 `ReviewFilters` 用例改为结构化作者筛选，并增加提交 overlay request 的类型用例：

```ts
import { createAppError, isRecoverableError } from '../../src/shared/errors';
import type { ReviewFilters, ReviewProject } from '../../src/shared/ipcTypes';
import type {
  AuthorFilterOption,
  CommitOverlayRequest,
  FileOverlay,
  SideBySideDiffRow
} from '../../src/shared/reviewTypes';

describe('shared contracts', () => {
  it('creates recoverable and blocking app errors', () => {
    const timeout = createAppError('GIT_COMMAND_TIMEOUT', 'Git timed out', true);
    const missingRepo = createAppError('REPOSITORY_NOT_FOUND', 'Repository missing', false);

    expect(isRecoverableError(timeout)).toBe(true);
    expect(isRecoverableError(missingRepo)).toBe(false);
  });

  it('allows project preferences and structured review filters used by IPC', () => {
    const project: ReviewProject = {
      id: 'project-1',
      name: 'demo',
      repoPath: 'E:/repos/demo',
      pinned: true,
      preferences: {
        defaultBranch: 'main',
        defaultDays: 30,
        defaultGlobRules: ['src/**/*.ts', '!**/*.test.ts']
      }
    };

    const alice: AuthorFilterOption = {
      key: 'alice@example.com',
      name: 'Alice',
      email: 'alice@example.com',
      commitCount: 3
    };

    const filters: ReviewFilters = {
      projectId: project.id,
      branch: 'main',
      authorKeys: [alice.key],
      messageQuery: 'review',
      globRules: project.preferences.defaultGlobRules
    };

    expect(filters.authorKeys).toEqual(['alice@example.com']);
    expect(filters.globRules).toContain('!**/*.test.ts');
  });

  it('supports full-file overlay rows and commit drilldown requests', () => {
    const row: SideBySideDiffRow = {
      oldLineNumber: 1,
      newLineNumber: 1,
      oldText: 'old',
      newText: 'new',
      type: 'modified',
      blockId: 'block-1'
    };

    const overlay: FileOverlay = {
      mode: 'range',
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
      rows: [row],
      blocks: [
        {
          id: 'block-1',
          oldStart: 1,
          oldEnd: 1,
          newStart: 1,
          newEnd: 1,
          rowStartIndex: 0,
          rowEndIndex: 0,
          changeType: 'modified',
          authors: [],
          rows: [row],
          relatedCommits: []
        }
      ],
      warnings: []
    };

    const request: CommitOverlayRequest = {
      taskId: 'task-1',
      filePath: 'src/app.ts',
      commitHash: 'abc123'
    };

    expect(overlay.rows[0].blockId).toBe('block-1');
    expect(request.commitHash).toBe('abc123');
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/sharedTypes.test.ts"
```

Expected:

- FAIL，提示 `AuthorFilterOption`、`CommitOverlayRequest`、`FileOverlay.mode`、`FileOverlay.rows`、`DiffBlock.rowStartIndex` 或 `SideBySideDiffRow.blockId` 不存在。

- [ ] **Step 3: 修改共享类型**

在 `src/shared/projectTypes.ts` 增加：

```ts
export interface DirectorySelection {
  path: string;
  name: string;
}
```

在 `src/shared/reviewTypes.ts` 调整和新增：

```ts
export interface ReviewFilters {
  projectId: ProjectId;
  branch: string;
  startAt?: string;
  endAt?: string;
  authorKeys: string[];
  messageQuery?: string;
  globRules: string[];
}

export interface AuthorFilterOption extends AuthorSummary {
  key: string;
  commitCount: number;
}

export interface ReviewAuthorOptionsRequest {
  projectId: ProjectId;
  branch: string;
  startAt?: string;
  endAt?: string;
}

export interface CommitOverlayRequest {
  taskId: TaskId;
  filePath: string;
  commitHash: string;
}
```

扩展已有行和块类型：

```ts
export interface SideBySideDiffRow {
  oldLineNumber?: number;
  newLineNumber?: number;
  oldText?: string;
  newText?: string;
  type: 'context' | 'added' | 'deleted' | 'modified';
  wordChanges?: WordChange[];
  blockId?: string;
}

export interface DiffBlock {
  id: string;
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
  rowStartIndex: number;
  rowEndIndex: number;
  changeType: 'added' | 'deleted' | 'modified';
  authors: AuthorSummary[];
  rows: SideBySideDiffRow[];
  relatedCommits: RelatedCommit[];
}

export interface FileOverlay {
  mode: 'range' | 'commit';
  file: ChangedFile;
  range: AnalysisRange;
  rows: SideBySideDiffRow[];
  blocks: DiffBlock[];
  warnings: AppError[];
  commit?: RelatedCommit;
  parentHash?: string;
}
```

在 `src/shared/ipcTypes.ts` 增加通道和 API：

```ts
projectsSelectDirectory: 'projects:selectDirectory',
reviewListAuthors: 'review:listAuthors',
reviewGetCommitOverlay: 'review:getCommitOverlay',
```

并在 `RevierApi` 增加：

```ts
selectDirectory(): Promise<DirectorySelection | undefined>;
listAuthors(request: ReviewAuthorOptionsRequest): Promise<AuthorFilterOption[]>;
getCommitOverlay(request: CommitOverlayRequest): Promise<FileOverlay>;
```

在 `src/preload/index.ts` 映射新增 API：

```ts
selectDirectory: () => ipcRenderer.invoke(ipcChannels.projectsSelectDirectory),
listAuthors: (request) => ipcRenderer.invoke(ipcChannels.reviewListAuthors, request),
getCommitOverlay: (request) => ipcRenderer.invoke(ipcChannels.reviewGetCommitOverlay, request)
```

- [ ] **Step 4: 运行共享类型测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/sharedTypes.test.ts"
```

Expected:

- PASS。

- [ ] **Step 5: 提交**

```bash
git add src/shared/projectTypes.ts src/shared/reviewTypes.ts src/shared/ipcTypes.ts src/preload/index.ts tests/unit/sharedTypes.test.ts
git commit -m "feat: 扩展 review 交互 IPC 合约"
```

## 任务 2：项目目录选择、Git root 规范化和重复校验

**Files:**
- Modify: `src/main/git/gitService.ts`
- Modify: `src/main/projects/projectStore.ts`
- Modify: `src/main/ipc/projectsIpc.ts`
- Modify: `src/renderer/stores/projectStore.ts`
- Modify: `src/renderer/components/projects/ProjectEditor.vue`
- Modify: `src/renderer/styles.css`
- Modify: `tests/unit/projectStore.test.ts`
- Modify: `tests/integration/gitService.test.ts`
- Modify: `tests/unit/rendererProjectStore.test.ts`
- Create: `tests/unit/projectEditor.test.ts`

- [ ] **Step 1: 写 GitService 和 ProjectStore 失败测试**

在 `tests/integration/gitService.test.ts` 增加：

```ts
import { mkdir } from 'node:fs/promises';

it('normalizes repository subdirectories to the Git root', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'revier-git-'));

  try {
    await initTestRepo(dir);
    await mkdir(join(dir, 'src', 'nested'), { recursive: true });

    const service = new GitService();
    const validation = await service.validateRepository(join(dir, 'src', 'nested'));

    expect(validation.valid).toBe(true);
    expect(validation.repoPath.replaceAll('\\', '/')).toBe(dir.replaceAll('\\', '/'));
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

it('returns a recoverable validation result for non-git directories', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'revier-not-git-'));

  try {
    const service = new GitService();
    const validation = await service.validateRepository(dir);

    expect(validation.valid).toBe(false);
    expect(validation.error).toBe('请选择一个 Git 仓库目录');
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
```

在 `tests/unit/projectStore.test.ts` 增加：

```ts
it('rejects duplicate normalized repository paths', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'revier-projects-'));
  const file = join(dir, 'projects.json');

  try {
    const store = new JsonProjectStore(file);
    await store.add('E:/repos/demo', { name: 'demo' });

    await expect(store.add('E:/repos/demo/', { name: 'again' })).rejects.toThrow(
      '该仓库已在项目列表中'
    );
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/integration/gitService.test.ts tests/unit/projectStore.test.ts"
```

Expected:

- FAIL，GitService 仍返回输入子目录，ProjectStore 允许重复。

- [ ] **Step 3: 实现 GitService root 解析**

在 `src/main/git/gitService.ts` 中增加私有规范化函数，并改造 `validateRepository`：

```ts
import { resolve } from 'node:path';

function normalizeRepoPath(repoPath: string): string {
  return resolve(repoPath).replace(/\\/g, '/').replace(/\/$/, '');
}

async validateRepository(repoPath: string): Promise<RepositoryValidation> {
  const git = simpleGit(repoPath);
  try {
    const isRepo = await git.checkIsRepo();
    if (!isRepo) {
      return { valid: false, repoPath, error: '请选择一个 Git 仓库目录' };
    }

    const root = normalizeRepoPath((await git.revparse(['--show-toplevel'])).trim());
    const branch = await simpleGit(root).branch();
    return { valid: true, repoPath: root, currentBranch: branch.current };
  } catch {
    return { valid: false, repoPath, error: '请选择一个 Git 仓库目录' };
  }
}
```

在同文件导出或复用等价的路径规范化逻辑时不要过度封装；ProjectStore 只需要自己的轻量规范化函数即可。

- [ ] **Step 4: 实现 ProjectStore 重复校验**

在 `src/main/projects/projectStore.ts` 中添加：

```ts
import { basename, dirname, resolve } from 'node:path';

function normalizeStoredRepoPath(repoPath: string): string {
  return resolve(repoPath).replace(/\\/g, '/').replace(/\/$/, '');
}
```

修改 `add`：

```ts
async add(repoPath: string, options: Partial<ReviewProject> = {}): Promise<ReviewProject> {
  const data = await this.read();
  const normalizedRepoPath = normalizeStoredRepoPath(repoPath);
  if (data.projects.some((project) => normalizeStoredRepoPath(project.repoPath) === normalizedRepoPath)) {
    throw new Error('该仓库已在项目列表中');
  }

  const project: ReviewProject = {
    id: randomUUID(),
    name: options.name?.trim() || basename(normalizedRepoPath),
    repoPath: normalizedRepoPath,
    pinned: options.pinned ?? false,
    lastOpenedAt: new Date().toISOString(),
    preferences: {
      ...defaultPreferences,
      ...options.preferences,
      defaultGlobRules: options.preferences?.defaultGlobRules ?? defaultPreferences.defaultGlobRules
    }
  };

  data.projects.push(project);
  await this.write(data);
  return project;
}
```

- [ ] **Step 5: 实现目录选择 IPC 和项目添加校验**

在 `src/main/ipc/projectsIpc.ts` 引入 `dialog` 和 `basename`：

```ts
import { app, dialog, ipcMain } from 'electron';
import { basename } from 'node:path';
```

注册目录选择：

```ts
ipcMain.handle(ipcChannels.projectsSelectDirectory, async () => {
  const result = await dialog.showOpenDialog({
    properties: ['openDirectory'],
    title: '选择 Git 仓库目录'
  });
  const selectedPath = result.filePaths[0];
  if (result.canceled || !selectedPath) {
    return undefined;
  }
  return {
    path: selectedPath,
    name: basename(selectedPath)
  };
});
```

改造 `projectsAdd`：

```ts
ipcMain.handle(ipcChannels.projectsAdd, async (_event, repoPath: string, options) => {
  const validation = await git.validateRepository(repoPath);
  if (!validation.valid) {
    throw new Error(validation.error ?? '请选择一个 Git 仓库目录');
  }
  return store.add(validation.repoPath, {
    ...options,
    preferences: {
      ...options?.preferences,
      defaultBranch: options?.preferences?.defaultBranch ?? validation.currentBranch
    }
  });
});
```

- [ ] **Step 6: 写 ProjectEditor 和 renderer store 测试**

创建 `tests/unit/projectEditor.test.ts`：

```ts
// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import ProjectEditor from '../../src/renderer/components/projects/ProjectEditor.vue';

describe('ProjectEditor', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('selects a directory and fills repo path and project name', async () => {
    const selectDirectory = vi.fn(async () => ({
      path: 'E:/Projects/revier',
      name: 'revier'
    }));
    vi.stubGlobal('window', {
      revier: {
        projects: { selectDirectory }
      }
    });

    const wrapper = mount(ProjectEditor, {
      global: {
        stubs: {
          'el-input': {
            props: ['modelValue'],
            emits: ['update:modelValue'],
            template: '<input :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />'
          },
          'el-button': {
            emits: ['click'],
            template: '<button type="button" @click="$emit(\'click\')"><slot /></button>'
          }
        }
      }
    });

    await wrapper.get('[data-test="select-repo-directory"]').trigger('click');
    const inputs = wrapper.findAll('input');

    expect(selectDirectory).toHaveBeenCalledTimes(1);
    expect((inputs[0].element as HTMLInputElement).value).toBe('E:/Projects/revier');
    expect((inputs[1].element as HTMLInputElement).value).toBe('revier');
  });
});
```

在 `tests/unit/rendererProjectStore.test.ts` 中补充：

```ts
it('stores add-project errors from duplicate or invalid repositories', async () => {
  const api = mockApi({
    add: vi.fn(async () => {
      throw new Error('该仓库已在项目列表中');
    })
  });
  vi.stubGlobal('window', { revier: api });

  const store = useProjectStore();
  await store.addProject('E:/Projects/revier', 'revier');

  expect(store.error).toBe('该仓库已在项目列表中');
  expect(store.loading).toBe(false);
});
```

- [ ] **Step 7: 实现 ProjectEditor UI**

在 `src/renderer/components/projects/ProjectEditor.vue` 中增加目录选择函数：

```ts
async function selectDirectory(): Promise<void> {
  const selection = await window.revier.projects.selectDirectory();
  if (!selection) {
    return;
  }
  form.repoPath = selection.path;
  if (!form.name.trim()) {
    form.name = selection.name;
  }
}
```

路径输入区改为带按钮的输入：

```vue
<label class="field">
  <span>仓库路径</span>
  <div class="path-picker">
    <el-input v-model="form.repoPath" placeholder="E:/Projects/revier" clearable />
    <el-button
      data-test="select-repo-directory"
      type="default"
      aria-label="选择仓库目录"
      @click="selectDirectory"
    >
      <FolderOpen :size="16" aria-hidden="true" />
    </el-button>
  </div>
</label>
```

在 `src/renderer/styles.css` 中添加：

```css
.path-picker {
  display: grid;
  grid-template-columns: minmax(0, 1fr) 36px;
  gap: 8px;
}

.path-picker .el-button {
  width: 36px;
  padding: 0;
}
```

- [ ] **Step 8: 运行项目相关测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/integration/gitService.test.ts tests/unit/projectStore.test.ts tests/unit/rendererProjectStore.test.ts tests/unit/projectEditor.test.ts"
```

Expected:

- PASS。

- [ ] **Step 9: 提交**

```bash
git add src/main/git/gitService.ts src/main/projects/projectStore.ts src/main/ipc/projectsIpc.ts src/renderer/stores/projectStore.ts src/renderer/components/projects/ProjectEditor.vue src/renderer/styles.css tests/integration/gitService.test.ts tests/unit/projectStore.test.ts tests/unit/rendererProjectStore.test.ts tests/unit/projectEditor.test.ts
git commit -m "feat: 支持选择和校验 Git 仓库目录"
```

## 任务 3：结构化筛选控件和作者候选

**Files:**
- Modify: `src/main/git/gitService.ts`
- Modify: `src/main/ipc/reviewIpc.ts`
- Modify: `src/renderer/stores/reviewStore.ts`
- Modify: `src/renderer/components/review/FilterPanel.vue`
- Modify: `src/renderer/pages/ReviewWorkspace.vue`
- Modify: `tests/integration/gitService.test.ts`
- Modify: `tests/unit/reviewIpc.test.ts`
- Modify: `tests/unit/rendererReviewStore.test.ts`
- Create: `tests/unit/filterPanel.test.ts`

- [ ] **Step 1: 写作者筛选和候选测试**

在 `tests/integration/gitService.test.ts` 增加：

```ts
import { commitFiles } from '../fixtures/gitRepo';

it('lists unique authors for a branch', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'revier-git-authors-'));

  try {
    const repo = await initTestRepo(dir);
    await commitFiles(repo, {
      message: 'feat: alice',
      authorName: 'Alice',
      authorEmail: 'alice@example.com',
      files: { 'src/a.ts': 'export const a = 1;\n' }
    });
    await commitFiles(repo, {
      message: 'fix: alice again',
      authorName: 'Alice',
      authorEmail: 'alice@example.com',
      files: { 'src/a.ts': 'export const a = 2;\n' }
    });
    await commitFiles(repo, {
      message: 'feat: bob',
      authorName: 'Bob',
      authorEmail: 'bob@example.com',
      files: { 'src/b.ts': 'export const b = 1;\n' }
    });

    const service = new GitService();
    const authors = await service.listAuthors(dir, 'HEAD');

    expect(authors).toContainEqual({
      key: 'alice@example.com',
      name: 'Alice',
      email: 'alice@example.com',
      commitCount: 2
    });
    expect(authors).toContainEqual({
      key: 'bob@example.com',
      name: 'Bob',
      email: 'bob@example.com',
      commitCount: 1
    });
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
```

在 `tests/unit/reviewIpc.test.ts` 把作者过滤测试改为：

```ts
const filters: ReviewFilters = {
  projectId: project.id,
  branch: 'main',
  startAt: '2026-05-01T00:00:00.000Z',
  endAt: '2026-05-31T00:00:00.000Z',
  authorKeys: ['alice@example.com'],
  messageQuery: 'feature',
  globRules: ['src/**/*.ts']
};
```

断言保持 `result.files` 只包含 Alice 命中的文件。

- [ ] **Step 2: 写 FilterPanel 组件测试**

创建 `tests/unit/filterPanel.test.ts`：

```ts
// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import FilterPanel from '../../src/renderer/components/review/FilterPanel.vue';

describe('FilterPanel', () => {
  it('emits structured filters from branch, authors and date range controls', async () => {
    const wrapper = mount(FilterPanel, {
      props: {
        projectId: 'project-1',
        defaultBranch: 'develop',
        defaultGlobRules: ['src/**/*.ts'],
        branches: [
          { name: 'develop', current: true },
          { name: 'main', current: false }
        ],
        authors: [
          { key: 'alice@example.com', name: 'Alice', email: 'alice@example.com', commitCount: 2 },
          { key: 'bob@example.com', name: 'Bob', email: 'bob@example.com', commitCount: 1 }
        ],
        loading: false
      },
      global: {
        stubs: {
          'el-select': {
            props: ['modelValue'],
            emits: ['update:modelValue'],
            template: '<select multiple @change="$emit(\'update:modelValue\', Array.from($event.target.selectedOptions).map(option => option.value))"><slot /></select>'
          },
          'el-option': {
            props: ['value', 'label'],
            template: '<option :value="value">{{ label }}</option>'
          },
          'el-date-picker': {
            emits: ['update:modelValue'],
            template: '<button type="button" data-test="date-range" @click="$emit(\'update:modelValue\', [new Date(\'2026-05-01T00:00:00.000Z\'), new Date(\'2026-06-01T00:00:00.000Z\')])">date</button>'
          },
          'el-input': {
            props: ['modelValue'],
            emits: ['update:modelValue'],
            template: '<textarea :value="modelValue" @input="$emit(\'update:modelValue\', $event.target.value)" />'
          },
          'el-button': {
            template: '<button type="submit"><slot /></button>'
          }
        }
      }
    });

    await wrapper.get('[data-test="date-range"]').trigger('click');
    await wrapper.find('form').trigger('submit.prevent');

    const event = wrapper.emitted('submit')?.[0]?.[0];
    expect(event).toMatchObject({
      projectId: 'project-1',
      branch: 'develop',
      startAt: '2026-05-01T00:00:00.000Z',
      endAt: '2026-06-01T00:00:00.000Z',
      globRules: ['src/**/*.ts']
    });
  });
});
```

- [ ] **Step 3: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/integration/gitService.test.ts tests/unit/reviewIpc.test.ts tests/unit/filterPanel.test.ts"
```

Expected:

- FAIL，`listAuthors` 不存在，`ReviewFilters.authorKeys` 未接入，FilterPanel props 和控件未实现。

- [ ] **Step 4: 实现 GitService.listAuthors**

在 `src/main/git/gitService.ts` 增加：

```ts
import type { AuthorFilterOption } from '../../shared/reviewTypes';

async listAuthors(
  repoPath: string,
  branch: string,
  startAt?: string,
  endAt?: string
): Promise<AuthorFilterOption[]> {
  const commits = await this.listCommits(repoPath, branch);
  const startTime = startAt ? new Date(startAt).getTime() : Number.NEGATIVE_INFINITY;
  const endTime = endAt ? new Date(endAt).getTime() : Number.POSITIVE_INFINITY;
  const authors = new Map<string, AuthorFilterOption>();

  for (const commit of commits) {
    const committedTime = new Date(commit.committedAt).getTime();
    if (committedTime < startTime || committedTime > endTime) {
      continue;
    }
    const key = authorKey(commit.authorName, commit.authorEmail);
    const current = authors.get(key);
    authors.set(key, {
      key,
      name: commit.authorName,
      email: commit.authorEmail,
      commitCount: (current?.commitCount ?? 0) + 1
    });
  }

  return [...authors.values()].sort((left, right) => {
    if (right.commitCount !== left.commitCount) {
      return right.commitCount - left.commitCount;
    }
    return left.name.localeCompare(right.name);
  });
}

function authorKey(name: string, email?: string): string {
  return (email?.trim() || name.trim()).toLowerCase();
}
```

- [ ] **Step 5: 改造 review IPC 作者筛选**

在 `src/main/ipc/reviewIpc.ts`：

```ts
ipcMain.handle(ipcChannels.reviewListAuthors, async (_event, request) => {
  const project = (await projectStore.list()).find((item) => item.id === request.projectId);
  if (!project) {
    throw new Error('Project not found');
  }
  return git.listAuthors(project.repoPath, request.branch, request.startAt, request.endAt);
});
```

替换 `hasDisplayCommitFilters`：

```ts
function hasDisplayCommitFilters(filters: ReviewFilters): boolean {
  return filters.authorKeys.length > 0 || Boolean(filters.messageQuery?.trim());
}
```

替换 `commitMatchesDisplayFilters`：

```ts
function commitMatchesDisplayFilters(commit: GitCommitSummary, filters: ReviewFilters): boolean {
  return (
    matchesAuthor(commit, filters.authorKeys) &&
    matchesQuery(commit.subject, filters.messageQuery)
  );
}

function matchesAuthor(commit: GitCommitSummary, authorKeys: string[]): boolean {
  if (authorKeys.length === 0) {
    return true;
  }
  const keys = new Set(authorKeys.map((key) => key.toLowerCase()));
  return keys.has(authorKey(commit.authorName, commit.authorEmail));
}
```

- [ ] **Step 6: 改造 review store**

在 `src/renderer/stores/reviewStore.ts` 增加 state：

```ts
authors: [] as AuthorFilterOption[],
authorsLoading: false,
```

增加 action：

```ts
async loadAuthors(request: ReviewAuthorOptionsRequest): Promise<void> {
  this.authorsLoading = true;
  this.error = undefined;
  try {
    this.authors = await window.revier.review.listAuthors(request);
  } catch (error) {
    this.error = toErrorMessage(error);
  } finally {
    this.authorsLoading = false;
  }
}
```

`start` 接收的 filters 必须包含 `authorKeys`，调用端不要再传 `authorQuery`。

- [ ] **Step 7: 改造 FilterPanel**

`src/renderer/components/review/FilterPanel.vue` props 增加：

```ts
import type { GitBranch } from '../../../shared/projectTypes';
import type { AuthorFilterOption, ReviewFilters } from '../../../shared/reviewTypes';

const props = defineProps<{
  projectId: string;
  defaultBranch: string;
  defaultGlobRules: string[];
  branches: GitBranch[];
  authors: AuthorFilterOption[];
  authorsLoading?: boolean;
  loading?: boolean;
}>();
```

form 改为：

```ts
const form = reactive({
  branch: props.defaultBranch,
  dateRange: [] as Date[],
  authorKeys: [] as string[],
  messageQuery: '',
  globRules: props.defaultGlobRules.join('\n')
});
```

submit 中输出：

```ts
const [startAt, endAt] = form.dateRange;
emit('submit', {
  projectId: props.projectId,
  branch: form.branch || 'HEAD',
  startAt: startAt?.toISOString(),
  endAt: endAt?.toISOString(),
  authorKeys: [...form.authorKeys],
  messageQuery: optionalText(form.messageQuery),
  globRules: form.globRules
    .split(/\r?\n/)
    .map((rule) => rule.trim())
    .filter(Boolean)
});
```

模板中使用：

```vue
<el-select v-model="form.branch" filterable>
  <el-option
    v-for="branch in branches"
    :key="branch.name"
    :label="branch.current ? `${branch.name}（当前）` : branch.name"
    :value="branch.name"
  />
</el-select>

<el-date-picker
  v-model="form.dateRange"
  type="datetimerange"
  start-placeholder="开始时间"
  end-placeholder="结束时间"
  value-format="x"
/>

<el-select v-model="form.authorKeys" multiple filterable clearable :loading="authorsLoading">
  <el-option
    v-for="author in authors"
    :key="author.key"
    :label="author.email ? `${author.name} <${author.email}>` : author.name"
    :value="author.key"
  />
</el-select>
```

如果 `el-date-picker` 使用 `value-format="x"` 后 model 是时间戳字符串，submit 中应把时间戳转换为 `new Date(Number(value)).toISOString()`。保持测试和实现一致。

- [ ] **Step 8: 改造 ReviewWorkspace**

在 `src/renderer/pages/ReviewWorkspace.vue`：

- 从 project store 获取分支：调用 `window.revier.projects.listBranches(projectId)` 并存到本地 `branches`。
- `defaultBranch` 变化后调用 `reviewStore.loadAuthors({ projectId, branch })`。
- `FilterPanel` 传入 `branches`、`authors`、`authorsLoading`。
- 运行分析时 filters 已包含 `authorKeys`。

- [ ] **Step 9: 运行筛选相关测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/integration/gitService.test.ts tests/unit/reviewIpc.test.ts tests/unit/rendererReviewStore.test.ts tests/unit/filterPanel.test.ts"
```

Expected:

- PASS。

- [ ] **Step 10: 提交**

```bash
git add src/main/git/gitService.ts src/main/ipc/reviewIpc.ts src/renderer/stores/reviewStore.ts src/renderer/components/review/FilterPanel.vue src/renderer/pages/ReviewWorkspace.vue tests/integration/gitService.test.ts tests/unit/reviewIpc.test.ts tests/unit/rendererReviewStore.test.ts tests/unit/filterPanel.test.ts
git commit -m "feat: 添加结构化 review 筛选控件"
```

## 任务 4：完整文件 overlay 数据和算法

**Files:**
- Modify: `src/main/analysis/overlayEngine.ts`
- Modify: `src/main/ipc/reviewIpc.ts`
- Modify: `tests/unit/overlayEngine.test.ts`
- Modify: `tests/unit/reviewIpc.test.ts`

- [ ] **Step 1: 写完整文件 rows 测试**

在 `tests/unit/overlayEngine.test.ts` 中改用新函数 `buildFileOverlayDiff`：

```ts
import { buildFileOverlayDiff } from '../../src/main/analysis/overlayEngine';

it('keeps context rows while marking changed rows with block ids', () => {
  const diff = buildFileOverlayDiff({
    file,
    oldText: 'same before\nconst name = "old";\nsame after\n',
    newText: 'same before\nconst name = "new";\nsame after\n'
  });

  expect(diff.rows).toHaveLength(3);
  expect(diff.blocks).toHaveLength(1);
  expect(diff.rows[0].type).toBe('context');
  expect(diff.rows[1].type).toBe('modified');
  expect(diff.rows[1].blockId).toBe(diff.blocks[0].id);
  expect(diff.rows[2].type).toBe('context');
  expect(diff.blocks[0].rowStartIndex).toBe(1);
  expect(diff.blocks[0].rowEndIndex).toBe(1);
});

it('keeps empty placeholders for added and deleted lines in full rows', () => {
  const diff = buildFileOverlayDiff({
    file,
    oldText: 'one\nthree\n',
    newText: 'one\ntwo\nthree\n'
  });

  const added = diff.rows.find((row) => row.type === 'added');
  expect(added?.oldLineNumber).toBeUndefined();
  expect(added?.newText).toBe('two');
  expect(added?.blockId).toBe('block-1');
});
```

在 `tests/unit/reviewIpc.test.ts` 的 overlay 测试补充：

```ts
expect(overlay.mode).toBe('range');
expect(overlay.rows.map((row) => row.type)).toEqual(['modified']);
expect(overlay.blocks[0].rowStartIndex).toBe(0);
expect(overlay.blocks[0].rowEndIndex).toBe(0);
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/overlayEngine.test.ts tests/unit/reviewIpc.test.ts"
```

Expected:

- FAIL，`buildFileOverlayDiff` 和完整 rows 字段不存在。

- [ ] **Step 3: 实现 overlayEngine 完整 rows**

在 `src/main/analysis/overlayEngine.ts` 保留 `splitLines` 和行生成逻辑，新增：

```ts
export interface BuiltFileOverlayDiff {
  rows: SideBySideDiffRow[];
  blocks: DiffBlock[];
}

export function buildFileOverlayDiff(input: BuildOverlayInput): BuiltFileOverlayDiff {
  const rows = buildSideBySideRows(input.oldText, input.newText);
  const blocks = groupChangedRows(rows);
  return { rows, blocks };
}

export function buildFileOverlayBlocks(input: BuildOverlayInput): DiffBlock[] {
  return buildFileOverlayDiff(input).blocks;
}
```

把原 `buildFileOverlayBlocks` 中生成 rows 的部分抽成：

```ts
function buildSideBySideRows(oldText: string, newText: string): SideBySideDiffRow[] {
  const parts = diffLines(oldText, newText, { newlineIsToken: false });
  const rows: SideBySideDiffRow[] = [];
  let oldLine = 1;
  let newLine = 1;
  // 保持现有 diffLines/diffWords 逻辑，完整保留 context rows。
  return rows;
}
```

改造 `groupChangedRows`：

```ts
function groupChangedRows(rows: SideBySideDiffRow[]): DiffBlock[] {
  const blocks: DiffBlock[] = [];
  let current: Array<{ row: SideBySideDiffRow; index: number }> = [];

  const flush = () => {
    if (current.length === 0) {
      return;
    }
    const blockId = `block-${blocks.length + 1}`;
    for (const item of current) {
      item.row.blockId = blockId;
    }
    const blockRows = current.map((item) => item.row);
    const oldNumbers = blockRows
      .map((row) => row.oldLineNumber)
      .filter((line): line is number => line !== undefined);
    const newNumbers = blockRows
      .map((row) => row.newLineNumber)
      .filter((line): line is number => line !== undefined);
    const changeTypes = new Set(blockRows.map((row) => row.type));
    blocks.push({
      id: blockId,
      oldStart: oldNumbers[0] ?? 0,
      oldEnd: oldNumbers.at(-1) ?? 0,
      newStart: newNumbers[0] ?? 0,
      newEnd: newNumbers.at(-1) ?? 0,
      rowStartIndex: current[0].index,
      rowEndIndex: current.at(-1)?.index ?? current[0].index,
      changeType: changeTypes.has('modified')
        ? 'modified'
        : changeTypes.has('added')
          ? 'added'
          : 'deleted',
      authors: [],
      rows: blockRows,
      relatedCommits: []
    });
    current = [];
  };

  rows.forEach((row, index) => {
    if (row.type === 'context') {
      flush();
    } else {
      current.push({ row, index });
    }
  });
  flush();
  return blocks;
}
```

- [ ] **Step 4: 改造 buildFileOverlayForTask**

在 `src/main/ipc/reviewIpc.ts` 中把：

```ts
const blocks = attachAttribution(
  buildFileOverlayBlocks({ file, oldText, newText }),
  relatedCommits
);
```

改为：

```ts
const diff = buildFileOverlayDiff({ file, oldText, newText });
const blocks = attachAttribution(diff.blocks, relatedCommits);
```

返回：

```ts
return {
  mode: 'range',
  file,
  range,
  rows: diff.rows,
  blocks: hasDisplayCommitFilters(filters)
    ? blocks.filter((block) => block.relatedCommits.some((commit) => commit.matchedByFilter))
    : blocks,
  warnings: []
};
```

如果作者/提交信息筛选过滤掉 block，需要同步 rows 的可点击状态。实现方式：保留完整 rows，但只有留存 block 的 `blockId` 可选中；被过滤 block 的 rows 将 `blockId` 清空。

- [ ] **Step 5: 运行算法测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/overlayEngine.test.ts tests/unit/reviewIpc.test.ts"
```

Expected:

- PASS。

- [ ] **Step 6: 提交**

```bash
git add src/main/analysis/overlayEngine.ts src/main/ipc/reviewIpc.ts tests/unit/overlayEngine.test.ts tests/unit/reviewIpc.test.ts
git commit -m "feat: 生成完整文件 overlay 行"
```

## 任务 5：完整文件 side-by-side Diff UI

**Files:**
- Modify: `src/renderer/components/review/DiffViewer.vue`
- Modify: `src/renderer/styles.css`
- Create: `tests/unit/diffViewer.test.ts`

- [ ] **Step 1: 写 DiffViewer 组件测试**

创建 `tests/unit/diffViewer.test.ts`：

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
      oldText: 'old',
      newText: 'new',
      type: 'modified',
      blockId: 'block-1'
    },
    {
      oldLineNumber: 3,
      newLineNumber: 3,
      oldText: 'after',
      newText: 'after',
      type: 'context'
    }
  ],
  blocks: [
    {
      id: 'block-1',
      oldStart: 2,
      oldEnd: 2,
      newStart: 2,
      newEnd: 2,
      rowStartIndex: 1,
      rowEndIndex: 1,
      changeType: 'modified',
      authors: [{ name: 'Alice', email: 'alice@example.com' }],
      rows: [],
      relatedCommits: []
    }
  ],
  warnings: []
};

describe('DiffViewer', () => {
  it('renders full file rows and emits selected block when a changed row is clicked', async () => {
    const wrapper = mount(DiffViewer, {
      props: { overlay },
      global: {
        stubs: {
          'el-empty': true,
          'el-alert': true,
          'el-tag': { template: '<span><slot /></span>' }
        }
      }
    });

    expect(wrapper.text()).toContain('same');
    expect(wrapper.text()).toContain('after');

    await wrapper.get('[data-block-id="block-1"]').trigger('click');
    expect(wrapper.emitted('selected')?.[0]?.[0]).toEqual(overlay.blocks[0]);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/diffViewer.test.ts"
```

Expected:

- FAIL，组件仍按 block 渲染，没有 `data-block-id` 行级点击。

- [ ] **Step 3: 改造 DiffViewer**

在 `DiffViewer.vue` 中增加 block map：

```ts
const props = defineProps<{
  overlay?: FileOverlay;
  selectedBlockId?: string;
  loading?: boolean;
}>();

const blocksById = computed(() =>
  new Map((props.overlay?.blocks ?? []).map((block) => [block.id, block]))
);

function selectRow(row: SideBySideDiffRow): void {
  if (!row.blockId) {
    return;
  }
  const block = blocksById.value.get(row.blockId);
  if (block) {
    emit('selected', block);
  }
}
```

模板改为遍历 `overlay.rows`：

```vue
<div class="diff-table diff-table--full">
  <div
    v-for="(row, index) in overlay.rows"
    :key="index"
    :class="[lineClass(row), { 'is-selected': row.blockId === selectedBlockId, 'is-clickable': row.blockId }]"
    :data-block-id="row.blockId"
    @click="selectRow(row)"
  >
    <span class="line-number">{{ row.oldLineNumber ?? '' }}</span>
    <code class="code-cell code-cell--old">...</code>
    <span class="line-number">{{ row.newLineNumber ?? '' }}</span>
    <code class="code-cell code-cell--new">...</code>
  </div>
</div>
```

作者标签显示在块起始行：

```vue
<DiffBlockAuthors
  v-if="row.blockId && blocksById.get(row.blockId)?.rowStartIndex === index"
  class="diff-row__authors"
  :authors="blocksById.get(row.blockId)?.authors ?? []"
/>
```

- [ ] **Step 4: 修改 CSS**

在 `src/renderer/styles.css` 中调整：

```css
.diff-table--full {
  min-width: 920px;
  background: #ffffff;
  border: 1px solid #d7dbe2;
  border-radius: 8px;
  overflow: hidden;
}

.diff-row {
  position: relative;
}

.diff-row.is-clickable {
  cursor: pointer;
}

.diff-row.is-clickable:hover {
  box-shadow: inset 3px 0 0 #9ca3af;
}

.diff-row.is-selected {
  box-shadow: inset 3px 0 0 #0f766e;
}

.diff-row__authors {
  position: absolute;
  top: 2px;
  right: 8px;
  z-index: 1;
  max-width: 36%;
}
```

- [ ] **Step 5: 运行 DiffViewer 测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/diffViewer.test.ts"
```

Expected:

- PASS。

- [ ] **Step 6: 提交**

```bash
git add src/renderer/components/review/DiffViewer.vue src/renderer/styles.css tests/unit/diffViewer.test.ts
git commit -m "feat: 展示完整文件 side-by-side diff"
```

## 任务 6：三栏宽度拖拽和持久化

**Files:**
- Create: `src/renderer/composables/useReviewLayoutSizes.ts`
- Create: `src/renderer/components/review/ReviewLayoutResizer.vue`
- Modify: `src/renderer/pages/ReviewWorkspace.vue`
- Modify: `src/renderer/styles.css`
- Create: `tests/unit/reviewLayoutSizes.test.ts`

- [ ] **Step 1: 写布局尺寸测试**

创建 `tests/unit/reviewLayoutSizes.test.ts`：

```ts
import {
  clampReviewLayout,
  defaultReviewLayout,
  reviewLayoutStorageKey
} from '../../src/renderer/composables/useReviewLayoutSizes';

describe('review layout sizes', () => {
  it('clamps side panes while preserving center minimum width', () => {
    const layout = clampReviewLayout(
      { left: 900, right: 900 },
      { containerWidth: 1200, centerMin: 520 }
    );

    expect(layout.left).toBeLessThanOrEqual(460);
    expect(layout.right).toBeLessThanOrEqual(520);
    expect(layout.left + layout.right).toBeLessThanOrEqual(680);
  });

  it('uses a versioned storage key', () => {
    expect(reviewLayoutStorageKey).toBe('revier.reviewLayout.v1');
    expect(defaultReviewLayout).toEqual({ left: 320, right: 320 });
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/reviewLayoutSizes.test.ts"
```

Expected:

- FAIL，composable 不存在。

- [ ] **Step 3: 实现 useReviewLayoutSizes**

创建 `src/renderer/composables/useReviewLayoutSizes.ts`：

```ts
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
    const ratio = maxSideTotal / total;
    left = Math.max(260, Math.floor(left * ratio));
    right = Math.max(280, Math.floor(right * ratio));
  }
  return { left, right };
}
```

继续实现 composable：

```ts
export function useReviewLayoutSizes() {
  const container = ref<HTMLElement>();
  const sizes = ref<ReviewLayoutSizes>(loadStoredLayout());
  const gridTemplateColumns = computed(() => `${sizes.value.left}px 6px minmax(520px, 1fr) 6px ${sizes.value.right}px`);

  function setContainer(element?: HTMLElement): void {
    container.value = element;
    applyClamp();
  }

  function resize(side: 'left' | 'right', delta: number): void {
    sizes.value = side === 'left'
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
    const parsed = JSON.parse(window.localStorage.getItem(reviewLayoutStorageKey) ?? 'null') as ReviewLayoutSizes | null;
    return parsed && Number.isFinite(parsed.left) && Number.isFinite(parsed.right)
      ? parsed
      : defaultReviewLayout;
  } catch {
    return defaultReviewLayout;
  }
}
```

- [ ] **Step 4: 实现 ReviewLayoutResizer**

创建 `src/renderer/components/review/ReviewLayoutResizer.vue`：

```vue
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
```

- [ ] **Step 5: 接入 ReviewWorkspace**

在 `src/renderer/pages/ReviewWorkspace.vue`：

```ts
import ReviewLayoutResizer from '../components/review/ReviewLayoutResizer.vue';
import { useReviewLayoutSizes } from '../composables/useReviewLayoutSizes';

const layout = useReviewLayoutSizes();
```

模板：

```vue
<main
  ref="layout.setContainer"
  class="review-workspace"
  :style="{ gridTemplateColumns: layout.gridTemplateColumns.value }"
>
  <aside class="review-sidebar">...</aside>
  <ReviewLayoutResizer side="left" @resize="layout.resize" />
  <section class="review-diff-pane">...</section>
  <ReviewLayoutResizer side="right" @resize="layout.resize" />
  <BlockDetailPanel class="review-detail-pane" ... />
</main>
```

如果模板 ref 不能直接调用函数，使用 `const workspaceEl = ref<HTMLElement>();` 并在 `onMounted(() => layout.setContainer(workspaceEl.value))`。

- [ ] **Step 6: 修改 CSS**

```css
.review-workspace {
  display: grid;
  grid-template-columns: 320px 6px minmax(520px, 1fr) 6px 320px;
}

.review-resizer {
  width: 6px;
  min-width: 6px;
  background: #f6f7f9;
  border-inline: 1px solid #d7dbe2;
  cursor: col-resize;
}

.review-resizer:hover,
.review-resizer:focus-visible {
  background: #d7dbe2;
  outline: none;
}
```

- [ ] **Step 7: 运行布局测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/reviewLayoutSizes.test.ts"
```

Expected:

- PASS。

- [ ] **Step 8: 提交**

```bash
git add src/renderer/composables/useReviewLayoutSizes.ts src/renderer/components/review/ReviewLayoutResizer.vue src/renderer/pages/ReviewWorkspace.vue src/renderer/styles.css tests/unit/reviewLayoutSizes.test.ts
git commit -m "feat: 支持 review 三栏宽度调整"
```

## 任务 7：提交下钻数据接口和 store 状态

**Files:**
- Modify: `src/main/git/gitService.ts`
- Modify: `src/main/ipc/reviewIpc.ts`
- Modify: `src/renderer/stores/reviewStore.ts`
- Modify: `tests/integration/gitService.test.ts`
- Modify: `tests/unit/reviewIpc.test.ts`
- Modify: `tests/unit/rendererReviewStore.test.ts`

- [ ] **Step 1: 写 GitService 第一父提交测试**

在 `tests/integration/gitService.test.ts` 增加：

```ts
it('resolves the first parent of a commit', async () => {
  const dir = await mkdtemp(join(tmpdir(), 'revier-git-parent-'));

  try {
    const repo = await initTestRepo(dir);
    const child = await commitFiles(repo, {
      message: 'feat: child',
      files: { 'src/app.ts': 'export const value = 1;\n' }
    });
    const service = new GitService();
    const parent = await service.getFirstParent(dir, child);

    expect(parent).toHaveLength(40);
    expect(parent).not.toBe(child);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});
```

- [ ] **Step 2: 写 commit overlay IPC 测试**

在 `tests/unit/reviewIpc.test.ts` 增加导出函数测试：

```ts
import { buildCommitOverlayForTask } from '../../src/main/ipc/reviewIpc';

it('builds a commit overlay from the commit first parent to the commit', async () => {
  const range: AnalysisRange = {
    branch: 'main',
    baseCommit: 'base',
    headCommit: 'head'
  };
  const related = commit('alice', '2026-05-10T00:00:00.000Z', 'Alice', 'feature: update app');
  const overlay = await buildCommitOverlayForTask({
    project,
    file: modifiedFile,
    range,
    commitHash: related.hash,
    rangeCommits: [related],
    git: {
      listCommits: vi.fn(),
      listChangedFiles: vi.fn(),
      readFileAtCommit: vi.fn(async (_repoPath, commitHash) =>
        commitHash === 'parent' ? 'const name = "old";\n' : 'const name = "new";\n'
      ),
      showFilePatch: vi.fn(async () => '@@ -1 +1 @@\n-old\n+new\n'),
      getFirstParent: vi.fn(async () => 'parent')
    }
  });

  expect(overlay.mode).toBe('commit');
  expect(overlay.parentHash).toBe('parent');
  expect(overlay.commit?.hash).toBe('alice');
  expect(overlay.blocks).toHaveLength(1);
});
```

- [ ] **Step 3: 写 reviewStore 下钻测试**

在 `tests/unit/rendererReviewStore.test.ts` 增加：

```ts
it('loads and clears commit drilldown overlay', async () => {
  const commitOverlay: FileOverlay = {
    ...overlay,
    mode: 'commit',
    commit: {
      hash: 'abc123',
      shortHash: 'abc123',
      authorName: 'Alice',
      authorEmail: 'alice@example.com',
      committedAt: '2026-05-10T00:00:00.000Z',
      subject: 'feature: update app',
      matchedByFilter: true,
      touchedRanges: []
    },
    parentHash: 'parent'
  };
  const api = mockApi({
    getCommitOverlay: vi.fn(async () => commitOverlay)
  });
  vi.stubGlobal('window', { revier: api });

  const store = useReviewStore();
  store.task = task;
  await store.loadCommitOverlay(file.path, 'abc123');

  expect(api.review.getCommitOverlay).toHaveBeenCalledWith({
    taskId: task.taskId,
    filePath: file.path,
    commitHash: 'abc123'
  });
  expect(store.drilldownOverlay).toEqual(commitOverlay);
  expect(store.selectedCommitHash).toBe('abc123');

  store.closeCommitDrilldown();
  expect(store.drilldownOverlay).toBeUndefined();
  expect(store.selectedCommitHash).toBeUndefined();
});
```

- [ ] **Step 4: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/integration/gitService.test.ts tests/unit/reviewIpc.test.ts tests/unit/rendererReviewStore.test.ts"
```

Expected:

- FAIL，`getFirstParent`、`buildCommitOverlayForTask`、store 下钻状态不存在。

- [ ] **Step 5: 实现 GitService 方法**

在 `src/main/git/gitService.ts` 增加：

```ts
async getFirstParent(repoPath: string, commitHash: string): Promise<string> {
  const line = (await simpleGit(repoPath).raw(['rev-list', '--parents', '-n', '1', commitHash])).trim();
  const [, parent] = line.split(/\s+/);
  if (!parent) {
    return `${commitHash}^`;
  }
  return parent;
}
```

- [ ] **Step 6: 实现 buildCommitOverlayForTask**

在 `src/main/ipc/reviewIpc.ts` 扩展 `AnalysisGitClient`：

```ts
getFirstParent(repoPath: string, commitHash: string): Promise<string>;
```

新增函数：

```ts
export async function buildCommitOverlayForTask({
  project,
  file,
  range,
  commitHash,
  rangeCommits,
  git
}: BuildCommitOverlayInput): Promise<FileOverlay> {
  const commit = rangeCommits.find((item) => item.hash === commitHash);
  if (!commit) {
    throw new Error('该提交不在当前筛选范围内');
  }

  const patch = await git.showFilePatch(project.repoPath, commitHash, file.path);
  if (!patch.trim()) {
    throw new Error('该提交未修改当前文件');
  }

  const parentHash = await git.getFirstParent(project.repoPath, commitHash);
  const oldText = await git.readFileAtCommit(project.repoPath, parentHash, file.oldPath ?? file.path);
  const newText = await git.readFileAtCommit(project.repoPath, commitHash, file.path);
  const diff = buildFileOverlayDiff({ file, oldText, newText });
  const relatedCommit: RelatedCommit = {
    hash: commit.hash,
    shortHash: commit.shortHash,
    authorName: commit.authorName,
    authorEmail: commit.authorEmail,
    committedAt: commit.committedAt,
    subject: commit.subject,
    matchedByFilter: false,
    touchedRanges: parsePatchTouchedRanges(patch)
  };

  return {
    mode: 'commit',
    file,
    range,
    rows: diff.rows,
    blocks: diff.blocks.map((block) => ({
      ...block,
      authors: [{ name: commit.authorName, email: commit.authorEmail }],
      relatedCommits: [relatedCommit]
    })),
    warnings: [],
    commit: relatedCommit,
    parentHash
  };
}
```

注册 IPC：

```ts
ipcMain.handle(ipcChannels.reviewGetCommitOverlay, async (_event, request): Promise<FileOverlay> => {
  const range = rangesByTask.get(request.taskId);
  const project = projectsByTask.get(request.taskId);
  const rangeCommits = rangeCommitsByTask.get(request.taskId) ?? [];
  const file = filesByTask.get(request.taskId)?.find((item) => item.path === request.filePath);
  if (!range || !project || !file) {
    throw new Error('Commit overlay request is not associated with an active task file');
  }
  return buildCommitOverlayForTask({
    project,
    file,
    range,
    commitHash: request.commitHash,
    rangeCommits,
    git
  });
});
```

- [ ] **Step 7: 实现 reviewStore 下钻状态**

在 `src/renderer/stores/reviewStore.ts` state 增加：

```ts
drilldownOverlay: undefined as FileOverlay | undefined,
selectedCommitHash: undefined as string | undefined,
drilldownLoading: false,
```

在 `start` 和 `loadOverlay` 开头调用 `this.closeCommitDrilldown()`。

新增 actions：

```ts
async loadCommitOverlay(filePath: string, commitHash: string): Promise<void> {
  if (!this.task) {
    this.error = 'No active analysis task';
    return;
  }

  this.drilldownLoading = true;
  this.error = undefined;
  this.selectedCommitHash = commitHash;
  try {
    this.drilldownOverlay = await window.revier.review.getCommitOverlay({
      taskId: this.task.taskId,
      filePath,
      commitHash
    });
  } catch (error) {
    this.error = toErrorMessage(error);
  } finally {
    this.drilldownLoading = false;
  }
},

closeCommitDrilldown(): void {
  this.drilldownOverlay = undefined;
  this.selectedCommitHash = undefined;
}
```

- [ ] **Step 8: 运行下钻数据测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/integration/gitService.test.ts tests/unit/reviewIpc.test.ts tests/unit/rendererReviewStore.test.ts"
```

Expected:

- PASS。

- [ ] **Step 9: 提交**

```bash
git add src/main/git/gitService.ts src/main/ipc/reviewIpc.ts src/renderer/stores/reviewStore.ts tests/integration/gitService.test.ts tests/unit/reviewIpc.test.ts tests/unit/rendererReviewStore.test.ts
git commit -m "feat: 添加提交 diff 下钻数据接口"
```

## 任务 8：提交下钻 UI、返回和覆盖动画

**Files:**
- Modify: `src/renderer/components/review/BlockDetailPanel.vue`
- Create: `src/renderer/components/review/DiffDrilldownOverlay.vue`
- Modify: `src/renderer/pages/ReviewWorkspace.vue`
- Modify: `src/renderer/styles.css`
- Create: `tests/unit/blockDetailPanel.test.ts`

- [ ] **Step 1: 写 BlockDetailPanel 点击提交测试**

创建 `tests/unit/blockDetailPanel.test.ts`：

```ts
// @vitest-environment jsdom
import { mount } from '@vue/test-utils';
import BlockDetailPanel from '../../src/renderer/components/review/BlockDetailPanel.vue';
import type { DiffBlock } from '../../src/shared/reviewTypes';

const block: DiffBlock = {
  id: 'block-1',
  oldStart: 1,
  oldEnd: 1,
  newStart: 1,
  newEnd: 1,
  rowStartIndex: 0,
  rowEndIndex: 0,
  changeType: 'modified',
  authors: [{ name: 'Alice', email: 'alice@example.com' }],
  rows: [],
  relatedCommits: [
    {
      hash: 'abc123',
      shortHash: 'abc123',
      authorName: 'Alice',
      authorEmail: 'alice@example.com',
      committedAt: '2026-05-10T00:00:00.000Z',
      subject: 'feature: update app',
      matchedByFilter: true,
      touchedRanges: []
    }
  ]
};

describe('BlockDetailPanel', () => {
  it('emits selected commit for drilldown', async () => {
    const wrapper = mount(BlockDetailPanel, {
      props: { block, selectedCommitHash: undefined },
      global: {
        stubs: {
          'el-empty': true,
          'el-tag': { template: '<span><slot /></span>' }
        }
      }
    });

    await wrapper.get('[data-commit-hash="abc123"]').trigger('click');
    expect(wrapper.emitted('commitSelected')?.[0]?.[0]).toEqual(block.relatedCommits[0]);
  });
});
```

- [ ] **Step 2: 运行测试确认失败**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/blockDetailPanel.test.ts"
```

Expected:

- FAIL，右侧提交行不可点击且没有事件。

- [ ] **Step 3: 改造 BlockDetailPanel**

在 `BlockDetailPanel.vue`：

```ts
const props = defineProps<{
  block?: DiffBlock;
  selectedCommitHash?: string;
}>();

const emit = defineEmits<{
  commitSelected: [commit: RelatedCommit];
}>();
```

提交行改成按钮：

```vue
<li v-for="commit in block.relatedCommits" :key="commitKey(commit)">
  <button
    class="commit-row"
    :class="{ 'is-selected': commit.hash === selectedCommitHash }"
    :data-commit-hash="commit.hash"
    type="button"
    @click="emit('commitSelected', commit)"
  >
    ...
  </button>
</li>
```

- [ ] **Step 4: 创建 DiffDrilldownOverlay**

创建 `src/renderer/components/review/DiffDrilldownOverlay.vue`：

```vue
<script setup lang="ts">
import { ArrowLeft } from 'lucide-vue-next';
import type { FileOverlay } from '../../../shared/reviewTypes';
import DiffViewer from './DiffViewer.vue';

defineProps<{
  overlay?: FileOverlay;
  loading?: boolean;
  selectedBlockId?: string;
}>();

const emit = defineEmits<{
  close: [];
}>();
</script>

<template>
  <Transition name="drilldown-cover" mode="out-in">
    <section v-if="overlay || loading" class="diff-drilldown">
      <header class="diff-drilldown__bar">
        <el-button text type="primary" @click="emit('close')">
          <ArrowLeft :size="16" aria-hidden="true" />
          <span>返回</span>
        </el-button>
        <div class="diff-drilldown__title">
          <code>{{ overlay?.commit?.shortHash ?? 'loading' }}</code>
          <span>{{ overlay?.commit?.subject ?? '加载提交变更' }}</span>
        </div>
      </header>
      <DiffViewer
        :overlay="overlay"
        :loading="loading"
        :selected-block-id="selectedBlockId"
      />
    </section>
  </Transition>
</template>
```

- [ ] **Step 5: 接入 ReviewWorkspace**

在 `ReviewWorkspace.vue`：

```ts
import DiffDrilldownOverlay from '../components/review/DiffDrilldownOverlay.vue';
import type { RelatedCommit } from '../../shared/reviewTypes';

const {
  task,
  files,
  overlay,
  drilldownOverlay,
  selectedBlock,
  selectedCommitHash,
  loading,
  drilldownLoading,
  error
} = storeToRefs(reviewStore);

async function openCommitDrilldown(commit: RelatedCommit): Promise<void> {
  if (!selectedFilePath.value) {
    return;
  }
  await reviewStore.loadCommitOverlay(selectedFilePath.value, commit.hash);
}
```

中间栏：

```vue
<section class="review-diff-pane">
  <DiffViewer
    :overlay="overlay"
    :loading="loading"
    :selected-block-id="selectedBlock?.id"
    @selected="reviewStore.selectBlock"
  />
  <DiffDrilldownOverlay
    :overlay="drilldownOverlay"
    :loading="drilldownLoading"
    :selected-block-id="undefined"
    @close="reviewStore.closeCommitDrilldown"
  />
</section>
```

右侧：

```vue
<BlockDetailPanel
  class="review-detail-pane"
  :block="selectedBlock"
  :selected-commit-hash="selectedCommitHash"
  @commit-selected="openCommitDrilldown"
/>
```

- [ ] **Step 6: 添加动画 CSS**

在 `src/renderer/styles.css`：

```css
.review-diff-pane {
  position: relative;
}

.diff-drilldown {
  position: absolute;
  inset: 0;
  z-index: 3;
  overflow: auto;
  background: #f6f7f9;
  border-left: 1px solid #d7dbe2;
}

.diff-drilldown__bar {
  position: sticky;
  top: 0;
  z-index: 4;
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: 48px;
  padding: 8px 14px;
  background: #ffffff;
  border-bottom: 1px solid #d7dbe2;
}

.diff-drilldown__title {
  display: flex;
  align-items: center;
  gap: 10px;
  min-width: 0;
  color: #1f2937;
  font-size: 13px;
}

.diff-drilldown__title span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.drilldown-cover-enter-active {
  transition:
    transform 200ms ease,
    opacity 200ms ease;
}

.drilldown-cover-enter-from {
  opacity: 0.92;
  transform: translateX(100%);
}

.drilldown-cover-enter-to {
  opacity: 1;
  transform: translateX(0);
}

.drilldown-cover-leave-active {
  transition: opacity 120ms ease;
}

.drilldown-cover-leave-to {
  opacity: 0;
}

@media (prefers-reduced-motion: reduce) {
  .drilldown-cover-enter-active,
  .drilldown-cover-leave-active {
    transition: none;
  }
}

.commit-row {
  width: 100%;
  color: inherit;
  background: transparent;
  border: 0;
  cursor: pointer;
  text-align: left;
}

.commit-row:hover,
.commit-row.is-selected {
  background: #eef8f6;
}
```

- [ ] **Step 7: 运行 UI 下钻测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test -- tests/unit/blockDetailPanel.test.ts"
```

Expected:

- PASS。

- [ ] **Step 8: 提交**

```bash
git add src/renderer/components/review/BlockDetailPanel.vue src/renderer/components/review/DiffDrilldownOverlay.vue src/renderer/pages/ReviewWorkspace.vue src/renderer/styles.css tests/unit/blockDetailPanel.test.ts
git commit -m "feat: 添加提交下钻交互"
```

## 任务 9：端到端验证和整体回归

**Files:**
- Modify: `tests/e2e/review-workflow.spec.ts`
- Modify: existing files only if E2E exposes defects.

- [ ] **Step 1: 更新 E2E 覆盖结构化筛选和完整 diff**

在 `tests/e2e/review-workflow.spec.ts` 保留现有真实 Electron 启动，增加断言：

```ts
await expect(page.getByRole('heading', { name: '筛选' })).toBeVisible();
await expect(page.getByRole('combobox').first()).toBeVisible();
await expect(page.locator('.diff-table--full')).toBeVisible({ timeout: 15000 });
await expect(page.locator('.diff-row--context').first()).toBeVisible();
```

对于系统目录选择对话框，E2E 不直接驱动原生 dialog。该能力由 `projectsIpc`/`ProjectEditor` 单元测试覆盖；E2E 继续通过手动路径输入完成完整工作流。

- [ ] **Step 2: 运行全量单元与集成测试**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test"
```

Expected:

- PASS。

- [ ] **Step 3: 运行 E2E**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm test:e2e"
```

Expected:

- PASS。

- [ ] **Step 4: 运行构建**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm build"
```

Expected:

- PASS。
- 输出目录为 `dist/main`、`dist/preload`、`dist/renderer`。

- [ ] **Step 5: 检查 lint 状态**

Run:

```bash
pwsh -NoLogo -ExecutionPolicy Bypass -Command "fnm use 24; pnpm lint"
```

Expected:

- 如果仍因 ESLint 9 缺少 `eslint.config.*` 失败，记录为既有项目配置问题，不在本计划内修复。
- 如果出现本轮代码导致的 lint 错误，修复后重新运行。

- [ ] **Step 6: 最终提交 E2E 调整**

```bash
git add tests/e2e/review-workflow.spec.ts
git commit -m "test: 覆盖 review 工作台交互改进"
```

## 自检清单

- 项目选择：
  - 任务 2 覆盖目录选择、非 Git 仓库、Git root 规范化、重复仓库。
- 结构化筛选：
  - 任务 3 覆盖分支下拉、作者多选、日期范围和结构化作者筛选。
- 完整文件 diff：
  - 任务 4 覆盖完整 rows 数据。
  - 任务 5 覆盖完整行渲染、变更块点击。
- 三栏调整：
  - 任务 6 覆盖宽度 clamp、拖拽组件、localStorage 持久化。
- 提交下钻：
  - 任务 7 覆盖单提交 overlay 数据。
  - 任务 8 覆盖右侧提交点击、下钻覆盖层、返回按钮、动画。
- 回归：
  - 任务 9 覆盖 `pnpm test`、`pnpm test:e2e`、`pnpm build`。

## 执行顺序

按任务 1 到任务 9 顺序执行。不要跳过测试步骤。每个任务结束后提交一次，提交消息使用语义化提交格式。
