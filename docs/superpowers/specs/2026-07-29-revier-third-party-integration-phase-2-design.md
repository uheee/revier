# Revier 成熟第三方库接入第二期设计

## 1. 文档状态

- 日期：2026-07-29
- 状态：已确认设计范围，待按实施计划分阶段开发
- 对应计划：`docs/superpowers/plans/2026-07-29-revier-third-party-integration-phase-2-implementation.md`
- 一期设计：`docs/superpowers/specs/2026-07-28-revier-third-party-integration-design.md`
- 一期验证：`docs/superpowers/verification/2026-07-28-revier-third-party-integration-phase-1.md`
- 本地待办：`docs/third-party-integration-backlog.md`，仅供本地维护，不加入 Git

## 2. 背景

第三方库接入一期已经完成以下工作：

1. 使用 `tauri-plugin-log + log` 统一 Rust、Tauri 与前端日志入口。
2. 保留 `thiserror` 领域错误，并仅在 Tauri/CLI 应用边界使用 `anyhow`。
3. 使用 `atomic-write-file` 原子保存项目配置。
4. 使用 `criterion` 和 `imara-diff` 建立行差异算法对照基准。
5. 使用 VueUse 收敛浏览器存储、事件、计时器和防抖代码。
6. 使用 Pinia Colada 完成项目列表普通查询试点。
7. 完成缓存、并发映射、配置校验和路径库评估。

一期解决了直接影响诊断、错误链、文件可靠性和局部维护成本的问题。第二期聚焦工程边界与长期质量，处理以下剩余风险：

- DuckDB 迁移由 Rust 函数和内联 SQL 手工组织，缺少可审计的迁移文件、checksum 和执行历史。
- Specta 只能提供编译期 TypeScript 类型，运行时 IPC、配置和持久化数据仍可能发生版本漂移或损坏。
- Diff、编码、缓存及前端异步事件包含大量适合性质测试的不变量，但当前主要依赖示例测试。
- Node 与 Rust 依赖缺少统一的未使用依赖、漏洞、许可证和来源检查。

第二期不是继续增加依赖数量，而是为已经存在的边界补充可验证、可审计且可持续运行的机制。

## 3. 已确认目标

### 3.1 供应链治理

1. 使用 `cargo-deny` 检查 Rust 许可证、已知安全公告、重复依赖、禁止依赖和依赖来源。
2. 使用 `knip` 检查 Node/TypeScript 未使用的直接依赖、文件和导出。
3. 使用 `pnpm audit` 检查 Node 依赖安全公告。
4. 将稳定、可重复的检查接入 CI。
5. 对现有疑似未使用依赖形成证据清单；删除依赖属于实现变更，必须在执行前独立确认。

### 3.2 DuckDB 迁移执行器增强

1. 保留当前 DuckDB 和自有迁移执行器，不引入 ORM，不迁移数据库。
2. 使用 `include_dir` 在编译期嵌入版本化 SQL 迁移文件。
3. 使用 `sha2` 为迁移正文生成稳定 checksum。
4. 增加迁移执行历史，记录版本、名称、checksum 和应用时间。
5. 保持迁移的事务性、幂等性、失败回滚和不兼容版本拒绝行为。
6. 对现有版本数据库采用兼容接管策略，不伪造已经逐条执行过的新式迁移历史。

### 3.3 运行时契约校验

1. 前端使用 `zod` 校验来自 IPC、配置文件和持久化文件的关键数据。
2. 不修改 Specta 生成类型作为唯一事实来源的定位；Zod 只补充运行时边界。
3. 不在前端内部已类型化对象之间重复校验。
4. Rust 侧继续使用 Serde 和现有领域校验，本期不引入 `garde` 或 `validator`。
5. 校验失败进入统一日志和通知链路，并返回稳定、可测试的边界错误。

### 3.4 性质测试与模糊测试

1. Rust 使用 `proptest` 覆盖关键领域不变量。
2. TypeScript 使用 `fast-check` 覆盖事件乱序、迟到结果和序列化往返。
3. 使用 `cargo-fuzz` 建立基础 fuzz target。
4. 固定回归种子必须能够进入普通测试；长期 fuzz 运行不作为普通 PR CI 的阻塞条件。
5. 性质测试和 fuzz 不替代现有针对性单元测试、契约测试和快照测试。

## 4. 非目标

第二期明确不包含：

- DuckDB ORM 复评、数据库迁移或仓储层重写。
- 系统通知、远程错误采集、指标或 OpenTelemetry。
- `imara-diff` 生产替换及 Diff 缓存签名变更。
- Pinia Colada 向 review 长任务、进度事件、取消流程和回退状态机扩展。
- `moka`、`lru`、`dashmap` 或 `parking_lot` 的生产接入。
- `garde`、`validator`、`directories`、`path-clean`、`dunce` 或 `camino` 接入。
- Dependabot、Renovate 或 `cargo-vet` 的自动化启用。
- 默认远程上报任何日志、错误、性能或使用信息。
- 为通过第三方检查而未经确认地删除依赖、修改业务行为或放宽既有测试。

`moka/lru` 仍保留为后续扩展候选。只有先将文件操作进度注册抽成专用结构，并确认运行中操作、取消令牌和事件 sink 的生命周期后，才重新评估生产接入。

## 5. 总体架构

第二期形成四条相互独立但可以组合验证的质量链路：

```text
依赖清单
  -> cargo-deny / knip / pnpm audit
  -> 本地报告
  -> CI 门禁

版本化 SQL
  -> include_dir 编译期嵌入
  -> sha2 checksum
  -> 自有 DuckDB runner
  -> 事务执行
  -> schema_migrations 历史

Tauri IPC / 配置 / 持久化数据
  -> Specta 静态类型
  -> Zod 运行时解析
  -> 领域对象
  -> Store / 页面
  -> 统一错误日志与通知

领域输入生成器
  -> proptest / fast-check
  -> 不变量断言
  -> 最小失败样本与固定种子
  -> cargo-fuzz 补充异常输入探索
```

四条链路按“供应链基线 → 数据迁移 → 运行时边界 → 生成式测试”的顺序实施。每一条链路均可独立提交和回滚。

## 6. 技术选型

### 6.1 `cargo-deny`

用途：

- 检查 RustSec 安全公告。
- 建立允许和拒绝的许可证规则。
- 限制未知 Git 源或非预期 registry。
- 发现重复版本和明确禁止的 crate。

设计约束：

- 首次扫描必须先形成当前依赖事实清单，再决定哪些告警进入阻塞模式。
- 许可证 allowlist 必须基于当前依赖实际许可证生成，并在实施节点单独确认。
- 多版本依赖不能仅因重复就直接拒绝；必须区分可消除重复和上游不可避免重复。
- 不在本期启用 `cargo-vet`，避免同时引入两套供应链审计工作流。

### 6.2 `knip`

用途：

- 检查未使用直接依赖、开发依赖、导出和入口文件。
- 复核 `@vueuse/components`、`diff`、`minimatch` 等当前未发现源码引用的依赖。

设计约束：

- 为 Vite、Vitest、Vue、Tauri CLI 和脚本入口显式配置，避免动态入口误报。
- 动态导入、Vue SFC、生成绑定和构建脚本必须纳入验证。
- Knip 报告只提供证据；删除依赖必须经过独立确认并运行完整回归。

### 6.3 `pnpm audit`

用途：

- 检查锁文件解析出的 Node 依赖安全公告。
- 与 `knip` 分工：`knip` 关注使用关系，`pnpm audit` 关注已知漏洞。

设计约束：

- CI 使用冻结锁文件。
- 阻塞等级在首次扫描后确认，避免未知历史告警直接使所有流程失效。
- 不使用 `--fix` 自动改写依赖版本。

### 6.4 `include_dir + sha2`

`include_dir` 用于将迁移目录作为编译期资源嵌入，避免发布包运行时依赖仓库相对路径。`sha2` 用于对标准化后的原始迁移字节计算 SHA-256。

每个迁移文件采用稳定命名：

```text
NNNN_<英文蛇形名称>.sql
```

示例：

```text
0004_create_migration_history.sql
0005_add_example_column.sql
```

规则：

- 版本号必须严格递增且不得重复。
- 文件名、原始字节和 checksum 在编译后保持确定性。
- 已成功应用的迁移文件不得修改。
- runner 启动时检查历史 checksum；已应用版本与当前文件不一致时返回 schema 不兼容错误。
- SQL 不允许依赖当前工作目录。

### 6.5 `zod`

采用 Zod 的原因：

- 支持组合对象、联合类型、可选字段和自定义校验。
- 错误路径适合映射为统一日志上下文。
- 可在前端边界使用 `safeParse`，避免异常直接穿透页面。
- 社区成熟，适合与现有 TypeScript 和 Vitest 测试配合。

本期不尝试从 Specta 类型自动生成全部 Zod schema。首期只为高价值边界手写小型 schema，并通过 `satisfies`、类型测试或等价方式尽量减少静态类型漂移。

优先边界：

1. 项目列表及项目配置持久化结果。
2. 编辑器配置读取结果。
3. 分支缓存状态等普通 IPC 查询结果。
4. 操作进度事件等跨异步边界数据。

长任务复杂结果是否纳入本期，以边界盘点和阶段确认结果为准；不得一次性包装全部 19 个 Tauri command。

### 6.6 `proptest`

优先覆盖：

- 行差异结果可以重建新旧两侧文本。
- Diff 块范围单调、不重叠且不越界。
- 文本编码解码往返及二进制识别不会 panic。
- 缓存模型序列化和反序列化往返保持关键字段。
- 迁移文件排序、版本解析和 checksum 对相同输入稳定。

性质测试使用确定的策略上限，避免普通测试运行时间无界增长。失败时保留最小化后的输入或固定 seed。

### 6.7 `fast-check`

优先覆盖：

- 项目、分支切换后迟到结果不能覆盖当前上下文。
- 同一查询失败只产生一次通知。
- 操作进度乱序时状态不从终态回退到运行态。
- 前端持久化模型经过编码、解码后保持关键字段。

生成器应描述领域约束，避免只生成大量无法到达业务路径的无效对象。

### 6.8 `cargo-fuzz`

首批 target：

- Diff 文本输入。
- 文本编码探测与解码。
- 迁移文件名及迁移清单解析。

限制：

- fuzz target 不读取用户仓库或真实项目配置。
- 不把生成 corpus、崩溃产物或大体积构建目录提交到 Git。
- 普通 CI 只验证 fuzz target 可构建；定时长跑或专用执行环境后续单独确认。

## 7. DuckDB 迁移模型

### 7.1 兼容策略

当前 schema 版本为 3。第二期不得让已有版本 1、2、3 数据库失去升级能力。

建议采用以下接管方式：

1. 保留现有版本 1、2 到版本 3 的兼容升级入口。
2. 新 runner 首次接管版本 3 数据库时，创建迁移历史表并写入一条明确标记的 legacy baseline。
3. 从新 runner 管理的下一版本开始，所有迁移必须来自嵌入 SQL 文件并记录 checksum。
4. 新建数据库直接初始化当前 schema，同时写入与 runner 一致的 baseline/迁移历史。
5. 不为旧数据库伪造“逐条执行过新式 SQL 文件”的历史。

最终表名采用：

```sql
schema_migrations
```

最小字段：

| 字段 | 含义 |
| --- | --- |
| `version` | 唯一递增版本 |
| `name` | 文件名中的稳定迁移名称 |
| `checksum` | SHA-256 十六进制摘要 |
| `applied_at` | UTC 应用时间 |
| `kind` | `baseline` 或 `migration` |

若实现阶段需要增加执行耗时或应用版本字段，必须在阶段确认时补充设计，不在实现中临时扩展。

### 7.2 执行事务

单个迁移按以下顺序执行：

```text
读取当前 schema_version 与历史
  -> 校验迁移清单连续性
  -> 校验已应用 checksum
  -> BEGIN TRANSACTION
  -> 执行一个迁移 SQL
  -> 写入 schema_migrations
  -> 更新 metadata.schema_version
  -> COMMIT
```

任一步失败：

- 执行 `ROLLBACK`。
- 保持迁移前 schema version 和历史。
- 返回保留 DuckDB source 的领域错误。
- 通过现有日志链记录版本、迁移名和阶段，不记录数据库内容。

每个迁移独立事务，便于确定失败边界；本期不将多个版本打包进一个大事务。

### 7.3 checksum 规则

- checksum 输入为嵌入文件的原始字节。
- 不自动改写换行符、尾部空白或编码。
- 历史中已存在同版本但 checksum 不同时，拒绝启动该数据库的分析操作。
- baseline 使用固定、可解释的标识，不伪装成普通 SQL 文件 checksum。

## 8. 运行时契约模型

### 8.1 校验层位置

校验适配器放在生成绑定与业务状态之间：

```text
generated/bindings.ts
  -> 原始 Tauri 调用
  -> boundary parser
  -> 已验证类型
  -> query / store / component
```

禁止直接修改生成文件加入 Zod 调用，因为重新生成绑定会覆盖手工代码。校验 schema 和 parser 放在独立目录，例如：

```text
src/renderer/contracts/runtime/
```

最终目录名在实施阶段开始前通过文件清单确认。

### 8.2 错误输出

校验失败至少包含：

- 稳定错误类别：`CONTRACT_VALIDATION_FAILED`。
- 来源边界：命令、事件、配置或持久化文件。
- Zod issue path 的安全摘要。
- 可关联日志上下文。

不得包含：

- 源码正文。
- 完整仓库绝对路径。
- 未确认可输出的作者邮箱。
- 整个无效 payload。

用户通知使用简化消息，例如“应用数据格式不兼容，请重试或升级应用”；详细 issue 只进入本地日志。

### 8.3 版本漂移

新增字段默认遵循向后兼容：

- schema 只读取当前业务需要字段。
- 对明确允许的额外字段不报错。
- 缺少必需字段、枚举值未知或类型错误时拒绝进入业务状态。
- 可选字段的默认值必须由领域规则明确指定，不使用未经确认的库默认值。

## 9. 测试策略

第二期继续采用 TDD。每个阶段遵循：

```text
新增失败测试
  -> 验证失败原因与目标一致
  -> 最小实现
  -> 定向测试
  -> 重构
  -> 全量回归
  -> 阶段审核
```

测试层次：

| 层次 | 主要工具 | 目标 |
| --- | --- | --- |
| 配置/静态检查 | `cargo-deny`、`knip`、`pnpm audit` | 供应链与依赖事实 |
| 单元测试 | Rust test、Vitest | parser、runner、错误映射 |
| 性质测试 | `proptest`、`fast-check` | 领域不变量 |
| 模糊测试 | `cargo-fuzz` | panic、越界和异常输入 |
| 集成测试 | DuckDB 临时数据库、现有 Tauri 边界测试 | 迁移和契约兼容 |
| 全量回归 | Cargo、pnpm、现有 CI 命令 | 防止跨模块回归 |

在 TDD 场景下，若必须修改既有测试的业务期望，例如 schema version 从 3 升级或错误码契约变化，必须先提交差异并取得书面确认。

## 10. CI 设计

建议新增独立质量检查工作流，在拉取请求和目标分支 push 时运行，避免只在发布标签阶段发现问题。

稳定检查候选：

```powershell
cargo deny check
fnm exec --using-file pnpm.CMD knip
fnm exec --using-file pnpm.CMD audit --audit-level high
```

CI 接入分两步：

1. 报告模式：收集当前告警并建立基线。
2. 门禁模式：仅对已确认类别失败。

长期 fuzz 不进入普通 CI。普通 CI 可以执行 fuzz target 的编译检查，具体命令需在实现阶段根据工具链确认。

## 11. 开发环境配置

### 11.1 Rust

- 使用项目锁定或当前稳定 Rust 工具链。
- Rust 依赖通过 `cargo add` 添加。
- `proptest` 作为开发依赖。
- `cargo-deny` 和 `cargo-fuzz` 作为开发工具安装，不加入应用运行时依赖。
- `cargo-fuzz` 需要的 nightly 工具链仅用于 fuzz 工作流，不改变普通构建工具链。

### 11.2 Node

- 使用仓库现有 `.node-version`/FNM 配置。
- 统一使用 `fnm` 和 `pnpm`。
- `zod` 为运行时依赖。
- `knip`、`fast-check` 为开发依赖。
- 不使用 npm 或 yarn 改写锁文件。

### 11.3 本地数据

- 迁移测试使用临时 DuckDB 数据库。
- 不直接以用户真实缓存数据库作为迁移测试夹具。
- fuzz corpus 和测试生成数据不得包含用户仓库内容。

## 12. 安全与隐私

- 供应链工具只读取依赖元数据，不上传项目源码。
- 若某个工具默认需要访问远程公告数据库，应在执行文档中记录数据来源和网络行为。
- Zod 错误日志只保留字段路径、预期类型和安全摘要，不记录完整 payload。
- 迁移日志不记录 SQL 查询结果和缓存正文。
- fuzz 崩溃输入在提交前检查是否仅由生成器构造。
- 第二期不引入任何远程遥测或崩溃上报。

## 13. 风险与控制

| 风险 | 影响 | 控制 |
| --- | --- | --- |
| 供应链初始告警过多 | CI 无法落地 | 先报告、确认基线，再启用门禁 |
| Knip 动态入口误报 | 删除有效依赖 | 配置入口并要求独立删除确认 |
| 历史迁移 checksum 漂移 | 旧数据库无法打开 | 使用 legacy baseline，从新 runner 后严格校验 |
| SQL 迁移部分执行 | schema 与版本不一致 | 单迁移事务、失败回滚、集成测试 |
| Zod 与 Specta 类型漂移 | 重复维护和误拒绝 | 只覆盖关键边界，增加类型/契约测试 |
| 校验错误泄漏 payload | 隐私风险 | 日志只保留 issue path 和安全摘要 |
| 性质测试运行过慢 | 开发反馈变慢 | 限定 case 数，长跑移出普通 CI |
| fuzz 结果不可复现 | 缺陷无法修复 | 固定 seed/corpus，转为普通回归测试 |

## 14. 独立确认节点

以下事项不得在实施时擅自决定：

1. `cargo-deny` 许可证 allowlist、禁止来源和重复依赖策略。
2. `pnpm audit` 的 CI 阻塞等级。
3. Knip 报告中实际删除哪些依赖。
4. DuckDB 新 runner 接管时的下一 schema version 和 baseline 具体记录。
5. 需要修改的既有 schema version 测试期望。
6. 第一批 Zod 边界的精确命令、事件和字段清单。
7. `CONTRACT_VALIDATION_FAILED` 是否需要进入共享生成契约。
8. 性质测试的默认 case 数及普通 CI 是否编译 fuzz target。
9. 新增 CI 工作流名称、触发分支和是否先采用非阻塞报告模式。

每个节点必须先提交事实、建议值和影响，取得书面确认后再实施。

## 15. 验收标准

### 15.1 供应链

- `cargo-deny`、`knip` 和 `pnpm audit` 可在本地重复运行。
- 已确认的许可证、安全公告和来源规则进入 CI。
- 未使用依赖清单有源码搜索和构建测试证据。
- 未经确认不删除依赖，不自动升级依赖。

### 15.2 迁移

- 迁移从编译期嵌入 SQL 读取，不依赖当前工作目录。
- 新式迁移记录版本、名称、checksum、时间和类型。
- 重复运行不重复应用迁移。
- SQL 或历史写入失败后完整回滚。
- 已应用迁移文件被修改时返回稳定不兼容错误。
- 现有版本 1、2、3 数据库仍可按确认策略升级。

### 15.3 契约校验

- 已确认的 IPC、配置和持久化边界均经过 Zod 校验。
- 无效数据不会进入 Store 或组件状态。
- 校验失败只产生一次用户通知并写入安全日志。
- 生成绑定可重新生成，校验层不会被覆盖。

### 15.4 测试

- `proptest` 与 `fast-check` 覆盖已确认不变量。
- 失败样本可以通过 seed 或固定夹具复现。
- 至少建立首批 `cargo-fuzz` target，并记录构建及短时运行方式。
- 全量 Rust、TypeScript 测试、类型检查和 lint 通过。

### 15.5 Git 范围

- `docs/third-party-integration-backlog.md` 保持未跟踪且不提交。
- 不提交依赖扫描缓存、审计数据库、fuzz corpus、崩溃产物或 Criterion 报告。
- 每一阶段使用独立语义化提交，便于审核和回滚。

## 16. 回滚策略

- 供应链：移除对应 CI 步骤和配置文件，不影响应用运行时。
- 迁移 runner：保留已发布迁移历史，不删除或倒退用户数据库；通过兼容代码回滚应用实现。
- Zod：逐边界恢复原调用适配器，不修改生成绑定和后端接口。
- 性质测试：测试依赖和测试文件可独立回滚，不影响生产代码。
- fuzz：移除 fuzz workspace/target，同时保留已经转化的回归测试。

禁止通过删除用户数据库、覆盖迁移历史或使用 `git reset --hard` 回滚。

## 17. 建议提交拆分

1. `docs(architecture): 增加第三方库接入第二期设计与计划`
2. `build(governance): 增加依赖与许可证检查`
3. `refactor(migration): 增加 DuckDB 版本化迁移执行器`
4. `feat(validation): 增加前端运行时契约校验`
5. `test(property): 增加 Rust 与前端性质测试`
6. `test(fuzz): 增加核心输入模糊测试目标`
7. `ci(quality): 接入依赖治理与质量检查`
8. `docs(verification): 记录第三方库接入第二期验证结果`

实际提交前必须再次确认 Git 范围；不得包含本地 backlog。
