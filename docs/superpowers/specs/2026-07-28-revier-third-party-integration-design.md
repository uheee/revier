# Revier 成熟第三方库接入一期设计

## 1. 文档状态

- 日期：2026-07-28
- 状态：待审核
- 适用范围：日志、错误源、项目配置原子写入、Diff 基准、VueUse 收敛、Pinia Colada 普通查询试点，以及缓存／校验／路径库评估
- 实施计划：`docs/superpowers/plans/2026-07-28-revier-third-party-integration-implementation.md`
- 本地待办：`docs/third-party-integration-backlog.md`

## 2. 背景与问题

Revier 当前已经使用多项成熟第三方能力：

- Tauri 2、Vue 3、Pinia、Vue Router、Naive UI；
- Monaco、Shiki、VueUse；
- Rust `thiserror`、Serde、Specta、DuckDB、gix、Clap；
- Vitest、Insta 和大量 Rust/TypeScript 测试。

本次不以增加依赖数量为目标，而是处理以下已经形成维护成本或可靠性风险的位置：

1. Rust、Tauri 和前端没有统一日志链路，调试信息仍散落在 `eprintln!` 与 `console.error`。
2. 分析库虽然使用 `thiserror`，但底层错误大量提前转换为字符串，导致错误源链丢失。
3. `projects.json` 通过 `fs::write` 直接覆盖，异常中断时可能产生半文件。
4. 行 Diff 使用自研 LCS，缺少统计稳定的基准和成熟算法对照。
5. VueUse 已安装，但本地存储、事件、计时器和防抖仍为手写实现。
6. 普通异步查询与长任务状态混在 Pinia Store 中，存在大量重复的请求序号、加载和错误处理。
7. 缓存、并发映射、配置校验和路径规范化存在第三方库接入机会，但尚不足以直接决定生产替换。

## 3. 已确认目标

1. 接入 `tauri-plugin-log + log`，形成 Rust、Tauri 和前端可关联的本地日志链路。
2. 保留 `thiserror`，修复可保留错误源却被字符串化的位置。
3. 评估 `anyhow` 是否仅用于 Tauri/CLI 应用边界，不让动态错误扩散到分析库领域接口。
4. 使用 `atomic-write-file` 原子保存 `projects.json`，保持现有 JSON 结构和存储位置。
5. 使用 Criterion 建立现有 LCS 与 `imara-diff` 的对照基准。
6. 基准完成前不替换生产 Diff；替换必须经过独立书面确认。
7. 使用现有 `@vueuse/core` 收敛存储、事件、计时器和防抖代码。
8. 使用 `@pinia/colada` 试点项目列表、分支、作者和分支缓存状态等普通查询。
9. 保留项目分析、Tauri 事件流、文件归因、提交下钻和回退状态机的现有控制权。
10. 对 `moka`、`dashmap`、`parking_lot`、`garde`、`directories`、`path-clean` 等形成评估结论，本期不直接接入。
11. 不引入 DuckDB ORM，不迁移数据库。
12. 所有实施使用 TDD；不得擅自修改既有测试业务语义。

## 4. 非目标

- 不在本期迁移 DuckDB、重写仓储层或引入 ORM。
- 不在本期替换数据库迁移执行器。
- 不改变 Git 分析、归因、缓存和提交下钻算法语义。
- 不改变 Tauri 命令名称、事件名称和前端生成绑定。
- 不改变项目列表 JSON 文件路径和结构。
- 不新增远程日志、远程指标或崩溃上报。
- 不默认记录源码正文、Diff 正文、作者邮箱或完整仓库绝对路径。
- 不以 Pinia Colada 替换整个 `reviewStore`。
- 不在没有基准数据和用户确认时替换现有 LCS。
- 不在本期直接接入 `moka`、`garde`、`directories` 或路径库。

## 5. 技术选型

### 5.1 日志

一期采用：

- Rust 日志门面：`log`
- Tauri 日志实现：`tauri-plugin-log`
- 前端桥接：`@tauri-apps/plugin-log`

选择原因：

- Tauri 官方插件可覆盖 Rust、WebView 和本地文件目标。
- 当前最直接的问题是日志落盘、统一入口和跨端关联，不需要先引入完整分布式追踪体系。
- `log` 对现有同步 Rust 分析代码侵入较小。

一期不直接采用 `tracing`。以下条件出现时再独立评估：

- 需要嵌套 span 表达项目分析、文件归因和提交下钻的调用树；
- 需要统一采集每阶段耗时而不是只记录事件；
- 需要与 OpenTelemetry 或其他结构化订阅器对接。

以下日志配置不得在实施时擅自决定，必须在日志阶段开始前单独确认：

- 调试和发布构建的默认级别；
- 文件轮转大小和保留数量；
- 是否输出到 stdout、stderr 和 WebView；
- 仓库路径、分支、作者和提交字段的脱敏粒度。

### 5.2 错误处理

继续使用两层模型：

```text
分析库内部错误
  -> thiserror 强类型枚举并保留 source
  -> Tauri 服务边界映射稳定错误码
  -> contracts::AppError
  -> 前端统一展示和日志记录
```

规则：

- 领域错误继续使用 `thiserror`。
- 可直接保存的 `duckdb::Error`、`std::io::Error`、`serde_json::Error` 等不得仅保存 `to_string()`。
- gix 错误类型较分散，是否按模块增加透明变体或在应用边界包装，需要按实际类型逐项评估。
- 前端继续只依赖稳定的 `code/message/detail` 契约。
- 内部 `source` 链进入诊断日志，不自动复制到用户可见消息。

`anyhow` 只作为待确认的应用边界候选：

- 若引入，仅允许用于 Tauri 启动、CLI `main` 或不需要领域匹配的编排层。
- 不将 `anyhow::Error` 加入分析库公共 API。
- 不用 `anyhow` 替代前端需要匹配的稳定错误码。

### 5.3 项目配置原子写入

采用 `atomic-write-file`：

```text
序列化完整 ProjectStoreFile
  -> 在目标目录创建临时文件
  -> 写入完整内容
  -> commit 原子替换 projects.json
  -> 成功后返回
```

兼容要求：

- `projects.json` 的路径不变。
- JSON 字段、camelCase 规则、缩进和末尾换行不变。
- 读取逻辑和文件不存在时返回空列表的行为不变。
- 现有 `PROJECT_STORE_*` 错误码保持稳定。
- 一期不引入备份文件格式，不迁移至 `tauri-plugin-store`。

多实例写入不在一期解决。若测试或实际运行证明存在多进程并发写入，再在 `tauri-plugin-single-instance` 与 `fs2` 之间单独选型。

### 5.4 Diff 基准

采用：

- 基准框架：`criterion`
- 对照算法：`imara-diff`
- 生产基线：现有 `overlay::line_diff`

基准输入至少包含：

1. 空文本与单行文本。
2. 小型典型代码改动。
3. 仅增行、仅删行和整文件替换。
4. 高重复行文本。
5. 大量相同行中夹杂少量变化。
6. 大文件且两侧差异显著。
7. 当前仓库可公开使用的固定样本或生成样本。

记录：

- 吞吐或单次耗时；
- 输入行数和字节数；
- 算法选择；
- 尾换行处理；
- 生成块数量；
- 语义差异摘要。

Criterion 和 `imara-diff` 首先作为开发依赖。基准完成后提交报告，报告至少回答：

- 哪些输入下现有 LCS 存在明显退化；
- `imara-diff` Histogram/Myers 哪个更适合当前 UI；
- 是否改变重复行锚点、块边界或尾换行语义；
- 适配 `LineDiffPart` 的复杂度；
- 是否值得进入生产替换。

生产替换属于新的关键决策节点，不包含在本设计自动授权范围内。

### 5.5 VueUse

一期只使用已安装的 `@vueuse/core`：

| 当前实现 | 目标能力 |
| --- | --- |
| `localStorage + JSON.parse/stringify` | `useStorage` |
| `addEventListener/removeEventListener` | `useEventListener` |
| `setInterval/clearInterval` | `useIntervalFn` |
| `setTimeout/clearTimeout` 防抖 | `useDebounceFn` |

约束：

- 保持 `revier.reviewLayout.v1` 等存储键不变。
- 保持默认布局、边界收缩和 resize 时机不变。
- 保持筛选保存的等待时长和最后一次写入优先语义不变。
- 保持操作状态栏约 10 Hz 的显示刷新语义不变。
- 不为了使用 VueUse 改变组件 UI 或 Store 公共接口。

当前依赖为既有版本。是否升级 VueUse 主版本属于独立确认事项；未确认时只使用当前版本已提供且测试可覆盖的 API。

### 5.6 Pinia Colada

一期定位为“普通查询试点”，不作为完整状态管理迁移。

适合试点：

- 项目列表；
- 分支列表；
- 作者列表；
- 分支缓存状态。

不适合迁移：

- `review_start_analysis` 及其任务快照合并；
- `review://task-updated` 和 `review://operation-progress`；
- 文件 overlay、Monaco Diff 和渐进归因；
- 用户取消；
- 刷新失败时恢复旧 overlay；
- 提交下钻的前台优先级；
- 依赖 operation ID 的迟到事件过滤。

目标数据流：

```text
页面／查询 composable
  -> Pinia Colada query key
  -> revierClient 普通 invoke
  -> 查询缓存和去重
  -> 成功数据
  -> 页面或现有 Store 消费

查询失败
  -> 统一 Query Hook
  -> toErrorMessage
  -> App Notification + 统一日志
```

在实施前需要确认集成边界：

- 方案 A：页面和新建 `queries/*` composable 直接使用 Pinia Colada，Store 只保存本地工作流状态。
- 方案 B：现有 Store 包装查询结果，暂时保持页面调用方式不变。

一期默认不定义查询缓存时间、重试次数和后台自动刷新；这些参数必须根据桌面本地 IPC 特征单独确认，不能沿用 Web API 默认行为。

### 5.7 缓存、校验和路径库评估

本期只形成评估结论：

| 领域 | 候选 | 主要问题 |
| --- | --- | --- |
| 有界缓存 | `moka`、`lru` | 能否替代操作注册表手写 TTL/容量淘汰 |
| 并发映射 | `dashmap` | 多个 Map 之间存在一致性时是否反而增加风险 |
| 互斥锁 | `parking_lot` | 不再 poison 是否符合任务状态安全要求 |
| 配置校验 | `garde`、`validator` | 手写嵌套校验的收益是否足以增加依赖 |
| 应用目录 | `directories` | 独立 CLI 是否应替代 APPDATA/HOME/XDG 手写分支 |
| 路径清理 | `path-clean`、`dunce`、`camino` | 词法清理、Windows canonicalize 和 UTF-8 路径边界 |

每项必须输出：

- 适用代码位置；
- 预期删除的样板代码；
- 兼容性和平台风险；
- 是否需要数据迁移或 API 变化；
- “接入、暂缓、拒绝”结论；
- 如建议接入，下一期的独立确认点。

## 6. 总体架构

```text
Vue 页面与 Store
  ├─ 普通查询 ─────────► Pinia Colada ─► revierClient.invoke
  ├─ 长任务状态 ───────► 现有 Pinia ReviewStore
  ├─ 浏览器工具 ───────► VueUse
  └─ 前端错误／诊断 ───► @tauri-apps/plugin-log
                                  │
                                  ▼
Tauri Builder ─────────► tauri-plugin-log ─► 本地日志目标
  │                               ▲
  ├─ Command / Service ─► log ────┘
  │
  ├─ ProjectService ────► atomic-write-file ─► projects.json
  │
  └─ ReviewService ─────► revier-analysis
                              ├─ thiserror + source
                              ├─ DuckDB / gix
                              └─ 现有生产 Line Diff

开发基准：
现有 Line Diff ─┬─► Criterion
imara-diff ─────┘
```

## 7. 模块划分

### 7.1 日志初始化模块

职责：

- 在 Tauri Builder 中初始化日志插件。
- 保存非阻塞日志或插件所需的生命周期资源。
- 定义允许记录的字段和禁止记录的内容。
- 在前端 bootstrap 时初始化前端日志桥接。

日志调用点保持就近，不新增多层日志包装框架。仅当需要统一附加上下文时增加轻量辅助函数。

### 7.2 错误映射模块

职责：

- 分析库错误保留领域类型和底层 source。
- Tauri 层集中把内部错误映射为稳定 DTO。
- 前端集中把未知异常转换为展示消息。
- 日志记录内部链，UI 只展示适合用户的内容。

### 7.3 ProjectService

继续负责项目列表领域规则和 JSON 序列化。原子写入库只替代文件落盘步骤，不接管：

- 项目去重；
- 名称默认值；
- 路径规范化；
- 偏好设置合并；
- 错误码定义。

### 7.4 Diff 基准模块

基准放在分析库 `benches` 下，不进入 Tauri 运行路径。基准适配层只负责把两个算法运行在相同输入上，不在本期修改 UI 契约。

### 7.5 前端查询模块

若最终确认采用独立 composable，新增 `src/renderer/queries/*`：

- query key 工厂；
- 普通查询定义；
- 查询失效入口；
- 统一错误 metadata。

若最终确认由 Store 包装，则仍需把 query key 和查询函数独立出来，避免把 Pinia Colada 细节散落在组件中。

## 8. 数据流程

### 8.1 日志流程

```text
操作开始
  -> 记录 operation_id / kind / stage
  -> 业务执行
     -> 阶段变化记录 debug/info
     -> 可恢复异常记录 warn
     -> 失败记录 error + 内部 source
  -> 终态记录 status / elapsed_ms / cache_state
```

日志字段不得直接包含源文件正文和 Diff 正文。完整路径和作者邮箱是否记录取决于实施前的隐私配置确认。

### 8.2 项目写入流程

```text
读取当前 projects.json
  -> 应用领域变更
  -> 序列化完整新快照
  -> AtomicWriteFile 写临时文件
  -> commit 替换
  -> 返回保存后的项目

任一步失败
  -> 返回既有 PROJECT_STORE_* 错误码
  -> 原 projects.json 保持可读
```

### 8.3 普通查询流程

```text
路由上下文变化
  -> 计算 query key
  -> 查询缓存命中：返回当前数据
  -> 未命中：执行 Tauri invoke
       -> 成功：缓存并发布
       -> 失败：统一格式化、通知和日志
```

长任务仍沿用 operation ID 和事件上下文，不通过普通查询缓存表达。

## 9. 接口兼容规范

一期默认不修改：

- Rust `contracts.rs` 的序列化契约；
- 生成的 `bindings.ts`；
- Tauri command 名称和参数；
- Tauri event 名称和 payload；
- `projects.json` Schema；
- Vue 路由地址；
- UI 布局和交互；
- Diff block、row 和 attribution 契约。

若实施发现必须修改以上任一接口，应停止对应阶段，提交：

1. 当前接口；
2. 拟修改接口；
3. 迁移和兼容方案；
4. 测试变化；
5. 修改原因；

取得书面确认后继续。

## 10. 错误、恢复与隐私

- 日志初始化失败不得导致应用静默退出；具体回退行为在日志配置节点确认。
- 日志写入失败不得覆盖原业务错误。
- 项目原子写入失败时保留旧文件，并返回稳定错误。
- Pinia Colada 查询失败不得清除仍可展示的旧数据，具体 stale-data 行为在查询参数节点确认。
- Diff 基准失败不影响生产构建和应用启动。
- 评估阶段不得把未确认依赖加入生产清单。
- 默认不进行远程上报。
- 日志不得记录源码正文、Diff 正文、访问令牌或系统环境变量内容。

## 11. 开发环境

- Shell：PowerShell。
- Rust：Cargo workspace。
- Node：通过 `fnm` 使用 `.node-version`，包管理器为 `pnpm`。
- Python：本期不需要；如需辅助脚本，使用 `uv`。
- 文档和代码注释使用中文。
- Git 提交消息使用语义化提交格式。
- 未经明确要求，不执行提交、推送或发布。
- `docs/third-party-integration-backlog.md` 永不加入 Git 提交。

## 12. 测试策略

### 12.1 日志

- Tauri 构建器注册日志插件。
- Rust 日志调用不再依赖 `eprintln!`。
- 前端异常通过日志 API 上报。
- 日志字段构造测试不包含源码正文。
- 如插件难以在单元测试环境启动，测试配置构造和调用适配层，不启动真实桌面窗口。

### 12.2 错误

- `source()` 可访问底层 DuckDB、I/O 和 JSON 错误。
- Tauri 映射后的错误码与现有契约一致。
- 内部 detail 与用户展示消息分离。
- 前端 `toErrorMessage` 继续处理 Error、结构化 DTO 和未知值。

### 12.3 原子写入

- 新文件首次保存成功。
- 现有文件替换成功。
- 失败时旧内容保持完整。
- 反序列化错误行为不变。
- 项目增删改测试继续通过。

### 12.4 Diff 基准

- 基准可独立编译运行。
- 固定输入的现有输出测试继续通过。
- 增加两个算法的语义对照测试，但不修改既有断言。
- 基准结果记录环境、输入规模和算法参数。

### 12.5 VueUse

- 布局存储键、默认值和 clamp 结果不变。
- 组件卸载自动清理监听器和定时器。
- 防抖只保存最后一次筛选。
- 状态栏耗时刷新和终态冻结行为不变。

### 12.6 Pinia Colada

- query key 包含足以区分项目、分支和筛选上下文的字段。
- 重复普通查询被去重。
- 上下文切换后旧数据不覆盖新上下文。
- 错误通知不重复。
- 长任务 Store 和事件测试不发生语义变化。

### 12.7 评估

- 每个候选库都有证据、适配点和结论。
- 评估结论不以安装依赖作为完成条件。

## 13. 验收标准

- Rust、Tauri 与前端使用统一日志入口，现有零散调试输出被收敛。
- 日志可通过 operation ID 关联操作阶段和终态，且默认不记录源码正文。
- 分析库主要底层错误保留 `source` 链。
- 前端错误码、提示和既有契约没有未经确认的变化。
- `projects.json` 使用原子替换，失败时旧文件保持可读。
- Criterion 可重复比较现有 LCS 与 `imara-diff`，并生成书面结论。
- 生产 Diff 算法在独立确认前保持不变。
- VueUse 替换手写工具代码后，存储键和交互时序保持一致。
- Pinia Colada 只管理确认范围内的普通查询。
- 长任务、事件、取消、刷新回退和归因状态行为不变。
- 缓存、校验、目录和路径库形成明确的接入建议，但本期不擅自增加生产依赖。
- 所有新增测试和既有回归通过。
- 本地待办文档没有进入 Git 暂存区。

## 14. 关键确认节点

1. 日志配置确认：级别、目标、轮转和隐私字段。
2. 错误边界确认：是否引入 `anyhow`，以及允许使用的层级。
3. VueUse 版本确认：保持当前版本或先升级。
4. Pinia Colada 集成边界确认：独立 composable 或 Store 包装。
5. Pinia Colada 查询策略确认：缓存、失效、重试和后台刷新。
6. Diff 基准结果确认：是否进入生产算法替换。
7. 评估结论确认：哪些候选进入下一期。

任何节点未确认时，只暂停对应阶段，不擅自采用默认值。

## 15. 风险与控制

| 风险 | 影响 | 控制 |
| --- | --- | --- |
| 日志记录敏感仓库内容 | 隐私泄露 | 字段白名单、正文禁记、配置节点确认 |
| 日志插件影响启动 | 应用无法打开 | 独立初始化测试、明确回退策略 |
| 错误类型调整改变错误码 | 前端分支失效 | source 与 DTO 分层、契约回归测试 |
| 原子写库的平台语义差异 | 配置替换失败 | Windows/Linux/macOS 测试和临时目录集成测试 |
| Diff 基准输入失真 | 得出错误替换结论 | 固定生成样本与真实样本并用、记录环境 |
| `imara-diff` 块边界不同 | 作者归因或 UI 变化 | 语义对照测试、生产替换独立确认 |
| VueUse 默认行为不同 | 存储或时序回归 | 显式参数、保持存储键、既有测试 |
| Pinia Colada 默认重试 | 本地 IPC 重复执行 | 参数单独确认，写操作不使用 query |
| 普通查询与工作流状态混淆 | 迟到结果覆盖前台状态 | 明确边界、query key 校验、保留 operation ID |
| 直接替换多个 Mutex Map | 跨 Map 一致性破坏 | 本期只评估，不直接替换 |
