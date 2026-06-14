# Revier 首页项目入口调整设计

> 状态：待用户审核。本文记录首页从左右两栏调整为“最近项目 / welcome 信息 + 按需打开项目弹窗”的设计方案。

## 目标

当前首页启动后同时展示左侧新增项目表单和右侧项目列表。新的目标是让首页更聚焦：

- 启动后不再使用左右两栏。
- 有项目时优先展示最近项目列表。
- 无项目时展示 welcome 空状态。
- 用户点击“打开项目”后，再弹出模式对话框选择仓库目录和填写项目信息。
- 通过弹窗添加项目成功后，直接进入该项目的 review 工作台。

## 已确认需求

- 首页顶部保留 `Revier`、项目数量和刷新入口。
- 首页新增主操作按钮 `打开项目`。
- 最近项目列表中已有项目的 `打开` 操作保持不变，直接进入 review 工作台。
- `打开项目` 按钮打开模式对话框。
- 模式对话框中继续提供仓库路径选择和项目名称输入。
- 新项目提交成功后，自动跳转到 `/review/:projectId`。
- 项目管理、持久化、目录选择和 review 工作台逻辑不扩展新功能。

## 技术选型说明

本次调整沿用现有技术栈：

- 渲染层：`Vue 3 + TypeScript`。
- 状态管理：`Pinia`，继续使用 `useProjectStore`。
- UI 组件：`naive-ui`，使用 `n-modal` 或等价的现有模式对话框能力。
- 图标：`lucide-vue-next`，继续使用现有 `FolderOpen` 图标。
- 路由：`vue-router`，继续通过命名路由 `review` 跳转。
- 包管理与脚本：按项目规范使用 `fnm + pnpm`。

不新增第三方依赖。

## 架构设计

本次改动只涉及渲染层项目首页，不改变 main、preload、shared IPC 边界。

组件职责：

- `ProjectHome.vue`
  - 负责首页布局。
  - 控制“打开项目”模式对话框显示状态。
  - 调用项目 store 加载、添加、移除项目。
  - 添加项目成功后定位新增项目并跳转 review 工作台。

- `ProjectEditor.vue`
  - 继续负责仓库路径、项目名称表单。
  - 继续通过 `window.revier.projects.selectDirectory()` 选择目录。
  - 继续通过 `submit` 事件向父组件提交 `{ repoPath, name }`。

- `ProjectList.vue`
  - 继续负责展示项目表格和行级打开、移除操作。
  - 不负责新增项目。

## 功能模块划分

### 首页头部

顶部包含：

- 左侧品牌信息：`Revier` 和项目数量。
- 右侧操作区：`刷新`、`打开项目`。

`打开项目` 是主按钮；`刷新` 是次级按钮。

### 最近项目区

有项目时显示项目列表：

- 表格字段沿用当前 `ProjectList`。
- 表格外层不再处于左右两栏中的右栏卡片。
- 行内 `打开` 操作仍进入已有项目的 review 工作台。

### Welcome 空状态

无项目时显示 welcome 信息：

- 标题说明当前还没有项目。
- 简短提示用户打开本地 Git 仓库开始 review。
- 提供 `打开项目` 按钮。

空状态只用于引导操作，不做营销页、不添加插画和装饰性内容。

### 打开项目对话框

模式对话框包含：

- 标题：`打开项目`。
- 内容：复用 `ProjectEditor` 表单。
- 用户选择仓库目录后自动填充路径和项目名称。
- 用户提交后关闭弹窗并进入新项目 review 工作台。

若添加项目失败：

- 保留在弹窗中。
- 使用现有 `projectStore.error` 在页面上方或弹窗上下文中显示错误。
- 不跳转。

## 数据流程设计

### 启动加载

1. `ProjectHome` 挂载。
2. 调用 `projectStore.loadProjects()`。
3. 页面根据 `projects.length` 显示最近项目列表或 welcome 空状态。

### 打开已有项目

1. 用户点击项目列表行内 `打开`。
2. `ProjectHome.openProject(projectId)` 调用路由跳转。
3. 进入 `review` 路由。

### 添加并打开新项目

1. 用户点击首页 `打开项目`。
2. `ProjectHome` 打开模式对话框。
3. 用户在 `ProjectEditor` 中选择目录并提交。
4. `ProjectHome.addProjectAndOpen(payload)` 调用 `projectStore.addProject(repoPath, name)`。
5. store 通过 preload IPC 添加项目并刷新项目列表。
6. 首页根据 `repoPath` 在刷新后的 `projects` 中定位新项目。
7. 定位成功后关闭弹窗，并跳转 `review` 路由。
8. 定位失败时保留错误提示，不跳转。

## 接口规范定义

### 组件事件

`ProjectEditor` 事件保持不变：

```ts
submit: [payload: { repoPath: string; name?: string }]
```

### Store 接口

优先不修改 `useProjectStore` 公开接口：

```ts
loadProjects(): Promise<void>
addProject(repoPath: string, name?: string): Promise<void>
removeProject(projectId: string): Promise<void>
```

`addProject` 当前会刷新 `projects`，首页可在调用完成后使用 `projects` 定位新增项目。

如果实现时发现无法可靠定位新增项目，再单独提交变更确认，考虑让 `addProject` 返回 `ReviewProject`。当前设计不默认修改 store 接口。

### 路由接口

继续使用现有命名路由：

```ts
router.push({ name: 'review', params: { projectId } })
```

## UI 与交互规范

- 首页不使用左右两栏。
- 不使用大 hero、营销文案、装饰插画或渐变背景。
- 主内容使用单列工具型布局。
- 表格保持紧凑、可扫描。
- welcome 信息保持克制，突出可执行操作。
- 弹窗宽度保持适合表单输入，不占满窗口。
- 按钮文案统一使用中文。
- 错误信息沿用当前项目错误提示风格。

## 错误处理

- `loadProjects` 失败：沿用现有顶部错误提示。
- `addProject` 失败：显示 store 中的错误，不关闭弹窗，不跳转。
- 目录选择取消：保持当前表单不变。
- 新增后无法定位项目：显示“项目已添加，但未能定位项目记录，请刷新后重试”一类错误；具体实现若需要新增 store 错误写入方式，需先确认。

为避免扩大范围，当前实现优先只处理已有错误通道能表达的场景。

## 测试设计

### 单元测试

现有 `ProjectEditor` 单元测试继续覆盖：

- 点击目录选择按钮。
- 自动填充仓库路径和项目名称。

本次若只移动承载位置，不需要修改该测试逻辑。

### E2E 测试

调整 `tests/e2e/review-workflow.spec.ts`：

- 启动后确认首页可见。
- 点击 `打开项目`。
- 在弹窗中填写仓库路径和项目名称。
- 提交后直接进入 review 工作台。
- 后续分析流程保持现有断言。

### 验证命令

实现完成后至少运行：

```bash
pnpm typecheck
pnpm test -- projectEditor
pnpm test:e2e
```

若时间允许，补充运行：

```bash
pnpm test
```

## 开发环境配置指南

本项目沿用现有环境：

1. 使用 `fnm` 切换到 `.node-version` 指定的 Node 版本。
2. 使用 `pnpm install` 安装依赖。
3. 使用 `pnpm dev` 启动 Electron 开发环境。
4. 使用 `pnpm test` 运行单元测试。
5. 使用 `pnpm test:e2e` 运行端到端测试。

本次改动不要求新增环境变量或系统依赖。

## 非目标

- 不修改项目持久化格式。
- 不新增最近项目排序规则。
- 不新增项目编辑偏好功能。
- 不改变 review 工作台布局。
- 不新增移动端专门设计。

## 自检

- 本文没有未完成占位词。
- 需求、数据流和测试设计一致：新项目通过弹窗添加后直接跳转。
- 方案保持在首页入口调整范围内，没有扩展项目管理能力。
- 当前设计明确不修改 store 接口；若实现中需要修改，会重新确认。
