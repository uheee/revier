# Revier MVP 实施计划

> **给执行代理的要求：** 实施本计划时必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans`，逐任务执行。每个任务使用 checkbox（`- [ ]`）跟踪进度。

**目标：** 构建 Revier MVP 桌面应用，支持打开本地 Git 仓库、筛选 review 区间、列出变更文件，并查看带变更块作者归因的 side-by-side 叠加 diff。

**架构：** Electron 主进程负责 Git 访问、项目配置持久化、分析任务、会话缓存和 IPC。Preload 暴露受控的 `window.revier` 类型化 API。Vue 渲染进程负责项目管理、筛选表单、变更文件导航、side-by-side diff、任务进度和变更块详情。

**技术栈：** Vue 3、TypeScript、Electron、electron-vite、Pinia、Vue Router、Element Plus、lucide-vue-next、simple-git、minimatch、diff、Vitest、Playwright、fnm、pnpm。

---

## 规格来源

以 `docs/superpowers/specs/2026-06-03-revier-mvp-design.md` 为产品和技术规格来源。本计划只实现已确认的 MVP 范围。

## 总体文件结构

最终项目结构按下列边界组织：

```text
package.json
pnpm-lock.yaml
.node-version
.gitignore
electron.vite.config.ts
index.html
tsconfig.json
tsconfig.node.json
tsconfig.web.json
vitest.config.ts
playwright.config.ts

src/
  main/
    app/
      createWindow.ts
    ipc/
      projectsIpc.ts
      reviewIpc.ts
    projects/
      projectStore.ts
    git/
      gitService.ts
      gitTypes.ts
    analysis/
      analysisTaskManager.ts
      attributionEngine.ts
      changedFiles.ts
      globRules.ts
      overlayEngine.ts
      patchRanges.ts
      rangeResolver.ts
    cache/
      sessionCache.ts
    index.ts

  preload/
    index.ts

  renderer/
    App.vue
    main.ts
    router.ts
    styles.css
    stores/
      projectStore.ts
      reviewStore.ts
    pages/
      ProjectHome.vue
      ReviewWorkspace.vue
    components/
      projects/
        ProjectEditor.vue
        ProjectList.vue
      review/
        BlockDetailPanel.vue
        ChangedFileList.vue
        DiffBlockAuthors.vue
        DiffViewer.vue
        FilterPanel.vue
        TaskProgress.vue

  shared/
    errors.ts
    ipcTypes.ts
    projectTypes.ts
    reviewTypes.ts

tests/
  fixtures/
    gitRepo.ts
  integration/
    gitService.test.ts
    reviewAnalysis.test.ts
  perf/
    mediumRepo.perf.test.ts
  unit/
    analysisTaskManager.test.ts
    attributionEngine.test.ts
    changedFiles.test.ts
    globRules.test.ts
    overlayEngine.test.ts
    patchRanges.test.ts
    projectStore.test.ts
    rangeResolver.test.ts
    sharedTypes.test.ts
  e2e/
    review-workflow.spec.ts
```

## 任务 1：初始化工程脚手架与工具链

**文件：**

- 创建：`.node-version`
- 创建：`.gitignore`
- 创建：`package.json`
- 创建：`electron.vite.config.ts`
- 创建：`index.html`
- 创建：`tsconfig.json`
- 创建：`tsconfig.node.json`
- 创建：`tsconfig.web.json`
- 创建：`vitest.config.ts`
- 创建：`playwright.config.ts`
- 创建：`src/main/index.ts`
- 创建：`src/main/app/createWindow.ts`
- 创建：`src/preload/index.ts`
- 创建：`src/renderer/main.ts`
- 创建：`src/renderer/App.vue`
- 创建：`src/renderer/styles.css`
- 创建：`tests/unit/scaffold.test.ts`

- [ ] **步骤 1：先写脚手架冒烟测试**

在 `tests/unit/scaffold.test.ts` 中验证：

- `package.json` 的 `name` 为 `revier`。
- `package.json` 的 `main` 为 `dist/main/index.js`。

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
fnm use 22
pnpm test -- --run tests/unit/scaffold.test.ts
```

预期：失败，因为脚手架和测试脚本尚未创建。

- [ ] **步骤 3：创建 Electron + Vue + TypeScript 工程**

实现要求：

- `.node-version` 固定为 `22`。
- `.gitignore` 排除 `node_modules/`、`dist/`、`out/`、`coverage/`、`test-results/`、`playwright-report/`、`.vite/`、`.superpowers/`。
- `package.json` 配置 `dev`、`build`、`preview`、`typecheck`、`test`、`test:e2e`、`test:perf`、`lint`。
- 依赖包含 `vue`、`electron`、`electron-vite`、`pinia`、`vue-router`、`element-plus`、`lucide-vue-next`、`simple-git`、`minimatch`、`diff`、`vitest`、`playwright`。
- `electron.vite.config.ts` 配置 main、preload、renderer 三个入口，并设置 `@renderer`、`@shared` 路径别名。
- 主进程创建 `BrowserWindow`，启用 `contextIsolation`，禁用 `nodeIntegration`。
- Renderer 初始只渲染一个 `Revier` 标题。

- [ ] **步骤 4：安装依赖**

运行：

```powershell
fnm use 22
pnpm install
```

预期：退出码为 0，并生成 `pnpm-lock.yaml`。

- [ ] **步骤 5：验证脚手架**

运行：

```powershell
pnpm test -- --run tests/unit/scaffold.test.ts
pnpm typecheck
pnpm build
```

预期：全部退出码为 0。

- [ ] **步骤 6：提交**

运行：

```powershell
git add .node-version .gitignore package.json pnpm-lock.yaml electron.vite.config.ts index.html tsconfig.json tsconfig.node.json tsconfig.web.json vitest.config.ts playwright.config.ts src tests/unit/scaffold.test.ts
git commit -m "chore: 初始化 Electron Vue 工程"
```

## 任务 2：定义共享契约与错误模型

**文件：**

- 创建：`src/shared/errors.ts`
- 创建：`src/shared/projectTypes.ts`
- 创建：`src/shared/reviewTypes.ts`
- 创建：`src/shared/ipcTypes.ts`
- 创建：`src/renderer/env.d.ts`
- 创建：`tests/unit/sharedTypes.test.ts`

- [ ] **步骤 1：先写共享契约测试**

在 `tests/unit/sharedTypes.test.ts` 中验证：

- `createAppError()` 能创建可恢复和不可恢复错误。
- `isRecoverableError()` 按 `recoverable` 返回布尔值。
- `ReviewProject` 支持项目偏好：默认分支、默认天数、默认 glob 规则。
- `ReviewFilters` 支持分支、作者、提交信息、glob 规则。

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/sharedTypes.test.ts
```

预期：失败，因为 shared 类型文件尚不存在。

- [ ] **步骤 3：实现 shared 类型**

实现要求：

- `errors.ts` 导出 `AppErrorCode`、`AppError`、`createAppError()`、`isRecoverableError()`。
- `projectTypes.ts` 导出 `ProjectId`、`ProjectPreferences`、`ReviewProject`、`RepositoryValidation`、`GitBranch`。
- `reviewTypes.ts` 导出 `TaskId`、`ReviewFilters`、`AnalysisRange`、`AnalysisTaskSnapshot`、`ChangedFile`、`FileOverlay`、`DiffBlock`、`SideBySideDiffRow`、`RelatedCommit`、`TouchedRange`、`WordChange`。
- `ipcTypes.ts` 导出 `RevierApi` 和 `Unsubscribe`。
- `src/renderer/env.d.ts` 将 `window.revier` 声明为 `RevierApi`。

- [ ] **步骤 4：验证共享契约**

运行：

```powershell
pnpm test -- --run tests/unit/sharedTypes.test.ts
pnpm typecheck
```

预期：全部退出码为 0。

- [ ] **步骤 5：提交**

运行：

```powershell
git add src/shared src/renderer/env.d.ts tests/unit/sharedTypes.test.ts
git commit -m "feat: 定义共享 IPC 契约"
```

## 任务 3：项目存储与 Git 仓库校验

**文件：**

- 创建：`src/main/projects/projectStore.ts`
- 创建：`tests/unit/projectStore.test.ts`
- 创建：`src/main/git/gitService.ts`
- 创建：`tests/integration/gitService.test.ts`
- 创建：`tests/fixtures/gitRepo.ts`

- [ ] **步骤 1：先写项目存储测试**

在 `tests/unit/projectStore.test.ts` 中验证：

- `JsonProjectStore.add()` 能保存项目名、仓库路径、固定状态、最近打开时间和偏好。
- `JsonProjectStore.list()` 能从 JSON 文件读取项目。
- `JsonProjectStore.update()` 能更新项目。
- `JsonProjectStore.remove()` 能按 id 移除项目。

- [ ] **步骤 2：运行测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/projectStore.test.ts
```

预期：失败，因为 `JsonProjectStore` 尚不存在。

- [ ] **步骤 3：实现 JSON 项目存储**

实现要求：

- 使用 Electron `userData` 中的 JSON 文件持久化项目列表。
- 缺省偏好为 `defaultDays: 30`、`defaultGlobRules: []`。
- 项目 id 使用 `crypto.randomUUID()`。
- 读取文件不存在时返回空项目列表。
- 写入前确保父目录存在。

- [ ] **步骤 4：写 Git fixture 与仓库校验测试**

在 `tests/fixtures/gitRepo.ts` 中提供：

- `initTestRepo(path)`：创建真实 Git 仓库，配置测试用户名和邮箱，提交初始 `README.md`。

在 `tests/integration/gitService.test.ts` 中验证：

- `GitService.validateRepository()` 能识别真实 Git 仓库。
- `GitService.listBranches()` 能返回本地分支，并标记当前分支。

- [ ] **步骤 5：运行 Git 测试确认失败**

运行：

```powershell
pnpm test -- --run tests/integration/gitService.test.ts
```

预期：失败，因为 `GitService` 尚不存在。

- [ ] **步骤 6：实现 Git 仓库校验**

实现要求：

- 使用 `simple-git(repoPath)`。
- `validateRepository(repoPath)` 调用 `checkIsRepo()` 和 `branch()`。
- `listBranches(repoPath)` 调用 `branchLocal()` 并返回 `{ name, current }`。
- 异常时返回 `valid: false` 和错误信息。

- [ ] **步骤 7：验证**

运行：

```powershell
pnpm test -- --run tests/unit/projectStore.test.ts tests/integration/gitService.test.ts
pnpm typecheck
```

预期：全部退出码为 0。

- [ ] **步骤 8：提交**

运行：

```powershell
git add src/main/projects src/main/git tests/unit/projectStore.test.ts tests/integration/gitService.test.ts tests/fixtures/gitRepo.ts
git commit -m "feat: 添加项目存储和仓库校验"
```

## 任务 4：解析提交区间与变更文件

**文件：**

- 修改：`src/main/git/gitService.ts`
- 创建：`src/main/git/gitTypes.ts`
- 创建：`src/main/analysis/rangeResolver.ts`
- 创建：`src/main/analysis/changedFiles.ts`
- 创建：`tests/unit/changedFiles.test.ts`
- 创建：`tests/unit/rangeResolver.test.ts`
- 修改：`tests/fixtures/gitRepo.ts`

- [ ] **步骤 1：扩展 Git fixture**

在 `tests/fixtures/gitRepo.ts` 中增加 `commitFiles(repo, options)`：

- 支持提交多个文件。
- 支持指定作者姓名、作者邮箱、提交信息、提交日期。
- 返回提交后的 `HEAD` hash。

- [ ] **步骤 2：先写提交区间测试**

在 `tests/unit/rangeResolver.test.ts` 中验证：

- 未提供时间时默认使用最近 30 天。
- 显式开始/结束时间能选出正确的 `baseCommit`、`headCommit` 和 `rangeCommits`。
- 提交按时间排序后再计算区间。

- [ ] **步骤 3：运行提交区间测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/rangeResolver.test.ts
```

预期：失败，因为 `rangeResolver.ts` 尚不存在。

- [ ] **步骤 4：实现提交区间解析**

实现要求：

- 导出 `resolveDefaultDateRange(input, now)`。
- 导出 `selectRangeCommits(commits, input)`。
- `startAt` 缺省时从 `endAt` 或当前时间往前推 30 天。
- `baseCommit` 为开始时间之前目标分支上的最后一个提交。
- `headCommit` 为结束时间之前目标分支上的最后一个提交。
- 没有可用提交时抛出明确错误。

- [ ] **步骤 5：先写变更文件解析测试**

在 `tests/unit/changedFiles.test.ts` 中验证：

- `parseNameStatus()` 能解析 added、modified、deleted、renamed。
- `parseNumstat()` 能填充 additions、deletions。
- `-	-	path` 被识别为二进制文件，`isPreviewable` 为 `false`。

- [ ] **步骤 6：运行变更文件测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/changedFiles.test.ts
```

预期：失败，因为 `changedFiles.ts` 尚不存在。

- [ ] **步骤 7：实现变更文件解析**

实现要求：

- `normalizeGitPath(path)` 将路径统一为 `/`。
- `parseNameStatus(output)` 解析 `git diff --name-status --find-renames`。
- `parseNumstat(files, output)` 合并 `git diff --numstat` 统计。
- rename 文件设置 `oldPath`。
- 二进制文件设置 `status: 'binary'`、`isBinary: true`、`isPreviewable: false`。

- [ ] **步骤 8：扩展 GitService**

实现要求：

- `src/main/git/gitTypes.ts` 定义 `GitCommitSummary`。
- `GitService.listCommits(repoPath, branch)` 返回 hash、shortHash、作者、邮箱、提交时间、标题。
- `GitService.listChangedFiles(repoPath, baseCommit, headCommit)` 调用 `git diff --name-status --find-renames` 和 `git diff --numstat`，返回 `ChangedFile[]`。

- [ ] **步骤 9：验证**

运行：

```powershell
pnpm test -- --run tests/unit/rangeResolver.test.ts tests/unit/changedFiles.test.ts tests/integration/gitService.test.ts
pnpm typecheck
```

预期：全部退出码为 0。

- [ ] **步骤 10：提交**

运行：

```powershell
git add src/main/git src/main/analysis/rangeResolver.ts src/main/analysis/changedFiles.ts tests/fixtures/gitRepo.ts tests/unit/rangeResolver.test.ts tests/unit/changedFiles.test.ts tests/integration/gitService.test.ts
git commit -m "feat: 解析提交区间和变更文件"
```

## 任务 5：Glob 筛选、会话缓存与分析任务管理

**文件：**

- 创建：`src/main/analysis/globRules.ts`
- 创建：`src/main/cache/sessionCache.ts`
- 创建：`src/main/analysis/analysisTaskManager.ts`
- 创建：`tests/unit/globRules.test.ts`
- 创建：`tests/unit/analysisTaskManager.test.ts`

- [ ] **步骤 1：先写 glob 测试**

在 `tests/unit/globRules.test.ts` 中验证：

- `src/**/*.ts` 能匹配 TypeScript 源文件。
- `!**/*.test.ts` 能排除测试文件。
- 没有规则时匹配所有路径。

- [ ] **步骤 2：运行 glob 测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/globRules.test.ts
```

预期：失败，因为 `globRules.ts` 尚不存在。

- [ ] **步骤 3：实现 glob 匹配**

实现要求：

- 使用 `minimatch`。
- 规则以 `!` 开头时作为 exclude。
- include 为空时默认包含所有文件。
- 匹配前统一路径分隔符为 `/`。

- [ ] **步骤 4：先写任务管理测试**

在 `tests/unit/analysisTaskManager.test.ts` 中验证：

- 同一项目创建新任务时，旧活跃任务被标记为 `cancelled`。
- `onUpdate()` 能收到 `running` 和 `completed` 快照。
- `getTask(taskId)` 能返回当前快照。

- [ ] **步骤 5：运行任务管理测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/analysisTaskManager.test.ts
```

预期：失败，因为 `AnalysisTaskManager` 尚不存在。

- [ ] **步骤 6：实现 SessionCache 和 AnalysisTaskManager**

实现要求：

- `SessionCache<T>` 使用 `Map`，提供 `get()`、`set()`、`delete()`、`clear()`。
- `AnalysisTaskManager` 使用 `randomUUID()` 创建 task id。
- 状态支持 `pending`、`running`、`completed`、`failed`、`cancelled`。
- 阶段支持 `readRepository`、`resolveRange`、`loadCommits`、`loadChangedFiles`、`ready`。
- 同一项目只保留一个活跃任务。
- 支持订阅任务更新并返回 unsubscribe 函数。

- [ ] **步骤 7：验证**

运行：

```powershell
pnpm test -- --run tests/unit/globRules.test.ts tests/unit/analysisTaskManager.test.ts
pnpm typecheck
```

预期：全部退出码为 0。

- [ ] **步骤 8：提交**

运行：

```powershell
git add src/main/analysis/globRules.ts src/main/analysis/analysisTaskManager.ts src/main/cache/sessionCache.ts tests/unit/globRules.test.ts tests/unit/analysisTaskManager.test.ts
git commit -m "feat: 添加筛选匹配和分析任务管理"
```

## 任务 6：文件叠加 diff 引擎

**文件：**

- 创建：`src/main/analysis/overlayEngine.ts`
- 创建：`tests/unit/overlayEngine.test.ts`

- [ ] **步骤 1：先写 overlay 测试**

在 `tests/unit/overlayEngine.test.ts` 中验证：

- 修改行生成 `modified` 行。
- 修改行包含词级 `wordChanges`。
- 相邻新增行聚合为同一个 `DiffBlock`。
- `oldStart/oldEnd/newStart/newEnd` 行号正确。

- [ ] **步骤 2：运行 overlay 测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/overlayEngine.test.ts
```

预期：失败，因为 `overlayEngine.ts` 尚不存在。

- [ ] **步骤 3：实现 overlay 引擎**

实现要求：

- 使用 `diffLines()` 生成行级 diff。
- 对删除后紧跟新增的组合视为 `modified`。
- 对修改行使用 `diffWords()` 生成词级高亮数据。
- 输出 `SideBySideDiffRow[]`。
- 将连续非 context 行聚合成 `DiffBlock[]`。
- 初始 `authors` 和 `relatedCommits` 为空，后续由归因引擎填充。

- [ ] **步骤 4：验证**

运行：

```powershell
pnpm test -- --run tests/unit/overlayEngine.test.ts
pnpm typecheck
```

预期：全部退出码为 0。

- [ ] **步骤 5：提交**

运行：

```powershell
git add src/main/analysis/overlayEngine.ts tests/unit/overlayEngine.test.ts
git commit -m "feat: 添加文件叠加 diff 引擎"
```

## 任务 7：Patch 行区间与变更块归因

**文件：**

- 创建：`src/main/analysis/patchRanges.ts`
- 创建：`src/main/analysis/attributionEngine.ts`
- 创建：`tests/unit/patchRanges.test.ts`
- 创建：`tests/unit/attributionEngine.test.ts`

- [ ] **步骤 1：先写 patch range 测试**

在 `tests/unit/patchRanges.test.ts` 中验证：

- 能从 unified patch hunk 头解析旧侧和新侧 touched ranges。
- 例如 `@@ -10,2 +10,3 @@` 解析为旧侧 10-11、新侧 10-12。

- [ ] **步骤 2：运行 patch range 测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/patchRanges.test.ts
```

预期：失败，因为 `patchRanges.ts` 尚不存在。

- [ ] **步骤 3：实现 patch range 解析**

实现要求：

- `parsePatchTouchedRanges(patch)` 使用 hunk 正则解析 touched ranges。
- `rangesIntersect(block, touched)` 判断旧侧或新侧区间是否相交。
- 没有行号的一侧不参与相交判断。

- [ ] **步骤 4：先写归因测试**

在 `tests/unit/attributionEngine.test.ts` 中验证：

- 与 `DiffBlock` 行区间相交的提交会进入 `relatedCommits`。
- 作者按姓名和邮箱去重后进入 `authors`。
- 不相交提交不会进入当前变更块。

- [ ] **步骤 5：运行归因测试确认失败**

运行：

```powershell
pnpm test -- --run tests/unit/attributionEngine.test.ts
```

预期：失败，因为 `attributionEngine.ts` 尚不存在。

- [ ] **步骤 6：实现归因引擎**

实现要求：

- `attachAttribution(blocks, commits)` 遍历每个 `DiffBlock`。
- 将 block 行号转换成 `TouchedRange`。
- 使用 `rangesIntersect()` 找到相关提交。
- 聚合相关提交作者，输出到 `authors`。
- 保留 `matchedByFilter` 字段供详情面板标记命中。

- [ ] **步骤 7：验证**

运行：

```powershell
pnpm test -- --run tests/unit/patchRanges.test.ts tests/unit/attributionEngine.test.ts
pnpm typecheck
```

预期：全部退出码为 0。

- [ ] **步骤 8：提交**

运行：

```powershell
git add src/main/analysis/patchRanges.ts src/main/analysis/attributionEngine.ts tests/unit/patchRanges.test.ts tests/unit/attributionEngine.test.ts
git commit -m "feat: 添加变更块作者归因"
```

## 任务 8：分析编排与 Review IPC

**文件：**

- 创建：`src/main/ipc/projectsIpc.ts`
- 创建：`src/main/ipc/reviewIpc.ts`
- 修改：`src/main/index.ts`
- 修改：`src/preload/index.ts`
- 修改：`src/shared/ipcTypes.ts`
- 修改：`src/main/git/gitService.ts`

- [ ] **步骤 1：定义 IPC channel 常量**

在 `src/shared/ipcTypes.ts` 中导出 `ipcChannels`：

- `projects:list`
- `projects:add`
- `projects:update`
- `projects:remove`
- `projects:validateRepository`
- `projects:listBranches`
- `review:startAnalysis`
- `review:cancelAnalysis`
- `review:getTask`
- `review:listChangedFiles`
- `review:getFileOverlay`
- `review:taskUpdated`

- [ ] **步骤 2：实现 preload API**

实现要求：

- 使用 `contextBridge.exposeInMainWorld('revier', api)`。
- `projects` API 转发项目列表、新增、更新、移除、校验仓库、列分支。
- `review` API 转发开始分析、取消分析、获取任务、订阅任务更新、列变更文件、获取文件 overlay。
- `onTaskUpdate()` 监听 `review:taskUpdated`，返回 unsubscribe。

- [ ] **步骤 3：实现 projects IPC**

实现要求：

- 在 `registerProjectsIpc()` 中创建 `JsonProjectStore`。
- 项目配置文件路径为 `join(app.getPath('userData'), 'projects.json')`。
- 注册所有 project IPC handler。
- `listBranches(projectId)` 先从 store 找项目，再调用 `GitService.listBranches(repoPath)`。

- [ ] **步骤 4：实现 review IPC 纵向切片**

实现要求：

- `registerReviewIpc(projectStore)` 创建 `GitService` 和 `AnalysisTaskManager`。
- `review:startAnalysis` 创建任务、保存 filters、读取项目、加载提交、加载变更文件、保存到 `filesByTask`。
- `review:cancelAnalysis` 标记任务取消。
- `review:getTask` 返回任务快照。
- `review:listChangedFiles` 返回任务对应文件列表。
- `review:getFileOverlay` 先返回可编译的空 overlay，任务 11 再替换为真实文件内容。
- 任务更新通过 `BrowserWindow.getAllWindows().forEach()` 广播给 renderer。

- [ ] **步骤 5：在 main 入口注册 IPC**

修改 `src/main/index.ts`：

- `app.whenReady()` 后先注册 projects IPC。
- 将返回的 project store 传给 review IPC。
- 再创建窗口。

- [ ] **步骤 6：验证**

运行：

```powershell
pnpm typecheck
pnpm build
```

预期：全部退出码为 0。

- [ ] **步骤 7：提交**

运行：

```powershell
git add src/main/ipc src/main/index.ts src/preload/index.ts src/shared/ipcTypes.ts
git commit -m "feat: 接入项目和分析 IPC"
```

## 任务 9：项目管理界面

**文件：**

- 修改：`src/renderer/main.ts`
- 修改：`src/renderer/App.vue`
- 创建：`src/renderer/router.ts`
- 创建：`src/renderer/stores/projectStore.ts`
- 创建：`src/renderer/pages/ProjectHome.vue`
- 创建：`src/renderer/components/projects/ProjectEditor.vue`
- 创建：`src/renderer/components/projects/ProjectList.vue`

- [ ] **步骤 1：接入 Pinia、Router 和 Element Plus**

实现要求：

- `src/renderer/main.ts` 使用 `ElementPlus`、`router`。
- `src/renderer/router.ts` 使用 hash 路由。
- `/` 指向 `ProjectHome`。
- `/review/:projectId` 指向 `ReviewWorkspace`，组件可懒加载。

- [ ] **步骤 2：实现项目 store**

在 `src/renderer/stores/projectStore.ts` 中实现：

- 状态：`projects`、`loading`、`error`。
- `loadProjects()` 调用 `window.revier.projects.list()`。
- `addProject(repoPath, name)` 调用 `window.revier.projects.add()` 后刷新列表。
- `removeProject(projectId)` 调用 `window.revier.projects.remove()` 后刷新列表。

- [ ] **步骤 3：实现项目页和组件**

实现要求：

- `ProjectHome.vue` 显示标题、项目编辑器和项目列表。
- `ProjectEditor.vue` 包含仓库路径、项目名称输入框和添加按钮，按钮使用 `FolderOpen` 图标。
- `ProjectList.vue` 使用 Element Plus table 展示名称、路径、默认分支和操作。
- 操作包含打开和移除。
- 打开项目跳转到 `/review/:projectId`。

- [ ] **步骤 4：验证**

运行：

```powershell
pnpm typecheck
pnpm build
```

预期：全部退出码为 0。

- [ ] **步骤 5：提交**

运行：

```powershell
git add src/renderer
git commit -m "feat: 添加项目管理界面"
```

## 任务 10：Review 工作台界面

**文件：**

- 创建：`src/renderer/stores/reviewStore.ts`
- 创建：`src/renderer/pages/ReviewWorkspace.vue`
- 创建：`src/renderer/components/review/FilterPanel.vue`
- 创建：`src/renderer/components/review/ChangedFileList.vue`
- 创建：`src/renderer/components/review/DiffViewer.vue`
- 创建：`src/renderer/components/review/DiffBlockAuthors.vue`
- 创建：`src/renderer/components/review/BlockDetailPanel.vue`
- 创建：`src/renderer/components/review/TaskProgress.vue`

- [ ] **步骤 1：实现 review store**

在 `src/renderer/stores/reviewStore.ts` 中实现：

- 状态：`task`、`files`、`overlay`、`selectedBlock`、`loading`、`error`。
- `start(filters)` 调用 `review.startAnalysis()`，再调用 `review.listChangedFiles()`。
- `loadOverlay(filePath)` 调用 `review.getFileOverlay()`。
- `selectBlock(block)` 更新右侧详情。

- [ ] **步骤 2：实现筛选和进度组件**

`FilterPanel.vue`：

- 输入分支、开始时间、结束时间、作者、提交信息、glob 多行规则。
- 提交时输出完整 `ReviewFilters`。
- glob 规则按换行拆分并去掉空行。

`TaskProgress.vue`：

- 有错误时显示 `el-alert`。
- 运行中显示进度条。
- 完成或取消时显示当前任务状态和阶段。

- [ ] **步骤 3：实现变更文件列表**

`ChangedFileList.vue`：

- 接收 `ChangedFile[]`。
- 显示路径、rename 的旧路径到新路径、变更类型、增删行数。
- 点击文件时 emit `selected(path)`。

- [ ] **步骤 4：实现 diff 与详情组件**

`DiffBlockAuthors.vue`：

- 将 `authors` 渲染为小尺寸 `el-tag`。

`DiffViewer.vue`：

- 渲染 `FileOverlay.blocks`。
- 每个块显示作者标签。
- 使用两列布局展示旧侧和新侧行号、文本。
- 修改行渲染 `wordChanges`，新增和删除词用不同 class。
- 点击块时 emit `selected(block)`。
- 二进制文件显示“不可预览”状态。

`BlockDetailPanel.vue`：

- 未选择块时显示空状态。
- 选择块后显示作者、相关提交列表、hash、标题、作者、时间。
- `matchedByFilter` 为 true 的提交显示“命中筛选”标签。

- [ ] **步骤 5：实现工作台页面**

`ReviewWorkspace.vue` 使用三栏布局：

- 左侧：`FilterPanel`、`TaskProgress`、`ChangedFileList`。
- 中间：`DiffViewer` 或空状态。
- 右侧：`BlockDetailPanel`。

- [ ] **步骤 6：补充工具型布局样式**

在 `src/renderer/styles.css` 中补充：

- 项目页间距。
- 项目编辑器网格布局。
- Review 三栏布局：左 320px、中间自适应、右 320px。
- 左右栏滚动。
- diff 区域滚动。
- 文件列表单行省略。

- [ ] **步骤 7：验证**

运行：

```powershell
pnpm typecheck
pnpm build
```

预期：全部退出码为 0。

- [ ] **步骤 8：提交**

运行：

```powershell
git add src/renderer
git commit -m "feat: 添加 review 工作台界面"
```

## 任务 11：接入真实文件内容与混合筛选语义

**文件：**

- 修改：`src/main/git/gitService.ts`
- 修改：`src/main/ipc/reviewIpc.ts`
- 修改：`src/main/analysis/globRules.ts`
- 修改：`src/main/analysis/attributionEngine.ts`
- 创建：`tests/integration/reviewAnalysis.test.ts`

- [ ] **步骤 1：先写混合筛选语义集成测试**

在 `tests/integration/reviewAnalysis.test.ts` 中构造真实 Git 仓库并验证：

- `baseCommit..headCommit` 生成最终净变化。
- `src/**/*.ts` 和 `!**/*.test.ts` 只显示源文件，不显示测试文件。
- `GitService.readFileAtCommit()` 能读取 base/head 文件内容。
- `buildFileOverlayBlocks()` 能基于真实内容生成变更块。

- [ ] **步骤 2：运行集成测试确认失败**

运行：

```powershell
pnpm test -- --run tests/integration/reviewAnalysis.test.ts
```

预期：失败，因为 `readFileAtCommit()` 尚不存在，review IPC 也尚未应用完整筛选语义。

- [ ] **步骤 3：扩展 GitService**

实现要求：

- `readFileAtCommit(repoPath, commit, filePath)` 调用 `git show commit:path`。
- 文件在某个 commit 不存在时返回空字符串。
- `showFilePatch(repoPath, commit, filePath)` 调用 `git show --format= --find-renames commit -- filePath`。

- [ ] **步骤 4：更新 review IPC 分析流程**

`review:startAnalysis` 实现要求：

- 按 `filters.projectId` 找项目。
- 调用 `git.listCommits(repoPath, filters.branch)`。
- 调用 `selectRangeCommits(commits, filters)`。
- 用 `range.baseCommit` 和 `range.headCommit` 计算变更文件。
- 用 `createGlobMatcher(filters.globRules)` 过滤文件。
- 按 `taskId` 保存 range、filters 和可见文件。

`review:getFileOverlay` 实现要求：

- 根据 task 查到 range 和 file。
- 二进制文件返回 warning，不读取内容。
- 文本文件读取 base 和 head 内容。
- 调用 `buildFileOverlayBlocks()` 生成 blocks。
- 后续将相关提交的 touched ranges 传给 `attachAttribution()`。

- [ ] **步骤 5：验证**

运行：

```powershell
pnpm test -- --run tests/integration/reviewAnalysis.test.ts tests/unit/globRules.test.ts tests/unit/overlayEngine.test.ts
pnpm typecheck
```

预期：全部退出码为 0。

- [ ] **步骤 6：提交**

运行：

```powershell
git add src/main/git src/main/ipc/reviewIpc.ts src/main/analysis tests/integration/reviewAnalysis.test.ts
git commit -m "feat: 接入真实文件叠加分析"
```

## 任务 12：端到端流程与性能验收

**文件：**

- 创建：`tests/e2e/review-workflow.spec.ts`
- 创建：`tests/perf/mediumRepo.perf.test.ts`
- 修改：`tests/fixtures/gitRepo.ts`
- 修改：`package.json`

- [ ] **步骤 1：添加 E2E 冒烟流程**

在 `tests/e2e/review-workflow.spec.ts` 中验证：

- 应用首页显示 `Revier`。
- 仓库路径输入框可见。
- 项目入口 UI 可交互。

- [ ] **步骤 2：添加中型仓库性能 fixture**

在 `tests/perf/mediumRepo.perf.test.ts` 中：

- 临时生成真实 Git 仓库。
- 创建 120 次提交作为自动化性能基线。
- 多作者交替提交。
- 调用 `GitService.listChangedFiles()`。
- 断言文件数正确。
- 断言耗时小于 10 秒。

说明：自动化测试使用 120 次提交以保持本地和 CI 可运行；人工性能验收可把生成规模提高到 1000-3000 次提交。

- [ ] **步骤 3：运行 E2E 和性能测试**

运行：

```powershell
pnpm test:e2e
pnpm test:perf
```

预期：全部退出码为 0。若本机缺少 Playwright 浏览器，先运行：

```powershell
pnpm exec playwright install
```

然后重新运行 `pnpm test:e2e`。

- [ ] **步骤 4：运行发布前验证**

运行：

```powershell
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

预期：全部退出码为 0。

- [ ] **步骤 5：提交**

运行：

```powershell
git add tests/e2e tests/perf tests/fixtures/gitRepo.ts package.json pnpm-lock.yaml
git commit -m "test: 添加端到端和性能验收"
```

## 自检清单

规格覆盖：

- 项目管理和仓库默认偏好：任务 3、任务 9。
- Git 分支、时间区间、变更文件、rename、二进制识别：任务 4、任务 11。
- glob include/exclude：任务 5、任务 11。
- side-by-side 行级 diff 和词级高亮：任务 6、任务 10。
- 变更块历史作者归因：任务 7。
- 任务化分析和会话缓存基础：任务 5、任务 8。
- 类型化 IPC 边界：任务 2、任务 8。
- Review 工作台 UI：任务 10。
- 单元测试、集成测试、E2E 和性能 fixture：任务 12。

类型一致性：

- 项目 id 使用 `ProjectId`。
- 任务 id 使用 `TaskId`。
- Main、preload、renderer 共享 `RevierApi`、`ReviewFilters`、`ChangedFile`、`FileOverlay`、`DiffBlock`、`RelatedCommit`。
- 核心类名统一为 `GitService`、`AnalysisTaskManager`、`JsonProjectStore`。

验证纪律：

- 每个任务都有失败测试或失败编译检查。
- 每个任务都有通过验证命令。
- 每个任务都以语义化提交结束。
