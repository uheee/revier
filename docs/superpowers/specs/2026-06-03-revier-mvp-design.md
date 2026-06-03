# Revier MVP 设计

> 状态：已确认方案。本文记录 Revier MVP 的产品范围、架构、算法、UI、性能、API 和验收设计。

## 目标

Revier 是一个基于 Vue 3 + TypeScript + Electron 的代码 review 辅助工具。MVP 目标是打开本地 Git 仓库，按分支、时间、作者、文件 glob、提交信息筛选提交范围，并浏览文件在该范围内的叠加差异结果。

核心体验：

- 展示某个仓库的 Git 提交内容。
- 支持按时间、作者、分支、文件名通配符、提交信息筛选。
- 打开变更文件后，默认展示筛选区间内的最终净变化。
- 变更块旁边显示历史参与作者，支持多人。
- 点击变更块后展开详情，显示匹配范围内相关提交信息。

## 已确认范围

- MVP 范围：选择本地仓库、筛选提交、浏览文件叠加 diff、显示作者和提交详情。
- 时间筛选：可选。不填时默认使用当前分支最近一段时间，建议默认最近 30 天。
- 叠加 diff：默认展示最终净变化；展开详情中保留被覆盖、回滚或中间修改历史。
- 作者列表：按历史参与作者显示。只要筛选范围内曾修改过该变更块相关区域，就显示。
- 分支：默认当前 HEAD 所在分支，用户可切换到其他分支。
- 文件通配符：支持 glob + 排除规则，例如 `src/**/*.ts` 和 `!**/*.test.ts`。
- Git 调用：使用 `simple-git + git CLI`。
- diff 粒度：行级 diff，并对修改行做词级高亮。
- 文件入口：只列出当前筛选范围内发生最终净变化的文件。
- 筛选语义：分支和时间决定真实 Git 区间；作者、提交信息、文件 glob 限制哪些文件和变更块进入视图。
- 筛选命中展示：显示完整变更块，并在详情中标记哪些提交/行命中筛选条件。
- 项目管理：保存仓库名称、路径、最近打开、固定、移除记录，并保存每仓库默认分支、默认时间范围、默认 glob 规则。
- 特殊文件变更：文本文件优先，支持 rename/move 识别；二进制文件列出但不可预览；删除文件可看删除 diff。
- diff 布局：UI 默认 side-by-side。数据层保留后续 unified 切换能力。
- 验收优先级：中型仓库性能，目标覆盖几千提交、上千文件。

## 推荐方案

采用“任务化计算 + 会话缓存”方案。

Electron 主进程通过 `simple-git` 调用本机 Git。筛选条件变化后创建后台分析任务，分阶段计算提交列表、文件列表、单文件 overlay 和块详情。前端通过 IPC 查询任务状态和结果。

选择该方案的原因：

- 比每次实时全量计算更适合中型仓库。
- 比一开始建立完整 SQLite 索引更适合 MVP。
- 能支持渐进式加载、任务取消、局部缓存和较好的 UI 响应。

## 架构设计

项目从空目录初始化，使用 Vite + Vue 作为渲染层，Electron 主进程负责本地仓库访问、Git 调用、缓存和后台任务，前端不直接访问文件系统或 Git。

核心分层：

1. `Renderer`
   负责页面、筛选表单、项目管理、文件列表、side-by-side diff 展示、块详情展开。它只通过 typed IPC 调用主进程 API。

2. `Preload`
   暴露受控 API，例如 `projects.openRepository()`、`review.startAnalysis()`、`review.getChangedFiles()`、`review.getFileOverlay()`。避免 renderer 获得完整 Node 能力。

3. `Main`
   负责应用窗口、IPC 路由、项目配置持久化、后台分析任务管理。所有耗时 Git 分析不直接阻塞 UI 请求。

4. `Git Service`
   基于 `simple-git + git CLI` 封装仓库读取：分支列表、提交列表、提交详情、文件变更、rename 检测、文件内容、diff patch、blame 或按提交追踪。

5. `Analysis Engine`
   负责把分支和时间区间转换为真实起止状态，生成最终净变化 diff；再用作者、提交信息、glob 过滤决定哪些文件和变更块显示；为每个变更块聚合历史参与作者和提交详情。

6. `Cache`
   MVP 先做会话缓存 + 项目偏好持久化。缓存提交列表、文件变更摘要、单文件 overlay 结果；项目偏好保存仓库名称、路径、默认分支、默认时间范围、默认 glob 规则。暂不做完整 SQLite 索引。

任务流程：

1. 前端提交筛选条件。
2. 主进程创建分析任务。
3. 任务分阶段产出提交摘要、变更文件列表、文件 overlay。
4. 前端轮询或订阅进度并逐步渲染。

## Git 数据流与算法设计

计算口径采用混合语义：

- 分支和时间范围决定真实 Git 区间。
- 作者、提交信息、文件 glob 决定哪些文件和变更块进入视图。

### 分析区间

用户选择项目后，默认使用项目默认分支；没有配置时使用当前 HEAD 分支。时间范围为空时使用最近 30 天。

主进程解析：

- `baseCommit`：时间范围开始前，目标分支上最后一个提交；如果找不到，则使用该分支最早可用提交的父边界。
- `headCommit`：时间范围结束前，目标分支上的最后一个提交；如果未填结束时间，则为目标分支当前 HEAD。
- `rangeCommits`：`baseCommit..headCommit` 中落在时间范围内的提交集合，用于作者、提交信息、历史详情和块归因。

最终净变化通过 `git diff baseCommit headCommit -- <paths>` 生成，所以结果代表真实仓库状态变化，而不是把筛选提交拼接出来。

### 提交和文件筛选

筛选条件分两层：

- 区间层：分支、时间决定 `rangeCommits`。
- 展示层：作者、提交信息、glob include/exclude 决定哪些文件和变更块显示。

glob 使用 `minimatch` 风格，支持多条规则，其中 `!pattern` 表示排除。MVP 中路径统一用 `/` 规范化。

文件列表来自 `git diff --name-status --find-renames baseCommit headCommit`，只显示最终区间有净变化的文件。再叠加 glob 规则。二进制文件显示不可预览；rename/move 在文件列表中作为同一项展示，overlay 尽量沿 rename 追踪历史。

### 文件 overlay

用户点击文件后，主进程按需计算该文件：

1. 读取 `baseCommit` 与 `headCommit` 的文件内容。
2. 生成行级 diff，并对修改行做词级高亮。
3. 按连续变更行聚合为 `DiffBlock`。
4. 每个块保留旧侧行号范围、新侧行号范围、变更类型、side-by-side 行数据。

UI 只默认展示 side-by-side；数据结构保留 `unified` 所需的行序和标记，后续可加切换。

### 历史归因与详情

对每个 `DiffBlock`，在 `rangeCommits` 内查找曾影响该块相关行区间或相关文件片段的提交：

1. 先用每个提交的 file patch 建立轻量映射：文件路径 -> touched ranges -> commit metadata。
2. rename 文件沿 rename 链把旧路径和新路径关联到同一个逻辑文件。
3. 对 overlay 块的旧/新行范围，与 touched ranges 做区间交集。
4. 命中的提交用于生成块详情，并聚合作者列表。

作者列表采用历史参与作者口径：只要筛选范围内曾修改过该块相关区域，就显示。若多位作者交叉修改同一块，都展示；单人则只展示一人。

### 作者和提交信息筛选命中语义

当用户填写作者或提交信息筛选：

- 文件仍必须在最终净变化中存在。
- 块只要有任一相关历史提交命中筛选条件，就进入视图。
- 显示完整 diff 块，避免上下文被切碎。
- 展开详情中标记哪些提交/行是筛选命中的。

### Merge commit

MVP 默认按目标分支一条历史遍历。对于 merge commit：

- 提交列表中显示 merge commit。
- 文件净变化仍由 `baseCommit..headCommit` 决定。
- 归因初版按 `git show` 对第一父提交的变化计算，符合主线 review 场景。
- 复杂多父归因不作为 MVP 验收重点。

## UI 页面与交互设计

MVP 采用偏工具型桌面应用布局，不做营销页。启动后直接进入项目工作台。

### 项目页

用于管理本地仓库项目：

- 新增项目：选择本地 Git 仓库路径，填写或自动生成项目名。
- 项目列表：显示名称、路径、默认分支、最近打开时间。
- 项目操作：打开、固定、移除记录。
- 项目偏好：默认分支、默认时间范围、默认 glob include/exclude 规则。

MVP 不做分组、批量管理和复杂排序。

### Review 工作台

打开项目后进入三栏结构：

- 左侧：筛选区和变更文件列表。
- 中间：side-by-side diff 主视图。
- 右侧：选中变更块详情面板。

筛选区包含：

- 分支选择，默认当前项目默认分支或当前 HEAD 分支。
- 时间范围，默认最近 30 天。
- 作者筛选。
- 提交信息关键词。
- 文件 glob 规则，支持 `!` 排除。
- 重新分析按钮和任务进度状态。

文件列表只展示当前筛选范围内有最终净变化的文件。每个文件显示路径、变更类型、增删行数、是否重命名、是否二进制不可预览。

### Diff 主视图

默认使用 side-by-side：

- 左侧旧版本，右侧新版本。
- 行级增删改展示。
- 修改行支持词级高亮。
- 每个连续变更块旁边显示历史参与作者。
- 点击变更块后展开或选中详情。
- 二进制文件显示不可预览状态。
- 删除文件显示删除 diff。
- rename 文件显示旧路径 -> 新路径。

UI 暂不提供 unified 切换，但数据层保留支持。

### 变更块详情

右侧详情面板展示当前块在筛选区间内的历史参与记录：

- 作者列表。
- 相关提交列表：hash、标题、作者、时间、是否命中当前筛选。
- 每个提交影响的行范围或 patch 摘要。
- 对作者/提交信息筛选命中的提交做标记。

详情默认折叠提交 patch 摘要，避免大型文件时面板过重。

### 任务状态

分析采用后台任务，前端显示阶段状态：

- 读取仓库信息。
- 解析提交区间。
- 计算变更文件。
- 生成文件 overlay。
- 聚合作者和提交详情。

文件 overlay 按需计算：先显示文件列表，用户点击某个文件后再计算该文件详细 diff，减少首次等待。

## 缓存、性能与错误处理设计

### 缓存策略

MVP 做两类缓存：

- 持久化项目配置：项目名、路径、固定状态、最近打开时间、默认分支、默认时间范围、默认 glob 规则。
- 会话分析缓存：应用运行期间缓存 Git 读取结果和分析结果，不做 SQLite 索引。

会话缓存按 key 管理：

- `repoPath + branch + timeRange`：提交区间和提交摘要。
- `repoPath + baseCommit + headCommit + globRules`：变更文件列表。
- `repoPath + baseCommit + headCommit + filePath + filtersHash`：单文件 overlay 和块详情。
- `commitHash + filePath`：单提交 patch 摘要。

缓存失效条件：

- 分支 HEAD 变化。
- 项目路径变化或不可访问。
- 筛选条件变化。
- 用户手动刷新。
- Git 命令返回仓库状态变化。

### 性能策略

目标面向几千提交、上千文件的中型仓库，MVP 采用渐进式分析：

- 打开项目时只读取仓库元信息、分支和项目偏好。
- 点击“分析”后先计算提交区间，再计算变更文件列表。
- 文件 overlay 不批量预计算，用户点击文件时按需计算。
- 每个后台任务支持取消；用户修改筛选后取消旧任务。
- 变更文件列表分页或虚拟滚动。
- diff 视图虚拟滚动，避免大文件一次性渲染所有行。
- 大文件设置软限制，例如超过 1MB 或超过 5000 行变更时提示用户确认加载。
- 单文件历史归因先基于 commit patch touched ranges 做区间匹配，避免对每行做昂贵 blame。
- 对 Git 命令设置超时和进度阶段，防止 UI 无响应。

### 并发模型

Electron 主进程维护 `AnalysisTaskManager`：

- 同一项目同一时间只保留一个活跃分析任务。
- 文件 overlay 任务可并发，但限制并发数，建议默认 2。
- 任务有 `pending/running/completed/failed/cancelled` 状态。
- 前端通过 IPC 订阅任务进度或轮询任务状态。
- 任务结果按 task id 获取，避免旧任务结果覆盖新筛选结果。

### 错误处理

可恢复错误：

- Git 命令超时。
- 文件过大。
- 二进制文件不可预览。
- 某个文件 rename 历史无法完整追踪。
- 单个提交 patch 解析失败。

这类错误显示在对应区域，不中断整个分析。

不可恢复错误：

- 路径不是 Git 仓库。
- 仓库路径不存在或权限不足。
- 未安装 Git。
- 分支不存在。
- `baseCommit` 或 `headCommit` 无法解析。

这类错误阻止分析，并在项目页或筛选区明确提示。

### 正确性边界

MVP 明确支持：

- 当前分支或指定分支。
- 最近 30 天默认范围，可修改。
- 作者、提交信息、glob + `!exclude`。
- 文本文件 diff。
- rename/move 识别。
- side-by-side 行级 diff + 修改行词级高亮。
- 块级历史作者聚合和提交详情。

MVP 明确不承诺：

- 完整多父 merge 精确归因。
- 语义级函数/类归并。
- 大规模全仓永久索引。
- 二进制文件内容预览。
- 权限位、copy detection 的完整展示。

## 技术选型与项目结构

MVP 使用一套稳定、低复杂度的桌面端技术栈：

- 构建与 Electron 集成：`electron-vite`。
- 前端：`Vue 3 + TypeScript`。
- 状态管理：`Pinia`。
- 路由：`Vue Router`。
- UI 组件：`Element Plus`。
- 图标：`lucide-vue-next`。
- Git 调用：`simple-git`。
- glob 匹配：`minimatch`。
- 行级和词级 diff：`diff`，即 jsdiff。
- 虚拟滚动：优先使用 `@vueuse/core` / `@vueuse/components`，不够再引入专门虚拟列表库。
- 测试：`Vitest` 做纯逻辑和 main/preload 单元测试，`Playwright` 做关键 UI 验证。
- 包管理：按项目约束使用 `fnm + pnpm`。

项目配置持久化先不用数据库，直接在 Electron `userData` 目录保存 JSON。这样能满足 MVP 的项目管理和默认偏好，不把第一版复杂度推到 SQLite 级别。

建议项目结构：

```text
src/
  main/
    app/
      createWindow.ts
    ipc/
      projectsIpc.ts
      reviewIpc.ts
    projects/
      projectStore.ts
      projectTypes.ts
    git/
      gitService.ts
      gitTypes.ts
    analysis/
      analysisTaskManager.ts
      rangeResolver.ts
      changedFiles.ts
      overlayEngine.ts
      attributionEngine.ts
      patchRanges.ts
    cache/
      sessionCache.ts
    index.ts

  preload/
    index.ts

  renderer/
    main.ts
    App.vue
    router.ts
    stores/
      projectStore.ts
      reviewStore.ts
    pages/
      ProjectHome.vue
      ReviewWorkspace.vue
    components/
      projects/
        ProjectList.vue
        ProjectEditor.vue
      review/
        FilterPanel.vue
        ChangedFileList.vue
        DiffViewer.vue
        DiffBlockAuthors.vue
        BlockDetailPanel.vue
        TaskProgress.vue

  shared/
    ipcTypes.ts
    reviewTypes.ts
    projectTypes.ts
    errors.ts
```

边界保持克制：

- `gitService` 只负责 Git 命令和原始结果转换。
- `analysis` 负责区间、文件、diff、归因。
- `renderer` 只消费结构化结果，不接触 Git 和文件系统。

## 数据类型与 IPC API 设计

MVP 先定义共享类型，让 main/preload/renderer 边界清楚。后续实现时可按代码进一步细化。

### 核心数据类型

```ts
type ProjectId = string;
type TaskId = string;

interface ReviewProject {
  id: ProjectId;
  name: string;
  repoPath: string;
  pinned: boolean;
  lastOpenedAt?: string;
  preferences: ProjectPreferences;
}

interface ProjectPreferences {
  defaultBranch?: string;
  defaultDays?: number;
  defaultGlobRules: string[];
}

interface ReviewFilters {
  projectId: ProjectId;
  branch: string;
  startAt?: string;
  endAt?: string;
  authorQuery?: string;
  messageQuery?: string;
  globRules: string[];
}

interface AnalysisRange {
  branch: string;
  baseCommit: string;
  headCommit: string;
  startAt?: string;
  endAt?: string;
}
```

### 任务与文件

```ts
type AnalysisTaskStatus =
  | 'pending'
  | 'running'
  | 'completed'
  | 'failed'
  | 'cancelled';

type AnalysisStage =
  | 'readRepository'
  | 'resolveRange'
  | 'loadCommits'
  | 'loadChangedFiles'
  | 'ready';

interface AnalysisTaskSnapshot {
  taskId: TaskId;
  projectId: ProjectId;
  status: AnalysisTaskStatus;
  stage: AnalysisStage;
  progress?: number;
  message?: string;
  error?: AppError;
}

interface ChangedFile {
  path: string;
  oldPath?: string;
  status: 'added' | 'modified' | 'deleted' | 'renamed' | 'binary';
  additions: number;
  deletions: number;
  isBinary: boolean;
  isPreviewable: boolean;
}
```

### Overlay 与块详情

```ts
interface FileOverlayRequest {
  taskId: TaskId;
  filePath: string;
}

interface FileOverlay {
  file: ChangedFile;
  range: AnalysisRange;
  blocks: DiffBlock[];
  warnings: AppError[];
}

interface DiffBlock {
  id: string;
  oldStart: number;
  oldEnd: number;
  newStart: number;
  newEnd: number;
  changeType: 'added' | 'deleted' | 'modified';
  authors: AuthorSummary[];
  rows: SideBySideDiffRow[];
  relatedCommits: RelatedCommit[];
}

interface SideBySideDiffRow {
  oldLineNumber?: number;
  newLineNumber?: number;
  oldText?: string;
  newText?: string;
  type: 'context' | 'added' | 'deleted' | 'modified';
  wordChanges?: WordChange[];
}

interface RelatedCommit {
  hash: string;
  shortHash: string;
  authorName: string;
  authorEmail?: string;
  committedAt: string;
  subject: string;
  matchedByFilter: boolean;
  touchedRanges: TouchedRange[];
}
```

### IPC API

Preload 暴露受控的 `window.revier`：

```ts
interface RevierApi {
  projects: {
    list(): Promise<ReviewProject[]>;
    add(repoPath: string, options?: Partial<ReviewProject>): Promise<ReviewProject>;
    update(project: ReviewProject): Promise<ReviewProject>;
    remove(projectId: ProjectId): Promise<void>;
    validateRepository(repoPath: string): Promise<RepositoryValidation>;
    listBranches(projectId: ProjectId): Promise<GitBranch[]>;
  };

  review: {
    startAnalysis(filters: ReviewFilters): Promise<AnalysisTaskSnapshot>;
    cancelAnalysis(taskId: TaskId): Promise<void>;
    getTask(taskId: TaskId): Promise<AnalysisTaskSnapshot>;
    onTaskUpdate(callback: (task: AnalysisTaskSnapshot) => void): Unsubscribe;
    listChangedFiles(taskId: TaskId): Promise<ChangedFile[]>;
    getFileOverlay(request: FileOverlayRequest): Promise<FileOverlay>;
  };
}
```

### 错误模型

```ts
interface AppError {
  code:
    | 'NOT_A_GIT_REPOSITORY'
    | 'REPOSITORY_NOT_FOUND'
    | 'GIT_NOT_INSTALLED'
    | 'BRANCH_NOT_FOUND'
    | 'GIT_COMMAND_TIMEOUT'
    | 'FILE_TOO_LARGE'
    | 'BINARY_FILE'
    | 'PATCH_PARSE_FAILED'
    | 'UNKNOWN';
  message: string;
  recoverable: boolean;
  detail?: string;
}
```

这个 API 设计的重点是：前端只知道项目、筛选、任务、文件和 overlay；Git 命令、缓存、归因算法都封在主进程。

## 测试与验收设计

MVP 的测试重点放在算法正确性、中型仓库性能和 Electron 边界稳定。

### 逻辑单元测试

用 `Vitest` 覆盖纯逻辑模块，不依赖真实 Electron 窗口：

- `rangeResolver`：验证默认最近 30 天、显式开始/结束时间、空提交区间、分支 HEAD 解析。
- `changedFiles`：验证 added/modified/deleted/renamed/binary 的解析。
- `globRules`：验证 include/exclude，例如 `src/**/*.ts` + `!**/*.test.ts`。
- `overlayEngine`：验证 side-by-side 行级 diff、修改行词级高亮、连续变更块聚合。
- `patchRanges`：验证 commit patch 到 touched ranges 的转换。
- `attributionEngine`：验证单作者、多作者交叉修改、作者/提交信息筛选命中标记。

这些测试尽量用小型内存样例，不跑真实 Git，保证反馈快。

### Git 集成测试

用 `Vitest` 在临时目录生成真实 Git 仓库，测试 `simple-git + git CLI` 封装：

- 初始化仓库并创建多次提交。
- 构造多作者提交。
- 构造 rename/move。
- 构造删除文件。
- 构造二进制文件。
- 构造提交信息筛选。
- 验证 `baseCommit/headCommit/rangeCommits` 和 `git diff --name-status --find-renames` 解析结果。

这些测试要求本机安装 Git；若 Git 不可用，集成测试应明确跳过或失败为环境错误。

### Electron / UI 验证

用 `Playwright` 做少量关键路径：

- 启动应用后显示项目页。
- 添加一个 fixture 仓库项目。
- 打开项目，默认加载分支和最近 30 天筛选。
- 点击分析后出现变更文件列表。
- 点击文本文件后显示 side-by-side diff。
- 点击变更块后右侧显示作者和相关提交。
- 二进制文件显示不可预览。
- 修改筛选条件后旧任务不会覆盖新结果。

UI 测试不追求覆盖所有样式，重点验证主流程和任务状态。

### 性能验收

中型仓库验收采用生成式 fixture，而不是提交一个巨大测试仓库到项目里。测试脚本生成：

- 约 1000 到 3000 次提交。
- 约 500 到 1500 个文件。
- 多作者。
- 部分 rename。
- 部分大文件和二进制文件。

MVP 性能目标：

- 打开项目并读取分支：2 秒内。
- 生成变更文件列表：中型仓库常规筛选 10 秒内。
- 点击单个普通文本文件生成 overlay：3 秒内。
- UI 在后台分析期间不阻塞。
- 修改筛选后旧任务能取消，最终只展示最新任务结果。

这些阈值作为开发机基线，不作为跨机器绝对承诺；测试报告记录机器环境和仓库规模。

### 手工验收样例

保留一组可人工复核的 fixture 场景：

- 同一代码块先由 A 修改，再由 B 修改，作者列表显示 A 和 B。
- 某个提交被后续提交覆盖，最终 diff 只显示净变化，详情仍能看到中间提交。
- 作者筛选命中块内历史提交时，显示完整块，并在详情中标记命中提交。
- glob 排除测试文件后，测试文件不出现在文件列表。
- rename 后的文件在列表中显示旧路径到新路径。

### 发布前验证命令

后续实现完成后，发布前至少运行：

```bash
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

如果加入性能脚本，再运行：

```bash
pnpm test:perf
```
