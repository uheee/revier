# Revier Monaco 可交互 Diff 编辑器 Implementation Plan

> **给 agentic workers：** 必须使用 `superpowers:subagent-driven-development`（推荐）或 `superpowers:executing-plans` 按任务执行本计划。所有步骤使用 checkbox（`- [ ]`）语法跟踪。

**目标：** 将 Revier 当前自绘 side-by-side Diff 升级为 Monaco 原生 Diff Editor，使左右两侧可临时编辑但永不保存，并完整实现语言高亮、编码切换、应用级 TOML 主题、真实块选择和外置 AuthorRail。

**架构：** Rust 继续作为 Git Blob、解码、真实 Diff 块和归因的事实来源，并通过 Tauri 返回完整左右文本与设置快照；Vue 用一个受控 Monaco 会话渲染和销毁内存 Model，原始态叠加真实块交互，首次编辑后切换为无归因草稿态。应用主题、Monaco Theme 与 Shiki Theme 都由启动时加载的 `editor.toml` 快照派生。

**技术栈：** Rust stable、Tauri 2、Vue 3 `<script setup>`、TypeScript、Pinia、Monaco Editor、Shiki、`@shikijs/monaco`、Vite Module Worker、Vitest、Vue Test Utils、Cargo test、TOML、`encoding_rs`、`pnpm`。

---

## 范围检查

本计划只实现已确认规格：

- `docs/superpowers/specs/2026-07-16-revier-monaco-editor-design.md`
- `docs/superpowers/specs/2026-07-16-revier-monaco-editor-visual.html`
- `docs/superpowers/specs/2026-07-16-revier-monaco-editor-visual.png`

设置、编码、Overlay 契约、Monaco 会话、AuthorRail 和整应用主题构成一条不可拆分的交付链：缺少任一环节都无法完成“可编辑但不保存、语言/编码可切换、归因不遮挡代码”的验收。因此保留为一份分阶段计划，每个任务独立测试、独立提交。

本计划不实现保存、导出、LSP、自动补全承诺、诊断、调试、终端、设置页面、TOML 热重载、草稿归因或旧自绘 Diff 运行时回退。

## 执行顺序勘误（2026-07-17）

Task 2 将 `AuthorSummary` 与 `FileOverlay` 的新增字段设为必需字段后，现有 `src-tauri/src/services/review.rs` 构造器会在 Task 5 适配前无法编译，因此原顺序中的 Task 3 无法先运行 `revier-tauri` 设置服务测试。

经用户书面确认，权威执行顺序调整为：

```text
Task 1 → Task 2 → Task 4 → Task 5 → Task 3 → Task 6 → Task 7 → Task 8 → Task 9 → Task 10 → Task 11 → Task 12 → Task 13
```

Task 编号与章节位置保持不变，以维持既有提交、审查记录和引用稳定。该调整只修正依赖顺序，不改变任何功能、文件范围、测试要求或验收标准。

## 已核实的公开 API

- Shiki 官方 Monaco 集成继续使用 `createHighlighter` 与 `shikiToMonaco`：<https://shiki.style/packages/monaco>
- Monaco Diff Editor 公开支持 `originalEditable`：<https://microsoft.github.io/monaco-editor/typedoc/interfaces/editor_editor_api.editor.IDiffEditorOptions.html>
- Monaco Worker 使用 `MonacoEnvironment.getWorker` 和 Vite `?worker` 导入；不得访问 Monaco 私有 DOM。

## 文件结构

### 新增文件

- `crates/revier-analysis/src/text_encoding.rs`
  - 确定性解析 `auto`，执行 UTF-8、GB18030、UTF-16 LE/BE 严格解码。
- `crates/revier-analysis/tests/text_encoding.rs`
  - 覆盖 BOM、合法/非法字节、手动编码和二进制拒绝。
- `src-tauri/src/services/editor_settings.rs`
  - 创建、读取、验证并缓存 `<app_config_dir>/editor.toml`。
- `src-tauri/src/commands/editor_settings.rs`
  - 暴露 `editor_settings_get`。
- `src/renderer/editor/editorLanguages.ts`
  - 文件名/扩展名到 Monaco/Shiki 语言 ID 的唯一注册表。
- `src/renderer/editor/editorTheme.ts`
  - 字体回落、CSS 变量、Naive UI、Monaco/Shiki 主题转换纯函数。
- `src/renderer/editor/monacoEnvironment.ts`
  - Monaco Worker 与 Shiki 初始化。
- `src/renderer/editor/monacoDiffSession.ts`
  - 创建、更新、销毁 Diff Editor/Model/Decoration/Disposable 的适配层。
- `src/renderer/editor/diffBlockGeometry.ts`
  - 行号命中、可见块几何、作者排序和裁剪纯函数。
- `src/renderer/composables/useEditorSettings.ts`
  - 启动设置快照、系统主题监听和根主题应用。
- `src/renderer/components/review/MonacoDiffSurface.vue`
  - Monaco 适配层的 Vue 生命周期外壳。
- `src/renderer/components/review/DiffAuthorRail.vue`
  - 外置 112px 作者轨道、`…` 与向左 Popover。
- `src/renderer/components/review/EditorStatusBar.vue`
  - 状态、光标、编码和语言上拉选择。
- `tests/unit/editorLanguages.test.ts`
- `tests/unit/editorTheme.test.ts`
- `tests/unit/monacoDiffSession.test.ts`
- `tests/unit/diffBlockGeometry.test.ts`
- `tests/unit/monacoDiffSurface.test.ts`
- `tests/unit/diffAuthorRail.test.ts`
- `tests/unit/editorStatusBar.test.ts`

### 修改文件

- `package.json`、`pnpm-lock.yaml`
  - 加入 Monaco/Shiki 依赖。
- `crates/revier-analysis/Cargo.toml`、`src-tauri/Cargo.toml`、`Cargo.lock`
  - 加入 `encoding_rs`、`toml`。
- `crates/revier-analysis/src/lib.rs`
  - 导出编码模块。
- `crates/revier-analysis/src/cli.rs`
  - `file-overlay` 接收 `--encoding`。
- `crates/revier-analysis/src/commands/trace_block.rs`
  - 构造内部 `FileOverlayArgs` 时显式沿用 `auto`，保持 trace-block 现有 CLI 语义。
- `crates/revier-analysis/src/contracts.rs`
  - 增加编码、完整文本、作者统计和编辑器设置契约。
- `crates/revier-analysis/src/bindings.rs`、`src/renderer/generated/bindings.ts`
  - 注册并重新生成契约。
- `crates/revier-analysis/src/git/blob.rs`
  - 从 Blob 字节进入严格解码。
- `crates/revier-analysis/src/git/diff.rs`、`crates/revier-analysis/src/commands/query_files.rs`
  - 内部保留真实变更类型，并让非 UTF-8 文本仍可出现在文件列表中。
- `crates/revier-analysis/src/attribution/patch_inference.rs`、`blame.rs`、`merge_trace.rs`、`deletion_trace.rs`
  - 将同一 resolved encoding 贯穿所有历史 Blob 读取，保证非 UTF-8 继续执行完整归因链。
- `crates/revier-analysis/src/overlay/file_overlay.rs`、`crates/revier-analysis/src/json.rs`
  - 输出完整文本与实际编码。
- `crates/revier-analysis/tests/fixtures.rs`
  - 为共享 `FileOverlayArgs` 测试夹具补充 `auto` 编码。
- `crates/revier-analysis/tests/index_git_diff.rs`
  - 将二进制夹具的内部 status 断言同步为真实变更类型，同时保留二进制与不可预览断言。
- `src-tauri/src/state.rs`、`src-tauri/src/lib.rs`、`src-tauri/src/services/mod.rs`、`src-tauri/src/commands/mod.rs`
  - 启动时加载一次设置并注册命令。
- `src-tauri/src/services/review.rs`
  - 传递编码、返回完整文本、聚合作者统计。
- `src/renderer/api/revierClient.ts`
  - 增加设置命令客户端。
- `src/renderer/main.ts`、`src/renderer/App.vue`
  - 挂载前加载设置与语法运行时，提供完整浅/深主题和非阻断配置 warning。
- `src/renderer/stores/reviewStore.ts`
  - 编码成功后原子替换 Overlay，失败保留当前视图。
- `src/renderer/components/review/DiffViewer.vue`
  - 改为三态容器并组合 Monaco、AuthorRail、状态栏。
- `src/renderer/components/review/DiffDrilldownOverlay.vue`
  - 复用新编辑器并转发编码/草稿事件。
- `src/renderer/components/review/BlockDetailPanel.vue`
  - 草稿态显示“临时草稿不提供归因”。
- `src/renderer/pages/ReviewWorkspace.vue`
  - 管理文件/提交切换、草稿清理和编码重载。
- `src/renderer/styles.css`
  - 替换旧自绘 Diff 样式，建立全局主题、选中边界与 IDE 视觉。
- `tests/unit/revierClient.test.ts`
- `tests/unit/rendererReviewStore.test.ts`
- `tests/unit/diffViewer.test.ts`
- `tests/unit/diffViewerSelectionStyles.test.ts`
- `tests/unit/diffDrilldownOverlay.test.ts`
- `tests/unit/blockDetailPanel.test.ts`
- `tests/unit/reviewWorkspace.test.ts`
  - 更新契约夹具并覆盖新交互。

### 删除文件

- `src/renderer/components/review/DiffBlockAuthors.vue`
  - 作者信息由外置 `DiffAuthorRail.vue` 完整替代。

## 执行前确认与环境门槛

- [ ] **Step 1: 获取测试变更书面确认**

执行者必须让用户明确确认：允许修改上方列出的现有测试文件，包括更新 `FileOverlay` / `AuthorSummary` 夹具、替换旧自绘行 DOM 断言和新增草稿/编码交互断言。未确认不得开始 Task 1。

- [ ] **Step 2: 检查用户改动**

运行：

```powershell
git status --short
```

预期：只允许出现已知的用户文件，例如当前已存在的 `.idea/`；不得删除、覆盖或提交无关改动。

- [ ] **Step 3: 激活 Node 24 与工具链**

运行：

```powershell
fnm use 24
node --version
pnpm --version
rustc --version
cargo --version
```

预期：Node 为 `v24.x`，pnpm 为 `10.x`，Rust/Cargo 可用。缺少任一工具时停止实施并报告环境阻塞。

- [ ] **Step 4: 记录基线**

运行：

```powershell
pnpm test
cargo test --workspace
pnpm typecheck
```

预期：全部通过。若基线失败，只记录并报告，不把无关失败混入本功能。

---

### Task 1: 安装并锁定编辑器、语法、编码和 TOML 依赖

**Files:**
- Modify: `package.json`
- Modify: `pnpm-lock.yaml`
- Modify: `crates/revier-analysis/Cargo.toml`
- Modify: `src-tauri/Cargo.toml`
- Modify: `Cargo.lock`

- [ ] **Step 1: 安装 Node 依赖**

运行：

```powershell
pnpm add monaco-editor shiki @shikijs/monaco
```

预期：三个包进入 `dependencies`，`pnpm-lock.yaml` 更新，无 peer dependency 错误。

- [ ] **Step 2: 安装 Rust 依赖**

运行：

```powershell
cargo add encoding_rs --package revier-analysis
cargo add toml --package revier-tauri
```

预期：`encoding_rs` 只进入分析 crate，`toml` 只进入 Tauri crate，`Cargo.lock` 更新。

- [ ] **Step 3: 验证依赖可解析**

运行：

```powershell
pnpm typecheck
cargo check --workspace
```

预期：基线代码仍通过；尚未引用新依赖。

- [ ] **Step 4: 提交依赖**

运行：

```powershell
git add package.json pnpm-lock.yaml crates/revier-analysis/Cargo.toml src-tauri/Cargo.toml Cargo.lock
git commit -m "build(editor): 添加 Monaco 与编码配置依赖"
```

---

### Task 2: 定义跨 Rust/Tauri/TypeScript 的唯一数据契约

**Files:**
- Create: `crates/revier-analysis/tests/editor_contract.rs`
- Modify: `crates/revier-analysis/src/contracts.rs`
- Modify: `crates/revier-analysis/src/bindings.rs`
- Modify: `src/renderer/generated/bindings.ts`

- [ ] **Step 1: 编写失败的序列化契约测试**

新增 `crates/revier-analysis/tests/editor_contract.rs`，完整覆盖以下 JSON 值：

```rust
use revier_analysis::contracts::{
    ResolvedTextEncoding, TextEncoding,
};

#[test]
fn text_encoding_serializes_to_confirmed_wire_values() {
    assert_eq!(serde_json::to_string(&TextEncoding::Auto).unwrap(), "\"auto\"");
    assert_eq!(serde_json::to_string(&TextEncoding::Utf8).unwrap(), "\"utf-8\"");
    assert_eq!(serde_json::to_string(&TextEncoding::Gb18030).unwrap(), "\"gb18030\"");
    assert_eq!(serde_json::to_string(&TextEncoding::Utf16Le).unwrap(), "\"utf-16le\"");
    assert_eq!(serde_json::to_string(&TextEncoding::Utf16Be).unwrap(), "\"utf-16be\"");
    assert_eq!(
        serde_json::to_string(&ResolvedTextEncoding::Utf16Le).unwrap(),
        "\"utf-16le\""
    );
}
```

同时新增一个 `EditorSettingsSnapshot` 序列化测试，断言字段名为 `configPath`、`defaultEncoding`、`fontFamilies`、`largeFile`、`workspaceBackground`、`diffRemovedStrong`，且 warning 缺失时不输出。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test editor_contract
```

预期：因 `TextEncoding`、`ResolvedTextEncoding` 和设置类型尚不存在而编译失败。

- [ ] **Step 3: 增加精确契约**

在 `contracts.rs` 新增并注册以下类型；编码枚举必须用逐项 `serde(rename)`，不得依赖自动大小写转换：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum TextEncoding {
    #[serde(rename = "auto")]
    Auto,
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "gb18030")]
    Gb18030,
    #[serde(rename = "utf-16le")]
    Utf16Le,
    #[serde(rename = "utf-16be")]
    Utf16Be,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
pub enum ResolvedTextEncoding {
    #[serde(rename = "utf-8")]
    Utf8,
    #[serde(rename = "gb18030")]
    Gb18030,
    #[serde(rename = "utf-16le")]
    Utf16Le,
    #[serde(rename = "utf-16be")]
    Utf16Be,
}
```

再新增以下完整设置契约；IPC 使用 camelCase，TOML service 用私有 snake_case 结构转换：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "lowercase")]
pub enum EditorThemeMode { System, Light, Dark }

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorFontSettings {
    pub font_families: Vec<String>,
    pub font_size: u32,
    pub line_height: u32,
    pub minimap: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LargeFileSettings {
    pub max_bytes: u64,
    pub max_lines: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorSyntaxColors {
    pub comment: String,
    pub keyword: String,
    pub string: String,
    pub number: String,
    pub r#type: String,
    pub function: String,
    pub variable: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorThemeColors {
    pub workspace_background: String,
    pub panel_background: String,
    pub editor_background: String,
    pub border: String,
    pub foreground: String,
    pub muted: String,
    pub accent: String,
    pub selection: String,
    pub diff_removed: String,
    pub diff_removed_strong: String,
    pub diff_removed_word: String,
    pub diff_added: String,
    pub diff_added_strong: String,
    pub diff_added_word: String,
    pub syntax: EditorSyntaxColors,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorThemes {
    pub light: EditorThemeColors,
    pub dark: EditorThemeColors,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettings {
    pub version: u32,
    pub theme: EditorThemeMode,
    pub default_encoding: TextEncoding,
    pub editor: EditorFontSettings,
    pub large_file: LargeFileSettings,
    pub themes: EditorThemes,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EditorSettingsSnapshot {
    pub settings: EditorSettings,
    pub config_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub warning: Option<String>,
}
```

修改现有契约：

```rust
pub struct FileOverlayRequest {
    pub task_id: TaskId,
    pub file_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::TextEncoding)]
    pub encoding: Option<TextEncoding>,
}

pub struct CommitOverlayRequest {
    pub task_id: TaskId,
    pub file_path: String,
    pub commit_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::TextEncoding)]
    pub encoding: Option<TextEncoding>,
}

pub struct AuthorSummary {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub email: Option<String>,
    pub commit_count: u64,
    pub last_committed_at: String,
}

pub struct FileOverlay {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::FileOverlayMode)]
    pub mode: Option<FileOverlayMode>,
    pub file: ChangedFile,
    pub range: AnalysisRange,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::vec::Vec<crate::contracts::SideBySideDiffRow>)]
    pub rows: Option<Vec<SideBySideDiffRow>>,
    pub blocks: Vec<DiffBlock>,
    pub warnings: Vec<AppError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = crate::contracts::RelatedCommit)]
    pub commit: Option<RelatedCommit>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[specta(optional, type = std::string::String)]
    pub parent_hash: Option<String>,
    pub old_content: String,
    pub new_content: String,
    pub resolved_encoding: ResolvedTextEncoding,
}
```

以上现有 struct 继续保留其 `Debug, Clone, Serialize, Deserialize, Type` derives 和 camelCase serde 属性。

- [ ] **Step 4: 注册并生成 TypeScript bindings**

在 `bindings.rs` 注册所有新增类型，然后运行：

```powershell
pnpm generate:bindings
cargo test -p revier-analysis --test editor_contract
```

预期：测试通过；生成的 TypeScript 中 `TextEncoding` 包含 `auto`，`ResolvedTextEncoding` 不包含 `auto`。

- [ ] **Step 5: 检查生成文件没有手工漂移**

运行：

```powershell
pnpm generate:bindings:check
```

预期：命令通过，第二次生成不再产生 diff。

- [ ] **Step 6: 提交契约**

运行：

```powershell
git add crates/revier-analysis/src/contracts.rs crates/revier-analysis/src/bindings.rs crates/revier-analysis/tests/editor_contract.rs src/renderer/generated/bindings.ts
git commit -m "feat(editor): 定义编辑器设置与文本编码契约"
```

---

### Task 3: 启动时创建、校验并缓存 editor.toml

> **执行依赖：** 只有 Task 4 与 Task 5 已完成、`cargo check -p revier-tauri` 恢复通过后才能开始本任务。不得用临时空文本、固定 UTF-8 或作者统计占位值绕过编译错误。

**Files:**
- Create: `src-tauri/src/services/editor_settings.rs`
- Create: `src-tauri/src/commands/editor_settings.rs`
- Modify: `src-tauri/src/services/mod.rs`
- Modify: `src-tauri/src/commands/mod.rs`
- Modify: `src-tauri/src/state.rs`
- Modify: `src-tauri/src/lib.rs`

- [ ] **Step 1: 在新 service 内先写失败测试**

测试模块至少包含四个具名测试：

- `creates_confirmed_default_toml_when_file_is_missing`：断言完整默认值与写入文件逐字段一致。
- `loads_complete_valid_toml`：断言 theme、font、large_file、light/dark 和 syntax。
- `keeps_broken_file_and_returns_defaults_with_warning`：先保存原始字节，load 后断言文件字节完全不变。
- `rejects_unknown_theme_and_invalid_hex_color_as_whole_snapshot`：分别输入非法枚举/颜色并断言整套默认回退。

测试使用 `tempfile::tempdir()`，不得读取用户真实配置目录。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
cargo test -p revier-tauri editor_settings
```

预期：因 service 尚未实现而编译失败。

- [ ] **Step 3: 实现私有 TOML 结构和严格校验**

`editor_settings.rs` 使用私有 snake_case 文件结构 `EditorSettingsFile`，转换到公开 camelCase IPC 契约。实现固定入口：

```rust
pub struct EditorSettingsService {
    snapshot: EditorSettingsSnapshot,
}

impl EditorSettingsService {
    pub fn load(path: PathBuf) -> Self;
    pub fn snapshot(&self) -> EditorSettingsSnapshot;
}
```

实现规则：

- 缺失时先创建父目录，再用 `toml::to_string_pretty` 写入设计文档的完整默认值。
- 文件损坏、缺字段、未知枚举、`version != 1`、字号/行高非正数、阈值为 0 或颜色不是 `#[0-9A-Fa-f]{6}` 时，整套回退内置默认值。
- 损坏文件绝不覆盖；warning 必须含配置路径和具体字段/解析错误。
- 创建默认文件失败时仍返回内置默认值和 warning，使应用可启动。
- `load` 只在 Tauri `setup` 调用一次；command 不重新读盘。
- 默认文件必须逐字段等同设计文档中的 TOML，包括 `JetBrainsMono Nerd Font Mono`、`Microsoft YaHei`、`monospace` 的顺序和完整 light/dark syntax 色值。

- [ ] **Step 4: 接入 AppState 与 Tauri command**

命令保持无参数：

```rust
#[tauri::command]
pub fn editor_settings_get(
    state: tauri::State<'_, AppState>,
) -> EditorSettingsSnapshot {
    state.editor_settings.snapshot()
}
```

`lib.rs` 必须使用：

```rust
let config_dir = app.path().app_config_dir()?;
let editor_settings = EditorSettingsService::load(config_dir.join("editor.toml"));
```

并把 command 注册进 `generate_handler!`。

- [ ] **Step 5: 运行 Rust 测试**

运行：

```powershell
cargo test -p revier-tauri editor_settings
cargo test -p revier-tauri
```

预期：设置测试和 Tauri crate 全部通过。

- [ ] **Step 6: 提交设置服务**

运行：

```powershell
git add src-tauri/src/services/editor_settings.rs src-tauri/src/commands/editor_settings.rs src-tauri/src/services/mod.rs src-tauri/src/commands/mod.rs src-tauri/src/state.rs src-tauri/src/lib.rs
git commit -m "feat(settings): 从 TOML 加载编辑器设置"
```

---

### Task 4: 严格解码 Git Blob 并向 Overlay 返回完整文本

**Files:**
- Create: `crates/revier-analysis/src/text_encoding.rs`
- Create: `crates/revier-analysis/tests/text_encoding.rs`
- Modify: `crates/revier-analysis/src/lib.rs`
- Modify: `crates/revier-analysis/src/cli.rs`
- Modify: `crates/revier-analysis/src/commands/trace_block.rs`
- Modify: `crates/revier-analysis/src/git/blob.rs`
- Modify: `crates/revier-analysis/src/git/diff.rs`
- Modify: `crates/revier-analysis/src/commands/query_files.rs`
- Modify: `crates/revier-analysis/src/attribution/patch_inference.rs`
- Modify: `crates/revier-analysis/src/attribution/blame.rs`
- Modify: `crates/revier-analysis/src/attribution/merge_trace.rs`
- Modify: `crates/revier-analysis/src/attribution/deletion_trace.rs`
- Modify: `crates/revier-analysis/src/json.rs`
- Modify: `crates/revier-analysis/src/overlay/file_overlay.rs`
- Modify: `crates/revier-analysis/tests/fixtures.rs`
- Modify: `crates/revier-analysis/tests/file_overlay_cli.rs`
- Modify: `crates/revier-analysis/tests/index_git_diff.rs`

- [ ] **Step 1: 编写解码失败测试**

`text_encoding.rs` 直接测试以下公开入口：

```rust
pub struct DecodedText {
    pub text: String,
    pub encoding: ResolvedTextEncoding,
}

pub fn decode_text_bytes(
    bytes: &[u8],
    requested: TextEncoding,
) -> Result<DecodedText, AppError>;
```

测试必须覆盖：UTF-8 BOM 去除、合法 UTF-8、GB18030 中文、UTF-16 LE/BE（有 BOM 与手动无 BOM）、奇数 UTF-16 字节、错误手动编码、`auto` 对无 BOM 非 UTF-8 返回错误、在 `auto`/UTF-8 下明显二进制 NUL 数据返回 `FileNotAnalyzable`。手动 UTF-16 中合法的交错 NUL 不得被二进制启发式提前拒绝。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
cargo test -p revier-analysis --test text_encoding
```

预期：模块或入口不存在导致编译失败。

- [ ] **Step 3: 实现无替换字符的严格解码**

实现顺序必须固定：

```text
auto: UTF-8 BOM -> UTF-16 LE BOM -> UTF-16 BE BOM -> 合法 UTF-8 -> 明确错误
manual: 按指定编码严格解码；任何 malformed sequence 都返回错误
```

GB18030 使用 `encoding_rs::GB18030.decode_without_bom_handling_and_without_replacement`；UTF-16 先检查偶数字节，再以指定端序构造 `u16` 并调用 `String::from_utf16`。不得使用 `from_utf8_lossy` 或带替换字符的解码。

严格解码还必须拒绝与请求/resolved encoding 冲突的 BOM：UTF-16 LE 不得接受 BE BOM，UTF-16 BE 不得接受 LE BOM，UTF-8/GB18030 不得把 UTF-16 BOM 当普通字符继续解码。历史两侧 BOM 冲突时 Overlay 返回明确错误。

把 `CommitFileChange.status` 改为始终保存真实的 `added|modified|deleted|renamed`，`is_binary` 单独保存启发式结果。只有 `query_files` 输出初始文件列表时才把 `is_binary=true` 映射为公开 `binary` 状态。UTF-16 BOM 不视为二进制；无法自动解码但没有 NUL 的文件在行数统计阶段保留 `0/0`，不得让整个 Review 分析失败，以便用户通过 `default_encoding` 再次加载。

同步更新 `tests/index_git_diff.rs` 的二进制夹具断言：内部 `status` 使用该夹具真实的 `added`，并继续断言 `is_binary=true`、`is_previewable=false`；不得削弱二进制检测覆盖。

- [ ] **Step 4: 让 file-overlay 接收编码并返回完整文本**

`FileOverlayArgs` 新增：

```rust
#[arg(long, default_value = "auto")]
pub encoding: String,
```

`commands/trace_block.rs` 和 `tests/fixtures.rs` 中现有的 `FileOverlayArgs` 构造器显式设置 `encoding: "auto".to_string()`；这只是调用方适配，不给 `trace-block` 增加新的 CLI 参数。

在 `build_file_overlay` 中解析为 `TextEncoding`。对同一历史文件使用一个解析后编码：优先以新侧 Blob（删除文件用旧侧）解析 `auto`，再用该 `ResolvedTextEncoding` 严格解码左右两侧；这样 `FileOverlay.resolvedEncoding` 始终有唯一含义。编码发生历史切换时要求用户手动选择，不偷偷让两侧使用不同编码。

`build_file_overlay` 不得仅因 `change.is_binary` 提前返回；先按请求编码尝试严格解码。解码成功时 Overlay 的 `file.status` 使用内部真实变更类型，`isBinary=false`、`isPreviewable=true`；解码仍呈现明显二进制或失败时才返回不可分析错误。这样 BOM UTF-16 和通过 TOML 指定的 BOM-less UTF-16 能进入编辑器，真实二进制仍不可预览。

同一 `ResolvedTextEncoding` 必须继续传入 `patch_inference`、`blame`、`merge_trace` 与 `deletion_trace` 的历史 Blob 读取路径；所有路径使用严格解码后的文本，GB18030/UTF-16 不得直接返回未归因的 `diff.blocks`。新增 GB18030 与 UTF-16 Overlay 测试必须断言作者、related commits 和 attribution，另加历史两侧冲突 BOM 的失败测试。

`FileOverlayOutput` 新增：

```rust
pub old_content: String,
pub new_content: String,
pub resolved_encoding: String,
```

必须直接保存解码字符串，不从 `rows` 重新拼接，以保留 CRLF、文件末尾换行和空文件。

- [ ] **Step 5: 扩展 CLI 集成测试**

在现有线性夹具断言：

```rust
assert_eq!(overlay["oldContent"], "one\n");
assert_eq!(overlay["newContent"], "one\ntwo\n");
assert_eq!(overlay["resolvedEncoding"], "utf-8");
```

再加入一个字节夹具，用 `--encoding gb18030` 成功输出中文完整文本，并断言 `--encoding auto` 失败且错误说明无法确定编码。

- [ ] **Step 6: 运行分析测试**

运行：

```powershell
cargo test -p revier-analysis --test text_encoding
cargo test -p revier-analysis --test file_overlay_cli
cargo test -p revier-analysis
```

预期：全部通过；现有二进制拒绝测试仍通过。

- [ ] **Step 7: 提交编码与完整文本**

运行：

```powershell
git add crates/revier-analysis/src/text_encoding.rs crates/revier-analysis/tests/text_encoding.rs crates/revier-analysis/src/lib.rs crates/revier-analysis/src/cli.rs crates/revier-analysis/src/commands/trace_block.rs crates/revier-analysis/src/git/blob.rs crates/revier-analysis/src/git/diff.rs crates/revier-analysis/src/commands/query_files.rs crates/revier-analysis/src/attribution/patch_inference.rs crates/revier-analysis/src/attribution/blame.rs crates/revier-analysis/src/attribution/merge_trace.rs crates/revier-analysis/src/attribution/deletion_trace.rs crates/revier-analysis/src/json.rs crates/revier-analysis/src/overlay/file_overlay.rs crates/revier-analysis/tests/fixtures.rs crates/revier-analysis/tests/file_overlay_cli.rs crates/revier-analysis/tests/index_git_diff.rs
git commit -m "feat(overlay): 按指定编码返回完整文件文本"
```

---

### Task 5: 在 Tauri ReviewService 传递编码并聚合作者统计

> **执行作用：** 本任务除实现既定编码与作者统计外，还负责完整适配 Task 2 的必需契约字段，使 `revier-tauri` 恢复可编译；完成并通过双重审查后再回到 Task 3。

**Files:**
- Modify: `src-tauri/src/services/review.rs`

- [ ] **Step 1: 先写 service 单元测试**

在 `review.rs` 现有测试模块新增三个具名纯适配测试：

- `aggregates_authors_by_normalized_email_and_sorts_source_data`：构造同邮箱不同大小写的三条 commit，断言合并、去重 hash、次数和最新时间。
- `falls_back_to_normalized_name_without_email`：构造无邮箱且姓名大小写不同的数据，断言按姓名合并。
- `keeps_full_contents_and_resolved_encoding_when_adapting_overlay`：输入含 CRLF 与末尾换行的左右文本，断言逐字节透传和 resolved encoding。

作者预期：`commit_count` 统计去重 commit hash，`last_committed_at` 取最大 RFC3339 时间。没有匹配 related commit 的作者保留 `commitCount = 0`、`lastCommittedAt = ""`，前端显示“未知”，不得阻断 Diff。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
cargo test -p revier-tauri services::review
```

预期：缺少新增字段或断言不满足而失败。

- [ ] **Step 3: 传递 range/commit overlay 编码**

- `get_file_overlay` 把 `request.encoding.unwrap_or(TextEncoding::Auto)` 转为 CLI 字符串。
- `get_commit_overlay` 使用同一个解码入口读取父提交/当前提交 Blob。
- 两个入口都返回 `old_content`、`new_content`、`resolved_encoding`。
- commit overlay 的唯一作者必须是 `commitCount = 1`、`lastCommittedAt = commit.committed_at`。

- [ ] **Step 4: 实现 Rust 作者聚合**

固定身份键：非空邮箱 `trim().to_lowercase()` 优先，否则姓名 `trim().to_lowercase()`。输出名称/邮箱采用最后提交时间最新的 related commit，统计完成后不在前端重新合并。

将适配签名改成：

```rust
fn adapt_authors(
    authors: Vec<AuthorOutput>,
    commits: &[RelatedCommitOutput],
) -> Vec<AuthorSummary>;
```

`adapt_block` 必须在移动 `related_commits` 前完成聚合。

- [ ] **Step 5: 验证 Tauri 与 workspace**

运行：

```powershell
cargo test -p revier-tauri
cargo test --workspace
```

预期：全部通过。

- [ ] **Step 6: 提交 Review 适配**

运行：

```powershell
git add src-tauri/src/services/review.rs
git commit -m "feat(review): 传递编码并聚合块作者统计"
```

---

### Task 6: 在前端启动阶段加载设置并统一浅深主题

**Files:**
- Create: `src/renderer/editor/editorTheme.ts`
- Create: `src/renderer/composables/useEditorSettings.ts`
- Create: `tests/unit/editorTheme.test.ts`
- Modify: `src/renderer/api/revierClient.ts`
- Modify: `tests/unit/revierClient.test.ts`
- Modify: `src/renderer/main.ts`
- Modify: `src/renderer/App.vue`

- [ ] **Step 1: 编写主题转换失败测试**

`editorTheme.test.ts` 锁定：

```ts
expect(toFontFamily(['JetBrainsMono Nerd Font Mono', 'Microsoft YaHei', 'monospace']))
  .toBe('"JetBrainsMono Nerd Font Mono", "Microsoft YaHei", monospace');
expect(resolveThemeMode('system', true)).toBe('dark');
expect(resolveThemeMode('light', true)).toBe('light');
expect(toCssVariables(settings.themes.dark)['--workspace-background']).toBe('#111821');
```

同时断言 Monaco、Shiki 和 Naive UI 的背景、前景、边框、accent、diff 红绿都来自同一个 `EditorThemeColors`。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/editorTheme.test.ts
```

预期：模块不存在导致失败。

- [ ] **Step 3: 实现纯主题转换和设置单例**

`editorTheme.ts` 只导出纯函数：

```ts
export function toFontFamily(families: string[]): string;
export function resolveThemeMode(mode: EditorThemeMode, systemDark: boolean): 'light' | 'dark';
export function toCssVariables(theme: EditorThemeColors): Record<string, string>;
export function toNaiveThemeOverrides(theme: EditorThemeColors, fontFamily: string): GlobalThemeOverrides;
export function toMonacoTheme(theme: EditorThemeColors): editor.IStandaloneThemeData;
export function toShikiTheme(name: string, theme: EditorThemeColors): ThemeRegistration;
```

`useEditorSettings.ts` 使用模块级只读 refs，公开：

```ts
export async function initializeEditorSettings(): Promise<void>;
export function useEditorSettings(): {
  snapshot: Readonly<Ref<EditorSettingsSnapshot>>;
  effectiveTheme: Readonly<Ref<'light' | 'dark'>>;
  activeColors: Readonly<Ref<EditorThemeColors>>;
  fontFamily: Readonly<Ref<string>>;
};
```

`matchMedia('(prefers-color-scheme: dark)')` 只在 `theme === 'system'` 时影响 effective theme；切换只应用 CSS/主题，不重新读取 TOML。

- [ ] **Step 4: 接入 Tauri 客户端与挂载顺序**

`revierClient` 增加：

```ts
settings: {
  getEditorSettings: () => invoke<EditorSettingsSnapshot>('editor_settings_get')
}
```

`main.ts` 必须在 `createApp(...).mount('#app')` 前 `await initializeEditorSettings()`。IPC 失败使用与 Rust 相同的内置默认快照并携带 warning，不能让白屏阻断启动。

`App.vue` 给 `n-config-provider` 同时传入 `darkTheme` 或 `null` 和动态 `themeOverrides`；根节点设置 `data-theme="light|dark"`。`snapshot.warning` 通过应用顶层非阻断 warning banner 显示配置路径/错误，可关闭但不写回配置文件。

- [ ] **Step 5: 更新客户端测试并运行**

在 `revierClient.test.ts` 断言无参数调用：

```ts
await revierClient.settings.getEditorSettings();
expect(invoke).toHaveBeenCalledWith('editor_settings_get');
```

运行：

```powershell
pnpm test -- tests/unit/editorTheme.test.ts tests/unit/revierClient.test.ts
pnpm typecheck
```

预期：全部通过。

- [ ] **Step 6: 提交启动设置与主题基础**

运行：

```powershell
git add src/renderer/editor/editorTheme.ts src/renderer/composables/useEditorSettings.ts tests/unit/editorTheme.test.ts src/renderer/api/revierClient.ts tests/unit/revierClient.test.ts src/renderer/main.ts src/renderer/App.vue
git commit -m "feat(theme): 启动时应用统一编辑器主题"
```

---

### Task 7: 建立语言注册表、Shiki 高亮和 Monaco Worker

**Files:**
- Create: `src/renderer/editor/editorLanguages.ts`
- Create: `src/renderer/editor/monacoEnvironment.ts`
- Create: `tests/unit/editorLanguages.test.ts`
- Modify: `src/renderer/vite-env.d.ts`
- Modify: `src/renderer/main.ts`

- [ ] **Step 1: 编写语言映射失败测试**

测试至少包含：

```ts
expect(detectEditorLanguage('src/App.vue')).toBe('vue');
expect(detectEditorLanguage('Cargo.toml')).toBe('toml');
expect(detectEditorLanguage('Makefile')).toBe('shell');
expect(detectEditorLanguage('scripts/build.ps1')).toBe('powershell');
expect(detectEditorLanguage('unknown.data')).toBe('plaintext');
expect(EDITOR_LANGUAGES.map((item) => item.id)).toEqual([
  'plaintext', 'javascript', 'typescript', 'vue', 'html', 'css', 'rust',
  'python', 'go', 'java', 'c', 'cpp', 'csharp', 'sql', 'markdown', 'json',
  'yaml', 'toml', 'shell', 'powershell'
]);
```

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/editorLanguages.test.ts
```

预期：注册表模块不存在。

- [ ] **Step 3: 实现唯一语言注册表**

每项结构固定为：

```ts
export interface EditorLanguageDefinition {
  id: string;
  label: string;
  extensions: string[];
  filenames?: string[];
  shikiLanguage?: string;
}
```

匹配优先级：完整文件名（不区分大小写）→ 最长扩展名 → `plaintext`。Vue 使用 Shiki `vue`，C/C++ 分别使用 `c`/`cpp`，Shell 包含 `.sh`/`.bash`，PowerShell 包含 `.ps1`/`.psm1`。

- [ ] **Step 4: 配置 Worker 和一次性 Shiki 初始化**

`monacoEnvironment.ts` 使用 Vite Worker 导入：editor、typescript、json、css、html；`getWorker` 按 label 分派，其他语言返回 editor worker。

公开初始化入口：

```ts
export async function initializeMonacoSyntax(
  themes: { light: EditorThemeColors; dark: EditorThemeColors }
): Promise<void>;
```

它必须：

1. 只向 Shiki 注册并加载 `EDITOR_LANGUAGES` 中的语言 ID；Monaco 自身可能随发行包携带其他基础 language contribution，但不把它们暴露到产品选择列表。
2. 用确认语言列表创建一个可复用 highlighter。
3. 用 `toShikiTheme` 生成 `revier-light` / `revier-dark`。
4. 调用 `shikiToMonaco` 一次。
5. 单语言加载失败时记录非阻断 warning，并让该 ID 使用 plaintext；不得影响其他语言。

`main.ts` 的启动顺序固定为：`initializeEditorSettings()` → `initializeMonacoSyntax(snapshot.settings.themes)` → mount。整个 Shiki 初始化失败时仍挂载 Monaco 基础编辑器，并把 warning 加入设置 warning 区；不得回退旧 Diff。

- [ ] **Step 5: 运行测试与构建检查**

运行：

```powershell
pnpm test -- tests/unit/editorLanguages.test.ts
pnpm typecheck
pnpm vite:build
```

预期：测试、类型检查和 Worker bundle 构建通过，不出现 `MonacoEnvironment.getWorker` 缺失警告。

- [ ] **Step 6: 提交语言运行时**

运行：

```powershell
git add src/renderer/editor/editorLanguages.ts src/renderer/editor/monacoEnvironment.ts tests/unit/editorLanguages.test.ts src/renderer/vite-env.d.ts src/renderer/main.ts
git commit -m "feat(editor): 配置 Monaco 语言高亮与 Worker"
```

---

### Task 8: 用可测试会话管理 Monaco Diff Editor 生命周期

**Files:**
- Create: `src/renderer/editor/monacoDiffSession.ts`
- Create: `tests/unit/monacoDiffSession.test.ts`
- Create: `src/renderer/components/review/MonacoDiffSurface.vue`
- Create: `tests/unit/monacoDiffSurface.test.ts`

- [ ] **Step 1: 编写会话生命周期失败测试**

用 Vitest mock `monaco.editor.createModel`、`createDiffEditor` 和 Disposable，验证：

- 创建两个带原文件扩展名、但 URI 不冲突的内存 Model。
- `createDiffEditor` 收到 `originalEditable: true`、`readOnly: false`、`renderSideBySide: true`、配置字体/行高/Minimap。
- 创建参数显式启用 `folding`、`matchBrackets: 'always'`、完整 Monaco contribution 和 `largeFileOptimizations: true`，以提供查找与命令面板。
- 两个 Model 任一首次内容变化只触发一次 `onDraftChange(true)`。
- `setLanguage` 同时调用两次 `setModelLanguage`，不重建 Model。
- `setTheme` 不重建 Model、不清空 undo stack。
- `dispose` 恰好释放 Diff Editor、两个 Model、Decoration collection 和全部监听器一次。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/monacoDiffSession.test.ts
```

预期：会话模块不存在。

- [ ] **Step 3: 实现会话接口**

固定公开接口：

```ts
export interface MonacoDiffSession {
  diffEditor: editor.IStandaloneDiffEditor;
  originalEditor: editor.ICodeEditor;
  modifiedEditor: editor.ICodeEditor;
  setLanguage(languageId: string): void;
  setTheme(themeName: 'revier-light' | 'revier-dark'): void;
  setSelectedBlock(block?: DiffBlock): void;
  layout(): void;
  dispose(): void;
}

export function createMonacoDiffSession(options: {
  container: HTMLElement;
  path: string;
  oldContent: string;
  newContent: string;
  languageId: string;
  settings: EditorSettings;
  themeName: 'revier-light' | 'revier-dark';
  blocks: DiffBlock[];
  onDraftChange(value: true): void;
  onBlockSelected(block: DiffBlock): void;
  onCursorChange(line: number, column: number): void;
  onEditorsReady(original: editor.ICodeEditor, modified: editor.ICodeEditor): void;
}): MonacoDiffSession;
```

块命中只在原始态启用：原始侧按 `oldStart..oldEnd`，修改侧按 `newStart..newEnd`；0 行范围不得命中。选中 Decoration 仅使用公开 `createDecorationsCollection`，为旧侧/新侧分别加外缘 class，teal 只加块首 top 与块尾 bottom class。

- [ ] **Step 4: 编写 Vue 外壳失败测试**

`monacoDiffSurface.test.ts` mock `createMonacoDiffSession`，断言：

- mount 创建一次。
- overlay 内容/上下文 key 变化先 dispose 旧会话再创建新会话。
- language/theme 变化调用 setter，不重建。
- unmount 调用 dispose。
- 初始化异常显示“编辑器加载失败”和“重试加载”。

- [ ] **Step 5: 实现 MonacoDiffSurface**

组件只负责 Vue 生命周期与错误 UI，通过 emit 转发：

```ts
const emit = defineEmits<{
  draftChange: [value: true];
  selected: [block: DiffBlock];
  cursorChange: [line: number, column: number];
  editorsReady: [original: editor.ICodeEditor, modified: editor.ICodeEditor];
}>();
```

使用 `ResizeObserver` 调用 `session.layout()`；重试必须先清理半初始化资源。

- [ ] **Step 6: 运行适配层测试**

运行：

```powershell
pnpm test -- tests/unit/monacoDiffSession.test.ts tests/unit/monacoDiffSurface.test.ts
pnpm typecheck
```

预期：全部通过。

- [ ] **Step 7: 提交 Monaco 会话**

运行：

```powershell
git add src/renderer/editor/monacoDiffSession.ts tests/unit/monacoDiffSession.test.ts src/renderer/components/review/MonacoDiffSurface.vue tests/unit/monacoDiffSurface.test.ts
git commit -m "feat(editor): 管理可编辑 Monaco Diff 会话"
```

---

### Task 9: 实现真实块几何、作者排序与外置 AuthorRail

**Files:**
- Create: `src/renderer/editor/diffBlockGeometry.ts`
- Create: `tests/unit/diffBlockGeometry.test.ts`
- Create: `src/renderer/components/review/DiffAuthorRail.vue`
- Create: `tests/unit/diffAuthorRail.test.ts`

- [ ] **Step 1: 编写纯函数失败测试**

锁定作者排序和裁剪：

```ts
expect(sortBlockAuthors(authors).map((author) => author.name))
  .toEqual(['高频且较新', '高频但较旧', '低频']);
expect(fitAuthors(20, authors)).toEqual({ visible: [], hasMore: true });
expect(fitAuthors(42, authors)).toEqual({ visible: [authors[0]], hasMore: true });
expect(fitAuthors(64, authors)).toEqual({ visible: authors.slice(0, 2), hasMore: true });
```

几何测试覆盖：修改/新增用 modified editor、删除用 original editor、部分出视口裁剪、完全出视口隐藏、折叠后无可见行隐藏、容器 height 不超过块可见高度。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/diffBlockGeometry.test.ts
```

预期：模块不存在。

- [ ] **Step 3: 实现确定性纯函数**

固定常量：

```ts
export const AUTHOR_RAIL_WIDTH = 112;
export const AUTHOR_ROW_HEIGHT = 20;
export const AUTHOR_ROW_GAP = 2;
export const AUTHOR_POPOVER_MAX_HEIGHT = 320;
```

排序：`commitCount desc -> Date.parse(lastCommittedAt) desc -> name localeCompare('zh-CN')`；无效/空时间按 0 处理。

容量：`floor((height + gap) / (rowHeight + gap))`。有隐藏作者时保留一个 `…` 槽；容量为 1 时只显示 `…`。

几何只使用公开接口提供的数据：`getVisibleRanges()`、`getTopForLineNumber()`、`getScrolledVisiblePosition()`、`getScrollTop()`、lineHeight 和 layoutHeight。

- [ ] **Step 4: 编写 AuthorRail 组件失败测试**

测试 mock 两侧 editor，断言：

- root 宽 112px。
- 作者容器 `top`/`height` 等于纯函数结果。
- 点击容器 emit `selected(block)`。
- 点击 `…` 使用 `stopPropagation`，只打开向左 Popover。
- Popover 显示姓名、邮箱、提交次数、最后提交时间，max-height 320px。
- `draft=true` 时完全不渲染 rail。
- 多次 scroll/layout/hidden-area 事件在同一帧只计算一次。

- [ ] **Step 5: 实现 rAF 合并和公开事件订阅**

监听修改/原始 editor 的：

```ts
onDidScrollChange
onDidLayoutChange
onDidChangeHiddenAreas
```

每个事件只调用 `scheduleLayout()`；`requestAnimationFrame` 中统一重算可见块。unmount 时取消未执行帧并 dispose 全部监听器。

- [ ] **Step 6: 运行轨道测试**

运行：

```powershell
pnpm test -- tests/unit/diffBlockGeometry.test.ts tests/unit/diffAuthorRail.test.ts
pnpm typecheck
```

预期：全部通过。

- [ ] **Step 7: 提交 AuthorRail**

运行：

```powershell
git add src/renderer/editor/diffBlockGeometry.ts tests/unit/diffBlockGeometry.test.ts src/renderer/components/review/DiffAuthorRail.vue tests/unit/diffAuthorRail.test.ts
git commit -m "feat(editor): 在 Minimap 外侧显示块作者轨道"
```

---

### Task 10: 实现状态栏语言/编码上拉与大文件三态 DiffViewer

**Files:**
- Create: `src/renderer/components/review/EditorStatusBar.vue`
- Create: `tests/unit/editorStatusBar.test.ts`
- Modify: `src/renderer/components/review/DiffViewer.vue`
- Modify: `tests/unit/diffViewer.test.ts`
- Modify: `tests/unit/diffViewerSelectionStyles.test.ts`
- Delete: `src/renderer/components/review/DiffBlockAuthors.vue`

- [ ] **Step 1: 先写状态栏失败测试**

断言：

- 原始态显示“原始 Diff”，草稿态显示“临时草稿”。
- 显示 `Ln 12, Col 8`、实际 `resolvedEncoding` 和语言 label。
- 点击编码/语言按钮后菜单 DOM 位于状态栏内，class 含 `editor-status-menu--upward`。
- 二进制状态不显示编码按钮。
- 编码选择 emit `encodingChange(TextEncoding)`；语言选择 emit `languageChange(id)`。

- [ ] **Step 2: 运行状态栏测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/editorStatusBar.test.ts
```

预期：组件不存在。

- [ ] **Step 3: 实现 EditorStatusBar**

菜单必须 `position: absolute; bottom: calc(100% + 4px)`，编码选项固定为 `auto`、`utf-8`、`gb18030`、`utf-16le`、`utf-16be`；状态栏显示的是 `resolvedEncoding`，选单勾选的是当前请求编码。

- [ ] **Step 4: 把 DiffViewer 测试改为组件组合契约**

移除对旧 `.diff-row` DOM 的依赖，mock `MonacoDiffSurface`、`DiffAuthorRail`、`EditorStatusBar`，新增断言：

- 顶部只显示文件路径、范围和块数，不显示文件类型/Minimap 按钮。
- 正常小文件创建 Monaco。
- 超过 `maxBytes` 或 `maxLines` 进入 `deferred`，确认前不 mount Monaco。
- 点击“仍然加载”进入 original；点击取消保留路径/大小/行数。
- surface 首次 `draftChange` 后显示恢复按钮、隐藏 AuthorRail、清除真实块选中并 emit `draftChange(true)`。
- 点击恢复通过改变 session key 销毁并重建，emit `draftChange(false)`。
- 手动语言只影响当前 overlay；文件路径变化恢复自动识别。

大文件字节数使用 `TextEncoder().encode(content).byteLength`，行数定义为：空文本 0 行，非空文本为换行数 + 1；比较左右较大值。

- [ ] **Step 5: 实现 DiffViewer 三态容器**

内部状态严格为：

```ts
type DiffViewerMode = 'deferred' | 'original' | 'draft';
```

结构顺序固定：header → Monaco/错误/确认主体 → bottom statusbar。主体布局为 `MonacoDiffSurface` 与 `DiffAuthorRail` 兄弟节点；draft 时 rail 不渲染且 Monaco 通过 ResizeObserver 占满释放宽度。

- [ ] **Step 6: 删除代码区作者组件和旧样式契约测试**

删除 `DiffBlockAuthors.vue`。将 `diffViewerSelectionStyles.test.ts` 改为读取新 Decoration class 的 CSS，断言旧侧深红外缘、新侧深绿外缘、首行 teal top、尾行 teal bottom，且中间 class 没有横线规则。

- [ ] **Step 7: 运行 DiffViewer 测试**

运行：

```powershell
pnpm test -- tests/unit/editorStatusBar.test.ts tests/unit/diffViewer.test.ts tests/unit/diffViewerSelectionStyles.test.ts
pnpm typecheck
```

预期：全部通过。

- [ ] **Step 8: 提交 Viewer 状态机**

运行：

```powershell
git add src/renderer/components/review/EditorStatusBar.vue tests/unit/editorStatusBar.test.ts src/renderer/components/review/DiffViewer.vue tests/unit/diffViewer.test.ts tests/unit/diffViewerSelectionStyles.test.ts src/renderer/components/review/DiffBlockAuthors.vue
git commit -m "feat(editor): 组合草稿状态与交互状态栏"
```

---

### Task 11: 原子重载编码并接入工作台、下钻页和详情栏

**Files:**
- Modify: `src/renderer/stores/reviewStore.ts`
- Modify: `tests/unit/rendererReviewStore.test.ts`
- Modify: `src/renderer/components/review/DiffDrilldownOverlay.vue`
- Modify: `tests/unit/diffDrilldownOverlay.test.ts`
- Modify: `src/renderer/components/review/BlockDetailPanel.vue`
- Modify: `tests/unit/blockDetailPanel.test.ts`
- Modify: `src/renderer/pages/ReviewWorkspace.vue`
- Modify: `tests/unit/reviewWorkspace.test.ts`

- [ ] **Step 1: 先写 Store 原子替换失败测试**

新增断言：

- `loadOverlay(path, encoding)` 把 encoding 传给 Tauri，并在文件上下文切换时立即清除旧 overlay。
- `reloadOverlayEncoding(path, encoding)` 在成功前保留旧 overlay，成功后一次性替换。
- 编码重载失败保留旧 overlay 与当前草稿组件，设置 error。
- commit overlay 使用对应的初次加载/编码重载两个入口。
- 后到达的旧请求不得覆盖新请求。

- [ ] **Step 2: 运行 Store 测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/rendererReviewStore.test.ts
```

预期：方法签名或原子替换断言失败。

- [ ] **Step 3: 实现请求成功后替换**

固定方法：

```ts
async loadOverlay(filePath: string, encoding?: TextEncoding): Promise<boolean>;
async reloadOverlayEncoding(filePath: string, encoding: TextEncoding): Promise<boolean>;
async loadCommitOverlay(filePath: string, commitHash: string, encoding?: TextEncoding): Promise<boolean>;
async reloadCommitOverlayEncoding(
  filePath: string,
  commitHash: string,
  encoding: TextEncoding
): Promise<boolean>;
```

返回值只表示是否成功应用。`loadOverlay` / `loadCommitOverlay` 用于上下文切换，先递增 request id、清除旧 overlay，由页面 key/unmount 立即销毁旧编辑器；两个 `reload*Encoding` 用于同一上下文，不清除当前 overlay，只有成功响应才能原子替换。编码失败不得销毁当前编辑器。

`ReviewWorkspace` 首次加载文件和提交时必须显式传入启动快照的 `settings.defaultEncoding`，不能在前端另写一个 `auto` 默认值；状态栏把页面保存的成功请求值作为当前勾选项。用户选择新编码后，页面只在 `reload*Encoding` 返回 true 时更新该请求值；切换文件后重新使用 TOML 默认编码。

- [ ] **Step 4: 接入草稿与右栏语义**

`BlockDetailPanel` 新增 `draft?: boolean`：draft 时只显示“临时草稿不提供归因”，不渲染作者/提交列表。

`ReviewWorkspace` 新增本地 `isEditorDraft`：

- 收到 `draftChange(true)` 立即 `reviewStore.selectBlock(undefined)`。
- 恢复或新 overlay 生效设 false。
- 切换文件、重新分析、打开其他提交、关闭提交、离开页面不询问，依赖 key/unmount 直接 dispose。
- 点击其他文件/提交不加草稿锁。

`DiffDrilldownOverlay` 转发 `draft-change`、`encoding-change`，并使用 commit hash + parent hash 作为 editor context key。

- [ ] **Step 5: 更新页面交互测试**

测试明确模拟：

1. 原始态点击块，右栏收到真实 block。
2. surface 发出 draft 后右栏收到 `draft=true` 且 block 清空。
3. draft 中点击其他文件，调用新 overlay 并销毁当前 viewer，无确认弹窗。
4. 编码成功后 viewer 接收新 overlay；失败时仍接收旧 overlay。
5. 左栏 `FilterPanel`、分支、作者、时间、message、glob 和文件列表仍存在，不因布局替换丢失。

- [ ] **Step 6: 运行集成单元测试**

运行：

```powershell
pnpm test -- tests/unit/rendererReviewStore.test.ts tests/unit/diffDrilldownOverlay.test.ts tests/unit/blockDetailPanel.test.ts tests/unit/reviewWorkspace.test.ts
pnpm typecheck
```

预期：全部通过。

- [ ] **Step 7: 提交工作台接入**

运行：

```powershell
git add src/renderer/stores/reviewStore.ts tests/unit/rendererReviewStore.test.ts src/renderer/components/review/DiffDrilldownOverlay.vue tests/unit/diffDrilldownOverlay.test.ts src/renderer/components/review/BlockDetailPanel.vue tests/unit/blockDetailPanel.test.ts src/renderer/pages/ReviewWorkspace.vue tests/unit/reviewWorkspace.test.ts
git commit -m "feat(review): 接入草稿销毁与编码重载"
```

---

### Task 12: 落地完整 IDE 视觉与统一浅深色

**Files:**
- Modify: `src/renderer/styles.css`
- Modify: `src/renderer/App.vue`
- Modify: `tests/unit/reviewDiffPaneLayout.test.ts`
- Modify: `tests/unit/diffViewerSelectionStyles.test.ts`

- [ ] **Step 1: 先写样式契约失败测试**

以现有源码样式测试方式锁定：

- `.review-workspace`、左栏、编辑器、AuthorRail、右栏、弹层、状态栏全部使用 CSS 变量，不再硬编码只适合浅色的面板背景。
- `.diff-viewer__editor-layout` 为 `minmax(0, 1fr) 112px`。
- draft 时为单列 `minmax(0, 1fr)`。
- 状态菜单向上，AuthorRail Popover 向左。
- 旧/新选中块外缘和 teal 首尾线完整。
- 不存在旧 `.diff-table--full`、`.diff-row__authors` 的运行时样式。

- [ ] **Step 2: 运行测试确认失败**

运行：

```powershell
pnpm test -- tests/unit/reviewDiffPaneLayout.test.ts tests/unit/diffViewerSelectionStyles.test.ts
```

预期：缺少新布局/主题规则而失败。

- [ ] **Step 3: 按最终视觉稿替换样式**

根变量名固定为：

```css
--workspace-background
--panel-background
--editor-background
--border-color
--foreground-color
--muted-color
--accent-color
--selection-color
--diff-removed
--diff-removed-strong
--diff-removed-word
--diff-added
--diff-added-strong
--diff-added-word
--editor-font-family
--editor-font-size
--editor-line-height
```

关键尺寸：AuthorRail 112px、作者行 20px、间距 2px、Popover max-height 320px。窄窗口保持 AuthorRail 宽度，让 Monaco 自身横向滚动；不得压缩作者名。

样式必须覆盖项目页和 Review 页，保证整应用同一时刻只有完整 light 或完整 dark，不出现浅色外壳 + 深色编辑器。

- [ ] **Step 4: 运行全部前端验证**

运行：

```powershell
pnpm test
pnpm typecheck
pnpm lint
pnpm vite:build
```

预期：全部通过。

- [ ] **Step 5: 提交视觉实现**

运行：

```powershell
git add src/renderer/styles.css src/renderer/App.vue tests/unit/reviewDiffPaneLayout.test.ts tests/unit/diffViewerSelectionStyles.test.ts
git commit -m "style(editor): 应用统一 IDE 浅深色视觉"
```

---

### Task 13: 全链路回归、Tauri 视觉验收和文档对照

**Files:**
- Modify only if a confirmed defect is found; otherwise no source changes

- [ ] **Step 1: 运行最终自动化验证**

运行：

```powershell
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm test
pnpm typecheck
pnpm lint
pnpm generate:bindings:check
pnpm vite:build
git status --short
```

预期：所有命令通过；status 只包含执行前记录的无关用户文件。

- [ ] **Step 2: 启动 Tauri 做浏览器级验收**

运行：

```powershell
pnpm tauri dev
```

逐项验证：

- 左栏分支、时间、作者、提交信息、glob 和文件列表完整保留。
- 顶部只显示路径/范围/块数；无文件类型和 Minimap 顶部按钮。
- 左右侧都可放光标、选择、编辑、undo/redo、查找、折叠、括号匹配、命令面板。
- Minimap 位于 Monaco 内部最右侧，AuthorRail 在其外侧且不遮挡代码。
- 原始态块可选；旧侧深红、新侧深绿、teal 只在首尾；无多余行间线。
- AuthorRail 高度不超过块；排序、裁剪、`…`、向左 Popover 正确。
- 首次编辑后 rail/真实块/归因消失，右栏显示临时草稿提示。
- 切文件、切提交、关闭下钻和离开页面直接销毁草稿，无保存提示。
- 状态栏语言/编码菜单原位上拉；语言立即作用于两侧，编码成功后重建，失败保留当前内容。
- light/dark/system 覆盖整个应用；系统主题变化不清空草稿。
- 字体计算值按 `JetBrainsMono Nerd Font Mono` → `Microsoft YaHei` → `monospace` 回落。
- 大于 1 MiB 或 5000 行先确认，确认前无 Monaco Model。

- [ ] **Step 3: 验证配置文件行为**

在测试用 Tauri 数据目录验证：首次启动创建完整 `editor.toml`；修改主题后重启生效；运行中修改文件不热更新；损坏 TOML 后原文件保留、应用用默认值并显示 warning 路径。

- [ ] **Step 4: 对照设计文档复核范围**

逐条核对设计文档“验收标准”“异常处理”“生命周期”“非目标”。不得顺手增加保存、设置页或旧 Diff 回退。

- [ ] **Step 5: 提交仅由验收发现且已获确认的修复**

若无修复，不创建空提交。若有修复，先获得用户对关键变更的书面确认，再按范围提交：

```powershell
git add <已确认的修复文件>
git commit -m "fix(editor): 修复 Monaco 验收问题"
```

---

## 实施完成定义

- [ ] 所有自动化命令通过。
- [ ] 所有视觉/交互验收项通过。
- [ ] `editor.toml` 默认值与设计文档完全一致。
- [ ] TypeScript bindings 可重复生成且无差异。
- [ ] 不存在草稿持久化路径。
- [ ] 不存在 Monaco 私有 DOM/API 访问。
- [ ] 不存在未释放的 Editor、Model、Decoration、监听器或 rAF。
- [ ] 不存在旧自绘 Diff 运行时回退或代码区作者标签。
- [ ] Git 历史由小步语义化提交组成，未包含 `.idea/` 等用户文件。
