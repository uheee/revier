# Revier 成熟第三方库接入一期实施计划

## 1. 实施依据

本计划对应：

`docs/superpowers/specs/2026-07-28-revier-third-party-integration-design.md`

本地待办清单：

`docs/third-party-integration-backlog.md`

实施遵循：

- 设计文档和本计划获得审核后才开始业务代码修改。
- 每个阶段使用 TDD：先新增失败测试，再实现最小代码，再运行定向验证。
- 不擅自修改既有测试业务语义；如必须修改，先提交原断言、拟修改断言和原因供确认。
- 每个关键节点完成后提交成果和验证结果，获得确认再进入下一节点。
- 只接入已确认的库、功能和代码位置，不顺带扩大范围。
- Shell 使用 PowerShell。
- Node 通过 `fnm` 使用 `.node-version`，包管理器使用 `pnpm`。
- Python 如有需要使用 `uv`。
- 所有文档、回答和代码注释使用中文。
- 未经明确要求，不执行 Git 提交、推送或发布。
- 如后续获准提交，提交消息使用语义化提交格式。
- `docs/third-party-integration-backlog.md` 不加入 Git 暂存区，不包含在任何提交中。

## 2. 预计改动范围

### 2.1 根依赖与配置

- `package.json`
- `pnpm-lock.yaml`
- `Cargo.lock`
- 可能调整根 `Cargo.toml`
- `crates/revier-analysis/Cargo.toml`
- `src-tauri/Cargo.toml`

### 2.2 Rust 分析库

- `crates/revier-analysis/src/error.rs`
- `crates/revier-analysis/src/index/connection.rs`
- `crates/revier-analysis/src/index/schema.rs`
- `crates/revier-analysis/src/index/migrations.rs`
- `crates/revier-analysis/src/index/queries.rs`
- `crates/revier-analysis/src/index/writer.rs`
- `crates/revier-analysis/src/cache/repository.rs`
- `crates/revier-analysis/src/git/*`
- `crates/revier-analysis/src/overlay/line_diff.rs`
- `crates/revier-analysis/src/overlay/diff_builder.rs`
- 新增 `crates/revier-analysis/benches/line_diff.rs`
- 新增或扩展 `crates/revier-analysis/tests/*`

错误阶段只修改确有字符串化问题的文件，不机械修改所有 `map_err`。

### 2.3 Tauri

- `src-tauri/src/lib.rs`
- `src-tauri/src/error.rs`
- `src-tauri/src/services/projects.rs`
- `src-tauri/src/services/review.rs`
- `src-tauri/src/commands/review.rs`
- 对应 Rust 单元测试

### 2.4 Vue

- `src/renderer/main.ts`
- `src/renderer/api/errors.ts`
- `src/renderer/api/revierClient.ts`
- `src/renderer/composables/useReviewLayoutSizes.ts`
- `src/renderer/composables/useEditorSettings.ts`
- `src/renderer/stores/projectStore.ts`
- `src/renderer/stores/reviewStore.ts`
- `src/renderer/pages/ProjectHome.vue`
- `src/renderer/pages/ReviewWorkspace.vue`
- `src/renderer/components/review/EditorStatusBar.vue`
- `src/renderer/components/review/OperationStatusBar.vue`
- `src/renderer/components/review/ReviewLayoutResizer.vue`
- `src/renderer/components/review/DiffAuthorRail.vue`
- `src/renderer/editor/monacoDiffSession.ts`
- 可能新增 `src/renderer/queries/*`
- 对应 `tests/unit/*`

### 2.5 文档

- 本设计文档与实施计划
- 完成后新增 `docs/superpowers/verification/*`
- 本地更新 `docs/third-party-integration-backlog.md`，但不提交
- 若最终改变用户可见配置或调试方式，经确认后更新 `README.md`

## 3. 阶段零：基线与决策确认

### 3.1 记录工作区状态

只读执行：

```powershell
git status --short
git diff --stat
```

要求：

- 记录实施前已有未跟踪和已修改文件。
- 不覆盖或整理用户已有改动。
- 单独确认 `docs/third-party-integration-backlog.md` 未被跟踪。

### 3.2 运行现有基线

按项目现有脚本执行：

```powershell
cargo test --workspace
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD test
fnm exec --using-file pnpm.CMD lint
```

若环境中的 `fnm` 调用形式与现有配置不兼容，先报告实际错误，不擅自换用 npm 或其他 Node 版本。

记录：

- 工具版本；
- 命令退出状态；
- 已有失败；
- 测试数量和耗时；
- 与本期相关的关键测试文件。

### 3.3 日志配置确认

提交以下选项供用户确认：

- 调试构建默认日志级别；
- 发布构建默认日志级别；
- stdout、stderr、文件和 WebView 目标；
- 单文件轮转大小；
- 保留文件数量；
- 路径、作者和提交字段的脱敏规则；
- 日志初始化失败时的回退行为。

未确认前不得增加日志插件依赖。

### 3.4 错误边界确认

完成字符串化错误清单，按以下类别提交：

- 可直接作为 `#[source]` 保存；
- 需要新增模块级错误变体；
- 只能保留格式化消息；
- 适合在 Tauri/CLI 边界附加上下文。

同时提交是否需要 `anyhow` 的建议。未确认前不添加 `anyhow`。

### 3.5 VueUse 与 Pinia Colada 确认

提交：

- 当前 VueUse 版本支持的目标 API；
- 是否需要先升级 VueUse；
- Pinia Colada 使用独立查询 composable 或 Store 包装；
- 普通查询的缓存、失效、重试和后台刷新规则。

完成以上确认后进入阶段一。

## 4. 阶段一：统一日志链路

### 4.1 依赖变更

获得日志配置确认后，计划使用：

```powershell
cargo add -p revier-tauri tauri-plugin-log
cargo add -p revier-tauri log
fnm exec --using-file pnpm.CMD add @tauri-apps/plugin-log
```

执行后检查：

```powershell
cargo tree -p revier-tauri
fnm exec --using-file pnpm.CMD list @tauri-apps/plugin-log
```

不得同时引入 `tracing`，除非用户重新确认。

### 4.2 先写失败测试

新增或扩展测试证明：

- Tauri 日志配置构造函数包含确认后的目标和级别。
- 开发环境开关不会改变发布构建的默认安全级别。
- 前端日志适配器能接收 Error、结构化 AppError 和未知值。
- 日志上下文字段不包含源码正文。
- 现有 bootstrap 测试在日志初始化成功和失败分支下行为明确。

测试不需要启动真实桌面窗口；优先把配置构造与调用适配提取为可测试函数。

### 4.3 实现 Rust 日志初始化

- 在 `src-tauri/src/lib.rs` 注册 `tauri-plugin-log`。
- 按确认值设置目标、级别、轮转和格式。
- 保持 `tauri_plugin_dialog` 初始化不变。
- 不改变现有 command 注册顺序和应用状态构造。
- 若插件需要持有 guard 或资源，确保生命周期覆盖应用运行期。

### 4.4 收敛 Rust 日志

- 将生产代码中的 `eprintln!` 替换为适当级别的 `log` 宏。
- 测试中的性能输出暂时保留到 Criterion 阶段，不把测试诊断混入应用日志。
- 为任务、文件和提交操作记录已确认的关联字段。
- 日志调用不得克隆大文本或序列化完整 overlay。

### 4.5 收敛前端日志

- 初始化 `@tauri-apps/plugin-log`。
- 将 `console.error` 迁移到统一适配器。
- 保留浏览器开发工具是否同步输出，按已确认配置执行。
- 错误通知与日志分离：通知面向用户，日志面向诊断。

### 4.6 定向验证

```powershell
cargo test -p revier-tauri
fnm exec --using-file pnpm.CMD test -- mainBootstrap errors diffAuthorRail monacoDiffSession
fnm exec --using-file pnpm.CMD typecheck
```

完成后提交：

- 依赖变化；
- 日志配置；
- 示例日志字段；
- 隐私检查；
- 测试结果。

获得确认后进入阶段二。

## 5. 阶段二：保留错误源与应用边界上下文

### 5.1 先写失败测试

新增测试证明：

- DuckDB 错误通过 `source()` 可访问。
- I/O 和 JSON 错误的 source 链保持完整。
- Tauri 映射后的 `code/message/detail` 与现有预期一致。
- 错误链日志包含阶段上下文，但用户消息不直接包含内部路径或 SQL。
- `Cancelled` 等可匹配领域错误仍可按枚举分支处理。

### 5.2 重构分析库错误

- 为适合保留的底层错误使用 `#[from]` 或 `#[source]`。
- 只有确实需要用户可读领域消息时保留字符串字段。
- 不用单一 `Box<dyn Error>` 抹平所有错误类型。
- 更新重复的 `duckdb_error` 辅助函数，使其保留 source。
- 不改变 `exit_code()` 的领域映射。

### 5.3 重构 Tauri 错误映射

- 集中内部错误到 DTO 的映射。
- 保持既有稳定错误码。
- 将适合诊断的 source 链写入日志。
- 不把完整内部错误链默认放入 `detail`。

### 5.4 条件性引入 `anyhow`

只有在阶段零获得确认时执行：

```powershell
cargo add -p revier-tauri anyhow
```

允许位置：

- Tauri 启动编排；
- CLI `main`；
- 不需要领域匹配的顶层任务编排。

禁止位置：

- `revier-analysis` 公共 API；
- Tauri command 返回类型；
- 前端生成契约。

### 5.5 定向验证

```powershell
cargo test -p revier-analysis error
cargo test -p revier-analysis --test api_contract
cargo test -p revier-analysis --test cli_contract
cargo test -p revier-tauri
fnm exec --using-file pnpm.CMD test -- errors
```

额外静态核查：

```powershell
rg -n "AppError::DuckDb\(error\.to_string\(\)\)|map_err\(\|error\| AppError::.*error\.to_string" crates/revier-analysis/src src-tauri/src
```

剩余字符串化位置必须有明确理由。

完成后提交错误变体变化、兼容性结果和残留清单供审核。

## 6. 阶段三：项目配置原子写入

### 6.1 依赖变更

获得阶段确认后执行：

```powershell
cargo add -p revier-tauri atomic-write-file
```

### 6.2 先写失败测试

在 `ProjectService` 测试中覆盖：

- 不存在文件时首次写入。
- 已有文件时完整替换。
- 保存后重新创建 Service 仍能读取。
- 写入或 commit 失败时旧内容保持不变。
- 目标父路径异常时返回既有写入错误码。
- JSON 格式和末尾换行不变。
- 项目增删改和重复仓库检查行为不变。

不通过修改既有测试期望来适配新实现。

### 6.3 实现原子写入

- 保留 `serde_json::to_string_pretty`。
- 在目标文件同目录创建原子写文件。
- 写入完整内容和末尾换行。
- 显式 commit。
- 将打开、写入和 commit 错误映射为现有 `PROJECT_STORE_WRITE_FAILED`。
- 不在失败后删除或重建旧文件。

### 6.4 定向验证

```powershell
cargo test -p revier-tauri services::projects
cargo test -p revier-tauri
```

Windows 上额外验证：

- 目标文件已存在时可替换；
- 文件句柄正常释放；
- 连续快速保存不会留下临时文件。

完成后提交文件格式对比、失败恢复测试和 Git diff 供审核。

## 7. 阶段四：Diff 基准与 `imara-diff` 评估

### 7.1 开发依赖

计划仅作为分析库开发依赖添加：

```powershell
cargo add -p revier-analysis --dev criterion
cargo add -p revier-analysis --dev imara-diff
```

在 `crates/revier-analysis/Cargo.toml` 增加明确的 bench target，并按 Criterion 要求关闭默认 harness。

### 7.2 固化生产语义测试

在写基准前新增或确认测试：

- 空文本；
- 尾换行存在与缺失；
- 重复行；
- 连续增加和删除；
- 替换块；
- 大小差异显著的两侧；
- `LineDiffPart` 合并规则；
- 通过 parts 重建旧文本和新文本。

这些测试只固定当前语义，不修改生产算法。

### 7.3 建立基准夹具

新增稳定夹具生成器：

- 小型代码样本；
- 中型重复行样本；
- 大型局部变化样本；
- 大型完全不同样本；
- 可配置行数但固定随机种子或完全确定生成逻辑。

基准不得依赖用户私有仓库内容。若使用当前仓库文件，只记录文件类别和规模，不把正文写入报告。

### 7.4 实现两个基准路径

- 现有 `diff_lines`。
- `imara-diff` Histogram。
- `imara-diff` Myers。
- 必要时增加 `similar` 只作为补充对照，但必须先确认是否增加开发依赖。

记录每组：

- 字节数；
- 行数；
- 变化比例；
- 算法；
- 耗时统计；
- 输出块数量。

### 7.5 语义对照

实现测试或分析工具比较：

- Equal/Added/Removed 序列；
- 重复行锚点；
- 块拆分和合并；
- 尾换行；
- 输出是否可重建两侧文本；
- 对上层 `rows` 和 `blocks` 的影响。

不同算法输出不要求逐块完全相同，但必须列出会影响 UI、签名和缓存的差异。

### 7.6 运行基准

```powershell
cargo bench -p revier-analysis --bench line_diff
```

记录：

- CPU 和内存基本信息；
- Rust 版本；
- 构建模式；
- 样本规模；
- Criterion 报告摘要。

### 7.7 独立确认节点

提交基准报告，给出以下建议之一：

- 保留现有算法；
- 仅优化现有算法；
- 采用 `imara-diff` Histogram；
- 采用 `imara-diff` Myers；
- 继续扩大样本后再决定。

未经书面确认：

- 不把 `imara-diff` 移到生产依赖；
- 不修改 `build_overlay_diff` 的生产调用；
- 不更新块签名或缓存版本；
- 不修改既有 Diff 测试预期。

## 8. 阶段五：使用 VueUse 收敛工具代码

### 8.1 版本能力核查

按阶段零确认结果：

- 若保持当前版本，只使用当前版本已存在的 API。
- 若确认升级，先单独完成 VueUse 升级和回归，再进行代码迁移。

禁止把依赖升级与行为重构混在一个未审核节点中。

### 8.2 `useStorage`

先扩展测试：

- 空存储返回默认布局。
- 合法旧值可恢复。
- 非法 JSON、非数字和缺字段回退行为保持一致。
- resize 后保存 clamp 后的布局值。
- 存储键保持 `revier.reviewLayout.v1`。

再替换：

- `JSON.parse`；
- `JSON.stringify`；
- 直接 `localStorage.getItem/setItem`。

显式配置错误处理和默认值合并，不依赖未经确认的库默认行为。

### 8.3 `useEventListener`

逐个迁移：

- 窗口 resize；
- 编辑器状态栏 document pointer/keyboard；
- 布局拖拽 pointermove/pointerup；
- 系统主题变化监听。

测试：

- 挂载时注册；
- 卸载时清理；
- once 行为保持；
- 主题监听异常仍按现有通知规则处理。

### 8.4 `useIntervalFn`

迁移操作状态栏计时：

- 运行态按当前频率更新时间；
- 终态停止更新；
- 组件卸载停止；
- 不改变 `elapsedMs` 展示格式。

### 8.5 `useDebounceFn`

迁移筛选保存：

- 延迟时间保持现值；
- 连续输入只保存最后一次；
- 页面卸载或项目切换时的待执行行为必须按现有测试确认；
- 保存失败仍通过统一错误通知和日志处理。

### 8.6 定向验证

```powershell
fnm exec --using-file pnpm.CMD test -- useReviewLayoutSizes useEditorSettings editorStatusBar operationStatusBar reviewWorkspace
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD lint
```

完成后提交：

- 删除的手写监听和计时器数量；
- 存储兼容结果；
- 组件清理验证；
- 测试结果。

## 9. 阶段六：Pinia Colada 普通查询试点

### 9.1 确认集成边界

根据阶段零书面选择执行：

- 独立 `queries/*` composable；或
- 现有 Store 包装查询。

同时确认：

- 项目列表缓存时间；
- 分支、作者和缓存状态失效时机；
- 是否重试；
- 是否后台刷新；
- 窗口重新聚焦是否刷新；
- mutation 成功后使哪些 query key 失效。

不得直接使用未经确认的默认值。

### 9.2 依赖变更

```powershell
fnm exec --using-file pnpm.CMD add @pinia/colada
```

在 `main.ts` 注册插件时保持顺序明确：

- 创建 Vue App；
- 安装 Pinia；
- 安装 Pinia Colada；
- 安装 Router 和 Naive UI；
- mount。

若插件要求的安装顺序与现有 bootstrap 测试冲突，先提交影响说明。

### 9.3 Query Key 测试

先测试：

- 项目列表使用稳定全局 key。
- 分支 key 包含 `projectId`。
- 作者 key 包含项目、分支及所有影响结果的筛选字段。
- 缓存状态 key 包含项目和分支。
- key 不包含大对象引用或不稳定临时对象。

### 9.4 项目列表试点

- 将项目列表读取迁入 query。
- 项目新增、修改、删除继续使用 mutation 或现有 action。
- 写操作成功后按确认规则失效项目列表。
- 保持 ProjectHome 加载、空状态和错误提示不变。

### 9.5 分支、作者和缓存状态

按风险从低到高逐项迁移：

1. 分支列表；
2. 分支缓存状态；
3. 作者列表。

每迁移一项都运行定向测试，不一次性删除所有 `requestId`。

作者查询必须确保筛选上下文完整进入 key，防止跨分支或跨项目复用。

### 9.6 统一查询错误副作用

- 使用 Pinia Colada Query Hook 或已确认的等价入口。
- metadata 指定面向用户的错误标题和来源。
- `toErrorMessage` 负责格式化。
- `addNotification` 负责用户通知。
- 日志适配器记录诊断。
- 同一查询失败只产生一次通知。

### 9.7 明确保留现有状态机

以下代码不得因试点被迁移或删除：

- `analysisRequestId`；
- `overlayRequestId`；
- `attributionRequestId`；
- `drilldownRequestId`；
- `pendingAnalysis`；
- `fileRefreshFallback`；
- operation ID 和进度快照合并；
- Tauri event 订阅与迟到事件过滤。

如某个普通查询当前共享上述状态，先拆分边界并提交设计确认。

### 9.8 定向验证

```powershell
fnm exec --using-file pnpm.CMD test -- rendererProjectStore rendererReviewStore projectHome reviewWorkspace revierClient
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD lint
```

重点验证：

- 重复读取去重；
- 项目／分支切换；
- mutation 后失效；
- 迟到结果；
- 错误通知一次；
- 长任务回归。

完成后提交 Store 复杂度变化、保留的请求序号和试点收益供审核。

## 10. 阶段七：缓存、校验和路径库评估

本阶段不安装候选生产依赖。

### 10.1 `moka/lru`

评估：

- `progress_by_file_operation` 的一小时过期和 128 条上限；
- 任务、文件和上下文是否需要不同淘汰策略；
- 淘汰回调是否需要清理取消令牌；
- 自动淘汰是否会删除仍在运行的操作。

输出：

- 可替换代码量；
- 所需 key/value 结构；
- 同步或异步 cache 选择；
- 建议接入或暂缓。

### 10.2 `dashmap/parking_lot`

评估：

- 单个 Map 的读写竞争；
- 多 Map 更新是否需要同一事务式临界区；
- 是否应先将任务、筛选、文件、上下文和取消令牌合并为 `TaskRecord`；
- panic 后继续使用状态是否安全。

不得仅以删除 `.lock().expect()` 为理由直接采用。

### 10.3 `garde/validator`

评估：

- `EditorSettingsFile` 的嵌套范围校验；
- 颜色字符串的自定义校验；
- 错误路径和中文消息；
- derive 对私有配置结构的侵入；
- 是否能减少现有测试和手写代码。

### 10.4 `directories/path-clean/dunce/camino`

评估：

- Tauri 层已有 `app.path()`，是否无需新增目录库；
- 独立分析 CLI 默认数据库路径；
- Windows 驱动器、UNC、大小写和符号链接；
- 词法清理与文件系统 canonicalize 的区别；
- 非 UTF-8 路径是否排除 `camino`。

### 10.5 输出评估结论

在本期验证记录中增加表格：

| 候选 | 代码位置 | 收益 | 风险 | 结论 | 下一步确认 |
| --- | --- | --- | --- | --- | --- |

每项结论只能是：

- 建议下一期接入；
- 暂缓；
- 拒绝。

不使用模糊的“以后可以考虑”作为最终结论。

## 11. 阶段八：全量回归与成果文档

### 11.1 自动化验证

```powershell
cargo fmt --all -- --check
cargo test --workspace
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD test
fnm exec --using-file pnpm.CMD lint
```

如果项目已有 Clippy 要求，经确认后执行：

```powershell
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

Clippy 不在未确认时作为新增强制门禁。

### 11.2 静态核查

```powershell
rg -n "eprintln!|console\.error" crates/revier-analysis/src src-tauri/src src/renderer
rg -n "fs::write\(&self\.file_path" src-tauri/src
rg -n "localStorage|addEventListener|removeEventListener|setInterval|setTimeout" src/renderer
rg -n "AppError::DuckDb\(error\.to_string\(\)\)" crates/revier-analysis/src
```

剩余匹配需要逐项说明，不能只追求搜索结果为零。

### 11.3 Diff 基准

再次运行：

```powershell
cargo bench -p revier-analysis --bench line_diff
```

报告本阶段只记录已确认的基准，不据此擅自替换算法。

### 11.4 验证记录

新增：

`docs/superpowers/verification/2026-07-28-revier-third-party-integration-phase-1.md`

内容：

- 实际新增依赖和版本；
- 变更文件清单；
- 日志配置和隐私规则；
- 错误链改造摘要；
- 原子写入验证；
- Diff 基准环境和结果；
- VueUse 迁移位置；
- Pinia Colada 试点范围和参数；
- 候选库评估结论；
- 自动化测试命令和退出状态；
- 已知限制；
- 下一期建议。

### 11.5 Git 状态核查

```powershell
git status --short
git diff --check
git diff -- docs/superpowers/specs/2026-07-28-revier-third-party-integration-design.md
git diff -- docs/superpowers/plans/2026-07-28-revier-third-party-integration-implementation.md
```

必须明确证明：

- `docs/third-party-integration-backlog.md` 没有加入暂存区；
- 没有修改用户原有未跟踪文档；
- 没有意外提交构建产物、日志文件或 Criterion 报告目录。

## 12. 阶段验收矩阵

| 阶段 | 主要成果 | 必须审核的内容 |
| --- | --- | --- |
| 0 | 基线和精确配置决策 | 日志、错误边界、VueUse 版本、查询策略 |
| 1 | 统一日志链路 | 目标、字段、隐私和启动行为 |
| 2 | 错误 source 保留 | 错误码兼容和 `anyhow` 使用范围 |
| 3 | 原子项目存储 | 失败恢复和文件格式 |
| 4 | Diff 基准 | 是否替换生产算法 |
| 5 | VueUse 收敛 | 存储、监听、计时和防抖语义 |
| 6 | Pinia Colada 试点 | 查询边界、缓存和失效 |
| 7 | 候选库评估 | 下一期接入清单 |
| 8 | 全量回归 | 测试、基准、已知限制和 Git 范围 |

## 13. 最终验收标准

- 日志、错误、原子写入、Diff 基准、VueUse、普通查询试点和库评估均按 Spec 完成。
- 不发生未经确认的 UI、接口、数据库或算法语义变化。
- 日志默认不包含源码正文和未确认的敏感字段。
- `projects.json` 写入失败时旧文件保持完整。
- `thiserror` 领域匹配和前端错误码保持稳定。
- Pinia Colada 不接管长任务和 Tauri 事件状态机。
- `imara-diff` 在独立确认前不进入生产路径。
- 本期只评估的库没有被擅自加入生产依赖。
- 所有定向测试和全量回归通过。
- 形成可复查的验证记录。
- `docs/third-party-integration-backlog.md` 保持未提交。

## 14. 回滚策略

每个阶段保持独立：

- 日志：移除插件初始化和前端桥接，恢复原入口。
- 错误：恢复错误变体，但不得丢失已经确认的错误码兼容测试。
- 原子写入：可恢复原写入实现，`projects.json` 格式不需要迁移。
- Diff 基准：开发依赖和 bench 文件可独立移除，不影响生产。
- VueUse：逐文件恢复手写实现，存储键不变。
- Pinia Colada：查询试点可逐项回退至现有 Store action。
- 评估：没有运行时变更，无需代码回滚。

不得使用 `git reset --hard` 或覆盖用户改动实施回滚。回滚应通过明确补丁完成。

## 15. 建议的语义化提交拆分

仅作为后续获得 Git 提交授权后的建议；不包含本地 backlog：

1. `docs(architecture): 增加第三方库接入一期设计与计划`
2. `feat(logging): 统一 Tauri 与前端本地日志链路`
3. `refactor(error): 保留分析错误源与边界上下文`
4. `fix(projects): 使用原子写入保存项目配置`
5. `perf(diff): 增加行差异算法基准`
6. `refactor(ui): 使用 VueUse 收敛浏览器工具代码`
7. `refactor(query): 使用 Pinia Colada 试点普通查询`
8. `docs(verification): 记录第三方库接入一期验证结果`

若 Diff 算法替换后续获得独立确认，应使用单独提交，不包含在第 5 项基准提交中。
