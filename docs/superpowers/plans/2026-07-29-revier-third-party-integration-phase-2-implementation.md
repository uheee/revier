# Revier 成熟第三方库接入第二期实施计划

## 1. 计划状态

- 日期：2026-07-29
- 状态：待分阶段实施
- 设计依据：`docs/superpowers/specs/2026-07-29-revier-third-party-integration-phase-2-design.md`
- 一期验证：`docs/superpowers/verification/2026-07-28-revier-third-party-integration-phase-1.md`
- 本地待办：`docs/third-party-integration-backlog.md`，不得加入 Git

## 2. 实施目标

本计划只实施以下四项：

1. B07：使用 `cargo-deny + knip + pnpm audit` 建立依赖、许可证和供应链治理。
2. B02：保留自有 DuckDB runner，使用 `include_dir + sha2` 建立版本化 SQL、checksum 和迁移历史。
3. B03：使用 `zod` 校验前端 IPC、配置和持久化数据边界。
4. B06：使用 `proptest + fast-check + cargo-fuzz` 增加生成式和模糊测试。

不实施 ORM、系统通知、遥测、Diff 生产替换、Pinia Colada 扩面或 `moka/lru` 生产接入。

## 3. 强制执行规则

1. 所有代码、文档和注释使用中文。
2. 使用 TDD：先增加失败测试，再写最小实现。
3. 不得为了适配实现擅自修改既有测试业务语义。
4. 任何既有测试期望变化必须先提交差异并取得书面确认。
5. Rust 依赖使用 `cargo add`，Node 依赖使用 `fnm + pnpm`。
6. 不使用 npm、yarn 或 `uv pip`。
7. 不自动执行 `pnpm audit --fix`。
8. 不自动删除 Knip 报告的依赖。
9. 不修改 Specta 生成文件来加入运行时校验。
10. 不以真实用户数据库、仓库源码或项目配置作为测试/fuzz 输入。
11. 每个阶段完成后提交变更、测试结果和风险供审核，确认后再进入下一阶段。
12. 未获得 Git 提交授权前，只修改工作区，不创建提交。

## 4. 阶段总览

| 阶段 | 内容 | 主要产物 | 确认节点 |
| --- | --- | --- | --- |
| 0 | 基线与精确决策 | 扫描报告、边界清单、文件清单 | 是 |
| 1 | 供应链治理 | deny 配置、Knip 配置、脚本 | 是 |
| 2 | DuckDB 迁移 runner | SQL 目录、checksum、历史表 | 是 |
| 3 | Zod 运行时校验 | schema、parser、错误映射 | 是 |
| 4 | 性质测试 | proptest、fast-check | 是 |
| 5 | 模糊测试 | cargo-fuzz targets | 是 |
| 6 | CI 与全量验收 | workflow、验证记录 | 是 |

## 5. 阶段零：基线与精确决策

### 5.1 工作区确认

运行：

```powershell
git status --short --untracked-files=all
git diff --check
git log -10 --oneline
```

确认：

- 一期提交均存在。
- 工作区中只有已知用户改动。
- `docs/third-party-integration-backlog.md` 仍为未跟踪文件。
- 本期不得覆盖任何用户未提交修改。

### 5.2 工具链确认

运行：

```powershell
rustc --version
cargo --version
fnm current
fnm exec --using-file node --version
fnm exec --using-file pnpm.CMD --version
```

记录：

- Rust stable 版本。
- Node 与 pnpm 版本。
- 操作系统和目标架构。
- `cargo-fuzz` 所需 nightly 是否已安装。

工具缺失时只报告，不在未确认时全局安装。

### 5.3 供应链首次扫描

不修改依赖，先形成事实：

```powershell
cargo tree --workspace --duplicates
fnm exec --using-file pnpm.CMD list --depth 0
rg -n '@vueuse/components|from.+diff|from.+minimatch' src tests scripts
```

若本机已具备工具，再运行非修改型扫描：

```powershell
cargo deny check
fnm exec --using-file pnpm.CMD knip
fnm exec --using-file pnpm.CMD audit
```

输出表格：

| 工具 | 当前告警 | 是否真实问题 | 建议规则 | 是否阻塞 |
| --- | ---: | --- | --- | --- |
| cargo-deny | 待扫描 | 待判断 | 待确认 | 否 |
| knip | 待扫描 | 待判断 | 待确认 | 否 |
| pnpm audit | 待扫描 | 待判断 | 待确认 | 否 |

### 5.4 DuckDB 基线

检查：

```powershell
Get-Content crates\revier-analysis\src\index\migrations.rs
Get-Content crates\revier-analysis\src\index\schema.rs
Get-Content crates\revier-analysis\tests\cache_schema.rs
Get-Content crates\revier-analysis\tests\index_schema.rs
```

形成清单：

- 当前 schema version。
- 支持的历史版本。
- 版本 1→2、2→3 的 SQL 与 Rust 调用。
- 新建数据库初始化路径。
- 迁移失败时的 rollback 行为。
- 读取数据库的所有入口。

提交建议的下一 schema version、baseline 行结构和现有数据库接管流程供确认。

### 5.5 运行时边界盘点

运行：

```powershell
rg -n '#\[tauri::command\]' src-tauri\src
rg -n 'listen\(|invoke\(' src\renderer
rg -n 'serde_json::from|toml::from|JSON\.parse' crates src-tauri src\renderer
```

按风险分级：

| 等级 | 边界 | 第二期处理 |
| --- | --- | --- |
| P0 | 项目配置、编辑器配置、操作进度事件 | 必须 |
| P1 | 项目列表、分支缓存状态等普通 IPC | 建议 |
| P2 | 大型 review 结果和长任务内部状态 | 默认不纳入 |

提交第一批精确命令、事件、字段和 parser 文件列表供确认。

### 5.6 性质测试边界盘点

列出当前示例测试已经覆盖与未覆盖的不变量：

- Diff 两侧重建。
- 块范围单调性。
- 编码解码及二进制识别。
- 缓存序列化往返。
- 前端迟到结果和终态不可回退。
- 迁移文件排序、版本解析和 checksum。

提交每个生成器的输入上限、case 数和性能预算供确认。

### 5.7 阶段零确认

进入阶段一前必须书面确认：

- 许可证 allowlist 和禁止来源。
- Audit 阻塞等级。
- Knip 忽略项和拟删除依赖。
- 迁移下一版本及 baseline 结构。
- Zod 第一批边界。
- Proptest/Fast-check case 数。
- CI 触发范围与初始报告/门禁模式。

## 6. 阶段一：供应链治理

### 6.1 添加开发依赖

确认后计划执行：

```powershell
fnm exec --using-file pnpm.CMD add -D knip fast-check
```

`fast-check` 在阶段四使用，但可与第二期 Node 开发依赖一起加入。若要求按阶段拆分锁文件变更，则延后到阶段四。

`cargo-deny` 是本地/CI 工具，不加入应用 `Cargo.toml`。安装命令需单独获得外部工具安装确认。

### 6.2 先建立失败检查

在修改配置前保存首次扫描输出，选择一个已确认的真实问题证明检查能够失败，例如：

- 明确未使用的直接依赖。
- 不允许的依赖来源。
- 未在许可证 allowlist 中的许可证。

不得通过人为加入恶意或漏洞依赖制造测试。

### 6.3 配置 `cargo-deny`

计划新增：

```text
deny.toml
```

配置内容：

- `[advisories]`：安全公告处理。
- `[licenses]`：已确认 allowlist。
- `[bans]`：重复版本和明确禁止 crate。
- `[sources]`：允许 registry 和 Git 来源。

每一项例外必须有中文原因和上游跟踪信息。未知许可证不得静默允许。

验证：

```powershell
cargo deny check
```

### 6.4 配置 Knip

计划新增 Knip 配置文件，具体采用 `knip.json` 或 `knip.config.ts` 在阶段零确认。

配置入口至少覆盖：

- `src/renderer/main.ts`
- Vue SFC
- Vite 与 Vitest 配置
- Tauri CLI
- `scripts/*`
- 生成绑定入口

验证：

```powershell
fnm exec --using-file pnpm.CMD knip
```

对每个疑似未使用依赖记录：

| 依赖 | 搜索证据 | 动态使用可能 | 删除建议 | 回归范围 |
| --- | --- | --- | --- | --- |
| `@vueuse/components` | 待执行 | 待判断 | 待确认 | 前端构建 |
| `diff` | 待执行 | 待判断 | 待确认 | Diff UI |
| `minimatch` | 待执行 | 待判断 | 待确认 | 文件筛选 |

只有获得独立确认后，才使用：

```powershell
fnm exec --using-file pnpm.CMD remove <已确认依赖>
```

### 6.5 增加 package scripts

建议脚本：

```json
{
  "check:deps": "knip",
  "audit:deps": "pnpm audit --audit-level high"
}
```

具体脚本名称和 audit 等级以阶段零确认值为准。

### 6.6 定向验证

```powershell
cargo deny check
fnm exec --using-file pnpm.CMD check:deps
fnm exec --using-file pnpm.CMD audit:deps
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD test
fnm exec --using-file pnpm.CMD lint
```

阶段成果：

- 当前供应链报告。
- 已确认配置。
- 忽略项及理由。
- 依赖删除候选，不擅自删除。

审核通过后进入阶段二。

## 7. 阶段二：DuckDB 版本化迁移执行器

### 7.1 依赖变更

确认后计划执行：

```powershell
cargo add -p revier-analysis include_dir
cargo add -p revier-analysis sha2
```

检查依赖范围：

```powershell
cargo tree -p revier-analysis -i include_dir
cargo tree -p revier-analysis -i sha2
```

依赖只用于分析库迁移模块，不扩散到前端或 Tauri 契约。

### 7.2 先写迁移清单失败测试

新增测试覆盖：

- 文件名版本解析。
- 文件名不合法时拒绝启动 runner。
- 版本重复时拒绝。
- 版本间断时按确认规则拒绝。
- 迁移按数值版本而不是字符串排序。
- 相同字节产生相同 SHA-256。
- 任意字节变化导致 checksum 变化。

先运行并确认测试因 runner/解析器尚不存在而失败：

```powershell
cargo test -p revier-analysis migration_manifest
```

### 7.3 实现迁移描述模型

建议内部结构：

```rust
struct EmbeddedMigration {
    version: u32,
    name: String,
    sql: &'static [u8],
    checksum: String,
}
```

约束：

- 只在迁移模块内部公开。
- SQL 必须验证为 UTF-8 后再交给 DuckDB。
- checksum 对原始字节计算。
- 不在日志中输出完整 SQL。

### 7.4 先写历史表失败测试

使用临时 DuckDB 覆盖：

- 首次接管版本 3 数据库时创建历史表。
- baseline 行类型明确。
- 新建数据库历史一致。
- 重复执行不重复写入。
- 已应用版本 checksum 不同时报不兼容。
- 历史版本高于应用支持版本时报不兼容。

如果测试需要将当前 schema version 期望从 3 改为新版本，先提交具体测试差异供确认。

### 7.5 增加版本化 SQL

计划新增目录：

```text
crates/revier-analysis/migrations/
```

至少包含 runner 接管所需的下一版本 SQL。迁移文件：

- 使用明确版本号和英文蛇形名称。
- 只包含本版本 schema 变化。
- 不包含 `BEGIN`、`COMMIT`，事务由 runner 控制。
- 不使用依赖工作目录的外部文件命令。

### 7.6 实现事务 runner

最小实现顺序：

1. 读取 `metadata.schema_version`。
2. 兼容处理旧版本 1、2、3。
3. 加载并校验嵌入迁移清单。
4. 创建或读取 `schema_migrations`。
5. 校验已应用 checksum。
6. 对每个待执行迁移开启独立事务。
7. 执行 SQL。
8. 写入历史。
9. 更新 `metadata.schema_version`。
10. 提交；失败时回滚。

保留现有 `ensure_compatible_schema` 公共入口，避免修改所有调用方。

### 7.7 失败与恢复测试

覆盖：

- SQL 语法失败。
- 表或列冲突。
- 写历史失败。
- 更新 metadata 失败。
- commit 失败的可模拟路径。
- rollback 后旧 schema 和版本仍可读取。
- 再次启动可以重新尝试未完成迁移。

不得删除旧数据库或通过重建数据库掩盖迁移失败。

### 7.8 定向验证

```powershell
cargo fmt --all -- --check
cargo test -p revier-analysis --test cache_schema
cargo test -p revier-analysis --test index_schema
cargo test -p revier-analysis migration
cargo test -p revier-tauri
```

额外检查：

```powershell
rg -n 'begin transaction|schema_version|schema_migrations' crates\revier-analysis\src crates\revier-analysis\migrations
```

阶段成果：

- 迁移清单和 checksum 规则。
- 旧版本兼容报告。
- 失败回滚证据。
- 新建和已有数据库的历史示例。

审核通过后进入阶段三。

## 8. 阶段三：Zod 运行时契约校验

### 8.1 添加依赖

确认后计划执行：

```powershell
fnm exec --using-file pnpm.CMD add zod
```

检查：

```powershell
fnm exec --using-file pnpm.CMD list zod
```

### 8.2 建立边界目录

根据阶段零确认的文件清单新增：

```text
src/renderer/contracts/runtime/
```

建议分层：

- `schemas.ts` 或按领域拆分的 schema。
- `parseBoundary.ts`：统一 `safeParse` 和错误转换。
- `errors.ts`：稳定错误类别与安全摘要。

禁止：

- 修改 `src/renderer/generated/bindings.ts`。
- 在页面组件中重复写 `safeParse`。
- 将完整无效 payload 写入日志。

### 8.3 先写统一 parser 失败测试

覆盖：

- 合法 payload 返回已验证对象。
- 缺少必填字段返回 `CONTRACT_VALIDATION_FAILED`。
- 类型错误包含安全字段路径。
- 额外允许字段不导致失败。
- 错误日志不包含完整 payload。
- 同一次失败只发送一次通知。

运行：

```powershell
fnm exec --using-file pnpm.CMD test -- runtimeContract
```

### 8.4 项目与配置边界

先测试后实现：

- 项目列表合法对象。
- 项目 ID、路径、名称字段缺失或类型错误。
- `projects.json` 损坏后的稳定错误。
- 编辑器配置数值、枚举和可选字段。
- 旧版本允许字段缺失时的明确默认值。

Rust 后端仍负责文件解析和领域校验；Zod 负责前端收到的数据不污染状态。

### 8.5 普通 IPC 查询边界

按阶段零清单逐项接入：

1. 项目列表。
2. 分支缓存状态。
3. 其他已确认普通查询。

每个 parser 必须位于 query/store 入口，不在组件层重复验证。

测试：

- 合法响应行为不变。
- 无效响应不写入 Store。
- 迟到无效响应不会覆盖当前上下文。
- 查询错误只通知一次。

### 8.6 操作进度事件边界

测试：

- 合法运行态和终态事件。
- 未知状态、阶段或 operation kind。
- 缺失 operation ID。
- 进度越界、负耗时或字段类型错误。
- 无效事件被忽略并记录日志。
- 终态不会因迟到运行态事件回退。

只校验事件边界，不借此重写 review 状态机。

### 8.7 错误映射

统一错误至少提供：

```text
code: CONTRACT_VALIDATION_FAILED
message: 面向用户的稳定中文消息
source: 命令、事件、配置或持久化边界
issues: 安全的字段路径摘要
```

若需要把该错误加入共享 Specta 契约，必须先提交接口变化供确认；默认先作为前端边界错误处理。

### 8.8 定向验证

```powershell
fnm exec --using-file pnpm.CMD test -- runtimeContract projectHome rendererProjectStore rendererReviewStore
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD lint
```

静态检查：

```powershell
rg -n 'safeParse|CONTRACT_VALIDATION_FAILED' src\renderer
rg -n 'JSON\.parse|as Review|as Operation' src\renderer
```

剩余断言必须逐项说明，不追求机械清零。

阶段成果：

- 已覆盖边界矩阵。
- 合法/非法 payload 测试。
- 日志隐私检查。
- 未纳入边界及原因。

审核通过后进入阶段四。

## 9. 阶段四：Rust 与前端性质测试

### 9.1 Rust 开发依赖

确认后计划执行：

```powershell
cargo add -p revier-analysis --dev proptest
```

### 9.2 Diff 不变量

先定义生成器：

- 空文本。
- 有无尾随换行。
- 重复行。
- Unicode、组合字符和不同换行符。
- 局部增加、删除和替换。
- 两侧规模明显不对称。

性质：

- parts 可重建旧文本和新文本。
- Equal 部分两侧一致。
- 块范围不越界。
- 算法不 panic。

限制单例文本长度和 case 数，保留少量大输入到专用测试。

### 9.3 编码与二进制识别

生成：

- 任意字节。
- UTF-8、UTF-16 LE/BE。
- BOM 存在与缺失。
- NUL 和常见二进制片段。

性质：

- 探测和解码不 panic。
- 明确支持编码的编码/解码可往返。
- 无效输入返回稳定错误或二进制分类。
- 不产生未确认的替换字符语义变化。

### 9.4 缓存序列化

生成最小合法缓存模型，验证：

- JSON/数据库映射往返保持关键字段。
- 枚举和可选字段稳定。
- 范围字段符合领域约束。
- 无效范围不能静默成为合法对象。

不连接用户真实缓存数据库。

### 9.5 迁移清单性质

验证：

- 合法文件列表排序确定。
- 重复版本必然被拒绝。
- 任意文件顺序不影响最终迁移顺序。
- checksum 对相同字节稳定。

### 9.6 前端 `fast-check`

为以下状态序列建立 command/model 测试或等价性质测试：

- 项目和分支切换。
- 请求开始、成功、失败。
- 进度运行态、完成态、失败态、取消态。
- 旧 request ID 和新 request ID 交错。

性质：

- 当前上下文只接受匹配请求。
- 终态不可回退到运行态。
- 同一错误不重复通知。
- 新上下文不会显示旧项目数据。

### 9.7 失败样本固化

任何性质测试发现的真实缺陷：

1. 保存 seed 和最小输入。
2. 增加一个确定性回归测试。
3. 再修复生产代码。
4. 不仅通过增加过滤条件绕过失败输入。

### 9.8 定向验证

```powershell
cargo test -p revier-analysis
fnm exec --using-file pnpm.CMD test -- property
fnm exec --using-file pnpm.CMD typecheck
```

记录每组：

- case 数。
- seed 策略。
- 最长输入。
- 本机耗时。
- 已固化失败样本。

审核通过后进入阶段五。

## 10. 阶段五：基础 `cargo-fuzz` 目标

### 10.1 工具确认

安装或使用 nightly 前必须单独确认。计划命令：

```powershell
cargo +nightly fuzz --version
```

如果环境缺失，只报告所需安装步骤，不绕过 `cargo-fuzz` 改用自制随机循环。

### 10.2 初始化范围

计划新增标准 fuzz 目录，但必须检查初始化结果，避免自动修改 workspace 中无关配置。

首批 target：

1. `line_diff`
2. `text_decode`
3. `migration_manifest`

每个 target：

- 只接受内存字节。
- 设置明确输入大小上限。
- 不访问网络。
- 不读取用户目录。
- 不写生产缓存。

### 10.3 短时验证

在获准环境中进行短时 smoke run，例如：

```powershell
cargo +nightly fuzz run line_diff -- -max_total_time=30
cargo +nightly fuzz run text_decode -- -max_total_time=30
cargo +nightly fuzz run migration_manifest -- -max_total_time=30
```

实际运行时间在阶段确认时确定，不执行超过沟通窗口的阻塞命令。

### 10.4 Git 忽略

检查并按需更新 `.gitignore`：

- fuzz target 构建目录。
- corpus。
- artifacts/crashes。
- coverage 输出。

不要忽略 fuzz target 源码和必要种子夹具。

### 10.5 缺陷处理

发现崩溃时：

1. 保存最小化输入。
2. 检查是否包含敏感内容。
3. 转换为普通回归测试。
4. 先提交缺陷及预期行为供确认。
5. 修复后重跑 target。

阶段成果：

- 可构建的 fuzz targets。
- 短时运行结果。
- corpus/artifact 管理说明。
- 未解决崩溃清单。

审核通过后进入阶段六。

## 11. 阶段六：CI、全量回归与验证记录

### 11.1 CI 工作流

根据阶段零确认，新增或调整质量工作流。

建议触发：

- Pull request。
- 主分支 push。
- 手工触发。

建议任务：

```text
Rust supply chain: cargo deny check
Node dependency usage: pnpm check:deps
Node advisories: pnpm audit:deps
Existing checks: fmt/test/typecheck/lint
Optional fuzz build: 仅构建，不长跑
```

首次可采用报告模式；切换阻塞门禁必须独立确认。

### 11.2 全量 Rust 验证

```powershell
cargo fmt --all -- --check
$env:RUSTFLAGS='-C debuginfo=0'
cargo test --workspace
cargo deny check
```

若需要 Clippy，按仓库既有约束或独立确认运行：

```powershell
cargo clippy --workspace --all-targets --all-features -- -D warnings
```

### 11.3 全量 Node 验证

```powershell
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD test
fnm exec --using-file pnpm.CMD lint
fnm exec --using-file pnpm.CMD check:deps
fnm exec --using-file pnpm.CMD audit:deps
fnm exec --using-file pnpm.CMD generate:bindings:check
```

### 11.4 构建验证

```powershell
fnm exec --using-file pnpm.CMD build
```

若构建需要网络、签名或平台依赖，先报告并按权限机制申请，不降低构建检查。

### 11.5 静态检查

```powershell
rg -n 'include_dir|Sha256|schema_migrations' crates\revier-analysis
rg -n 'safeParse|CONTRACT_VALIDATION_FAILED' src\renderer
rg -n 'proptest|fast-check' crates tests src
rg -n 'console\.error|eprintln!' crates src-tauri src\renderer
git diff --check
```

剩余日志兜底和测试输出按一期规则逐项说明。

### 11.6 新增验证记录

计划新增：

```text
docs/superpowers/verification/2026-07-29-revier-third-party-integration-phase-2.md
```

内容：

- 实际新增依赖和版本。
- 供应链规则和当前例外。
- 删除或保留的依赖及原因。
- 迁移版本、SQL 文件和 checksum 规则。
- 旧数据库兼容测试。
- Zod 覆盖边界和未覆盖边界。
- Proptest/Fast-check case 数和 seed。
- Cargo-fuzz targets 和短时运行结果。
- CI 触发和门禁策略。
- 全量验证命令与结果。
- 已知限制及第三期候选。

### 11.7 Git 范围核查

```powershell
git status --short --untracked-files=all
git diff --check
git diff --stat
git ls-files --error-unmatch docs/third-party-integration-backlog.md
```

最后一条命令应失败，以证明本地 backlog 未被跟踪。

额外确认没有提交：

- `fuzz/artifacts`
- 大体积 corpus
- 审计缓存
- 本地日志
- DuckDB 临时数据库
- Criterion 报告

## 12. 阶段验收矩阵

| 阶段 | 必须通过 | 不允许发生 |
| --- | --- | --- |
| 0 | 基线、边界和规则获得确认 | 未确认即安装或改依赖 |
| 1 | 三类供应链检查可重复运行 | 自动修复或自动删除依赖 |
| 2 | 迁移幂等、回滚、checksum、旧库兼容 | 重建或删除旧数据库 |
| 3 | 无效 payload 不进入业务状态 | 修改生成绑定或记录完整 payload |
| 4 | 性质测试可复现且时间受控 | 用过滤器掩盖真实失败 |
| 5 | fuzz target 可构建并短时运行 | 提交敏感 corpus 或大体积产物 |
| 6 | 全量测试、检查、构建和 Git 范围通过 | backlog 或构建产物进入提交 |

## 13. 预计变更范围

以下仅为计划清单，阶段零后需提交精确文件列表：

```text
Cargo.toml
Cargo.lock
deny.toml
package.json
pnpm-lock.yaml
knip 配置
.github/workflows/*
.gitignore
crates/revier-analysis/Cargo.toml
crates/revier-analysis/migrations/*
crates/revier-analysis/src/index/migrations.rs
crates/revier-analysis/src/index/schema.rs
crates/revier-analysis/tests/*
src/renderer/contracts/runtime/*
src/renderer/queries/*
src/renderer/stores/*
tests/unit/*
fuzz/*
docs/superpowers/verification/*
```

不得因为该预计清单而默认修改所有文件。

## 14. 回滚步骤

### 14.1 供应链

- 移除质量工作流中的对应步骤。
- 移除 Knip 脚本和配置。
- 保留已确认的依赖修正；若需恢复依赖，使用明确补丁和包管理器。

### 14.2 迁移

- 不删除 `schema_migrations`。
- 不降低用户数据库 schema version。
- 通过兼容 runner 继续识别已经应用的 checksum。
- 如需停止发布，回滚应用构建，不篡改数据库。

### 14.3 Zod

- 按边界逐项恢复原调用路径。
- 保持后端命令和生成绑定不变。
- 不移除用于证明损坏 payload 行为的测试，除非获得测试变更确认。

### 14.4 性质测试和 fuzz

- 可独立移除测试依赖和 target。
- 已发现缺陷的确定性回归测试继续保留。
- 删除构建产物时必须验证路径位于工作区目标目录，禁止宽泛递归删除。

## 15. 建议语义化提交

按以下顺序拆分：

1. `docs(architecture): 增加第三方库接入第二期设计与计划`
2. `build(governance): 增加依赖与许可证检查`
3. `refactor(migration): 增加 DuckDB 版本化迁移执行器`
4. `feat(validation): 增加前端运行时契约校验`
5. `test(property): 增加 Rust 与前端性质测试`
6. `test(fuzz): 增加核心输入模糊测试目标`
7. `ci(quality): 接入依赖治理与质量检查`
8. `docs(verification): 记录第三方库接入第二期验证结果`

每次提交前：

```powershell
git status --short
git diff --check
git diff --stat
```

只暂存本阶段文件，明确排除：

```text
docs/third-party-integration-backlog.md
```

## 16. 最终完成定义

第二期只有同时满足以下条件才算完成：

- B02、B03、B06、B07 均完成已确认范围。
- 供应链规则经过基线审核并进入确认的 CI 模式。
- DuckDB 迁移有版本化 SQL、checksum、历史、事务和旧库兼容测试。
- 关键运行时边界有 Zod 校验，错误不污染状态且不泄漏 payload。
- Proptest、Fast-check 和 Cargo-fuzz 均有可运行的首批目标。
- 现有 Rust 与前端全量测试、类型检查、lint 和构建通过。
- 形成第二期验证记录。
- 所有变更均按确认节点审核。
- 本地 backlog 始终未加入 Git。
