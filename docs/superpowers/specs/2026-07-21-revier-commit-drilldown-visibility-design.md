# Revier 提交下钻可见性与边界处理设计

> 状态：已确认。本文记录 2026-07-21 提交下钻显示范围及返回入口缺失问题的修复设计。

## 1. 背景与问题

评审工作台默认在中栏展示分析范围的文件级完整 Diff。右侧相关提交列表支持打开提交级下钻，但当前存在两个问题：

1. 提交级接口虽然返回“第一父提交与目标提交”的文件内容，Monaco Diff 仍完整显示两侧文件，用户无法直观看出当前提交独有的变更。
2. 下钻容器仅在请求加载中或已有 overlay 时挂载。请求失败后容器被卸载，中栏重新露出文件级 Diff，顶部返回按钮同时消失。

部分请求失败来自相关提交与提交级接口的边界不一致：归因结果可能包含当前分析范围之前但可从 `headCommit` 追溯的来源提交；根提交没有父提交；文件经过多次重命名后，当前文件路径无法直接匹配历史提交中的路径。

## 2. 目标与非目标

### 2.1 目标

- 提交级下钻只突出显示所选提交对文件造成的变更。
- 未变更区域默认折叠，变更前后各保留 3 行上下文，并允许用户展开。
- 点击相关提交后，下钻容器持续覆盖中栏，加载成功、加载中和加载失败时始终提供返回按钮。
- 支持当前 `headCommit` 可追溯的范围外来源提交、根提交和历史重命名路径。
- 无法生成提交级 Diff 时，在下钻容器内展示明确错误，不回退显示底层文件级 Diff。

### 2.2 非目标

- 不改变文件级完整 Diff 的显示方式。
- 不修改变更块归因算法、筛选逻辑或相关提交排序。
- 不新增重试按钮、提交导航、键盘快捷键或其他交互。
- 不改变 TDD 测试用例既有业务口径；仅新增覆盖并同步已确认的新契约。

## 3. 架构与技术选型

沿用现有 Vue 3、Pinia、Monaco Editor、Tauri 与 Rust 服务架构，不增加第三方依赖。

- 展示层：`DiffDrilldownOverlay.vue` 负责下钻壳、标题、返回入口和错误态。
- Diff 复用层：`DiffViewer.vue`、`MonacoDiffSurface.vue` 透传“折叠未变更区域”配置。
- 编辑器层：`monacoDiffSession.ts` 仅在提交级下钻启用 Monaco 原生 `hideUnchangedRegions`，配置 `contextLineCount: 3`。
- 状态层：`reviewStore.ts` 保存当前提交元数据与下钻错误，关闭下钻时统一清理。
- 服务层：`review.rs` 校验目标提交可从任务 `headCommit` 追溯，构造目标提交相对第一父提交的文件 Diff；根提交使用空树作为旧侧。
- Git 分析层：复用或补充提交可达性、历史文件路径解析能力，不通过 UI 传入不可信的历史路径。

## 4. 功能模块划分

### 4.1 提交级折叠显示

`DiffViewer` 增加默认关闭的提交级显示选项。`DiffDrilldownOverlay` 显式开启该选项，文件级 `DiffViewer` 不传入，保持完整显示。

Monaco 配置：

```ts
hideUnchangedRegions: {
  enabled: true,
  contextLineCount: 3
}
```

Monaco 自带的折叠区控件负责展开未变更内容，不增加自定义按钮。

### 4.2 下钻生命周期与错误态

点击提交后立即记录所选提交元数据并清空上一次错误。下钻组件以“存在所选提交”为主要挂载条件，而不是依赖 overlay 请求是否成功。

- 加载中：显示提交摘要、返回按钮、取消按钮及加载状态。
- 成功：显示提交摘要、返回按钮和提交级 Diff。
- 失败：保留提交摘要与返回按钮，在内容区显示错误信息。
- 返回或取消：使当前请求失效，并清理所选提交、overlay、错误和 loading 状态。

### 4.3 提交与路径边界

- 范围外来源提交：由“必须位于 `base..head`”调整为“必须为当前任务 `headCommit` 的可达祖先（含 head）”。这样归因追溯出的早期来源提交可以下钻，同时拒绝无关分支对象。
- 根提交：以 Git 空树作为第一父提交的等价旧侧，新增文件旧内容为空。
- 历史路径：从任务当前文件路径及已知 `oldPath` 出发，沿 `headCommit` 到目标提交方向跟踪重命名，解析目标提交边界上的真实路径，再匹配该提交的文件变更。
- 多父提交：继续采用第一父提交语义，与现有提交级 Diff 契约一致。
- 无法匹配：返回结构化错误，由前端错误态承载。

## 5. 数据流程

```text
右侧点击 RelatedCommit
  -> ReviewWorkspace.openCommitDrilldown(commit)
  -> reviewStore 记录 selectedCommit 并进入 loading
  -> review_get_commit_overlay(taskId, currentFilePath, commitHash, encoding)
  -> 校验 commit 可从任务 head 追溯
  -> 解析目标提交时的历史路径与第一父提交（根提交为空树）
  -> 构建 FileOverlay(mode=commit)
  -> 成功：下钻 DiffViewer 启用未变更区折叠
  -> 失败：下钻壳保留并展示错误
  -> 返回：清理状态，恢复文件级 Diff
```

## 6. 接口规范

外部 Tauri 命令及 `CommitOverlayRequest` 字段保持不变：

- `taskId`：已完成分析任务 ID。
- `filePath`：任务文件列表中的当前路径。
- `commitHash`：右侧相关提交哈希。
- `encoding`：可选文本编码。

`FileOverlay` 成功响应结构保持不变。失败继续使用结构化命令错误；前端新增专用下钻错误状态，不修改公共绑定结构。

## 7. 测试策略

- `diffDrilldownOverlay.test.ts`：失败后容器和返回按钮仍存在；提交摘要稳定；错误内容可见。
- `monacoDiffSurface.test.ts` / `monacoDiffSession.test.ts`：提交级配置透传并启用 3 行上下文折叠；默认文件级配置不启用。
- `rendererReviewStore.test.ts`：失败保留所选提交和错误；关闭后完整清理；过期请求不能覆盖新状态。
- `reviewWorkspace.test.ts`：点击提交传递完整元数据和错误状态。
- Rust 服务测试：范围外但可达来源提交成功；不可达提交被拒绝；根提交成功；历史重命名路径成功。
- 回归：现有提交 overlay、编码、二进制文件、竞态取消与文件级 Diff 测试保持通过。

## 8. 开发环境与验证命令

Node 环境按用户指定方式初始化：

```powershell
& 'C:\Users\Snowind\AppData\Local\Microsoft\WinGet\Links\fnm.exe' env --use-on-cd --shell powershell | Out-String | Invoke-Expression
pnpm test
pnpm typecheck
```

Rust 使用工作区现有工具链：

```powershell
cargo test -p revier-tauri
```

若包名与实际清单不同，以 `Cargo.toml` 中工作区包名为准执行等价的定向测试。

## 9. 验收标准

- 点击任一相关提交后，中栏立即出现且持续保留下钻壳和返回按钮。
- 成功加载时仅展开提交变更及其前后 3 行上下文，未变更区可手动展开。
- 文件级 Diff 仍完整显示。
- 请求失败时中栏显示错误且可返回，不露出底层文件级 Diff。
- 当前 head 可追溯的范围外来源提交、根提交及历史重命名提交能生成正确提交级 Diff。
- 相关前端与 Rust 测试、类型检查全部通过。
