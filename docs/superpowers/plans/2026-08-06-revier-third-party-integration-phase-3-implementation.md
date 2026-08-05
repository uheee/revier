# Revier 第三方库接入第三期实施计划

- 日期：2026-08-06
- 状态：已确认，待实施
- 设计文档：`docs/superpowers/specs/2026-08-06-revier-third-party-integration-phase-3-design.md`

## 1. 执行原则

1. 所有开发、文档和注释使用中文。
2. 行为变化先补测试，再按最小垂直切片实现。
3. 每个阶段独立提交，提交消息使用语义化格式。
4. 不提交 `docs/third-party-integration-backlog.md`。
5. 不提交本地 `system.toml`、日志、构建产物、benchmark 原始报告、fuzz corpus 或数据库。
6. B04 是本期正式实施项；其他评估类候选库只形成结论，不默认增加生产依赖。
7. Diff 算法替换、远程遥测、错误 DTO 扩展和 Store 公共接口破坏必须另行确认。
8. 新增依赖必须通过锁文件、漏洞、许可证和供应链检查。

## 2. 阶段总览

| 阶段 | 内容 | 主要产物 | 提交建议 |
| --- | --- | --- | --- |
| 0 | 基线核查 | 精确范围和基线结果 | 不单独提交 |
| 1 | 日志链路收敛 | 日志入口、脱敏规则、测试 | `refactor(logging)` |
| 2 | 错误源保留 | source 测试、DTO 兼容测试 | `refactor(error)` |
| 3 | 系统设置与系统通知 | `system.toml`、插件、开关、终态通知 | `feat(notification)` |
| 4 | Diff benchmark | Criterion 基准和评估报告 | `test(benchmark)` |
| 5 | VueUse 与 Pinia Colada 深化 | 浏览器 API 和普通查询收敛 | `refactor(renderer)` |
| 6 | 候选库评估 | 缓存、校验和路径评估报告 | `docs(evaluation)` |
| 7 | CI 复核与总验证 | 门禁调整和验证记录 | `ci(quality)`、`docs(verification)` |

## 3. 阶段零：基线核查

### 3.1 环境初始化

```powershell
fnm env --use-on-cd --shell powershell | Out-String | Invoke-Expression
node --version
pnpm --version
cargo --version
rustc --version
```

如果 `fnm` 不在 PATH，使用：

```powershell
& 'C:\Users\Snowind\AppData\Local\Microsoft\WinGet\Links\fnm.exe' env --use-on-cd --shell powershell | Out-String | Invoke-Expression
```

### 3.2 核查命令

```powershell
git status --short --untracked-files=all
rg -n "eprintln!|console\.error|console\.warn|console\.debug" src-tauri src crates tests
rg -n "source\(|to_string\(\)|map_err\(|AppError" crates\revier-analysis src-tauri\src
rg -n "addEventListener|removeEventListener|setInterval|setTimeout|localStorage" src\renderer
rg -n "useQuery|@pinia/colada|projectQueryKeys" src\renderer
rg -n "criterion|imara-diff" crates\revier-analysis
rg -n "notification|system\.toml" src src-tauri crates tests
```

### 3.3 基线输出

- 精确变更文件清单。
- 日志和错误链路剩余问题清单。
- 当前通知中心调用点和任务终态路径清单。
- Diff、VueUse 和 Pinia Colada 现状清单。
- 当前 CI 报告模式输出。

## 4. 阶段一：日志链路收敛

### 4.1 测试先行

补充 `src-tauri/src/services/logging.rs` 测试：

- 调试和发布默认级别。
- 1 MiB、5 文件轮转配置。
- 损坏配置降级。
- WebView 调试开启、发布关闭。
- 路径、邮箱和任意正文不进入日志 context。

补充 renderer logger 测试：

- Tauri 环境写入插件。
- 非 Tauri 测试环境使用 console 兜底。
- context 过滤 `undefined` 和敏感字段。

### 4.2 实现

- 将运行期 Rust 诊断输出统一改为 `log::*`。
- 将 `src-tauri/src/lib.rs` 启动期 stderr 兜底集中封装。
- 前端业务异常统一通过 renderer logger。
- 测试性能输出改为受控 helper 或删除。

### 4.3 验证

```powershell
cargo test -p revier-tauri services::logging
pnpm test -- tests/unit/mainBootstrap.test.ts tests/unit/errors.test.ts
rg -n "eprintln!|console\.error" src-tauri src crates
```

### 4.4 提交

```text
refactor(logging): 收敛应用诊断日志链路
```

## 5. 阶段二：错误源保留与边界上下文

### 5.1 盘点范围

- `crates/revier-analysis/src/error.rs`
- `crates/revier-analysis/src/index/*`
- `crates/revier-analysis/src/cache/repository.rs`
- `src-tauri/src/error.rs`
- `src-tauri/src/services/projects.rs`
- `src-tauri/src/services/editor_settings.rs`

### 5.2 测试先行

- 分析库错误 source 链测试。
- DuckDB、I/O、JSON 和 gix 错误不被提前字符串化。
- Tauri DTO 的 `code/message/detail` 兼容测试。
- 发布构建 detail 不泄露完整路径和底层错误正文。

### 5.3 实现

- 对可保留底层错误的分支增加 `#[source]`。
- 应用边界使用 `anyhow::Context` 补充上下文。
- Tauri DTO 保持现有契约，不增加关联 ID。

### 5.4 验证

```powershell
cargo test -p revier-analysis error
cargo test -p revier-tauri error
pnpm test -- tests/unit/errors.test.ts
```

### 5.5 提交

```text
refactor(error): 保留边界错误源链
```

## 6. 阶段三：系统设置与系统通知

### 6.1 依赖接入

通过已初始化的 fnm 和 pnpm 环境执行：

```powershell
pnpm tauri add notification
```

核查自动变更，确保存在匹配的 Tauri 2 依赖：

- `src-tauri/Cargo.toml` 中的 `tauri-plugin-notification`。
- `package.json` 中的 `@tauri-apps/plugin-notification`。
- `src-tauri/src/lib.rs` 中的插件初始化。
- `src-tauri/capabilities/default.json` 中的最小权限。

自动命令如果加入插件完整默认权限，改为只保留：

```text
notification:allow-is-permission-granted
notification:allow-request-permission
notification:allow-notify
```

安装后立即执行：

```powershell
cargo deny check
pnpm audit:deps
```

### 6.2 Rust 合同与配置服务测试

先增加失败测试，覆盖：

- 缺少 `system.toml` 时排他创建默认文件。
- 默认值为 `version = 1` 和 `notifications.system.enabled = false`。
- 完整有效配置可加载。
- 非 UTF-8、未知版本、缺失字段和错误类型整体降级。
- 损坏配置保留原文件并返回 warning 和不可写状态。
- 原子写成功后更新快照。
- 原子写失败时内存快照保持不变。
- 并发更新串行化，不产生部分 TOML。

### 6.3 Rust 实现

新增：

- `SystemSettings`、`SystemNotificationSettings` 和 `SystemSettingsSnapshot` 合同。
- `src-tauri/src/services/system_settings.rs`。
- `src-tauri/src/commands/system_settings.rs`。
- `AppState.system_settings`。
- `system_settings_get`。
- `system_notifications_set_enabled`。

配置格式：

```toml
version = 1

[notifications.system]
enabled = false
```

持久化必须使用 `atomic-write-file`。只有原子替换成功后才更新服务内存状态。

### 6.4 绑定与前端边界

```powershell
pnpm generate:bindings
```

更新：

- `src/renderer/api/revierClient.ts` 的系统设置接口。
- `src/renderer/contracts/runtime/schemas.ts` 的系统设置快照校验。
- `src/renderer/generated/bindings.ts` 的生成结果。

运行时边界必须拒绝非法版本、缺失字段和非布尔通知开关。

### 6.5 前端系统通知测试

新增测试覆盖：

- 初始化只读取一次 `SystemSettingsSnapshot`。
- 默认关闭且不主动请求权限。
- 开启时先检查权限，再按需请求权限，授权后才写配置。
- 权限拒绝时不写 `enabled = true`。
- 关闭时写入 `enabled = false`。
- 配置 warning 时开关不可写，应用内警告只发布一次。
- 权限被系统撤销时保留配置意愿，但有效状态关闭。
- 前台和聚焦窗口不发送系统通知。
- 后台实时任务完成和失败各发送一次。
- 取消、普通查询、缓存恢复和历史终态不发送。
- 相同 `taskId + status` 重复事件只发送一次。
- 发送失败只写脱敏 warn，不递归发布应用内错误。
- 通知正文不包含项目 ID、路径、分支、作者、提交和底层错误。

### 6.6 前端实现

新增 `src/renderer/composables/useSystemNotifications.ts`，作为唯一系统通知入口。它负责：

- 配置初始化和开关状态。
- 操作系统权限检查与请求。
- 窗口焦点和最小化判断。
- 实时任务状态转换记录。
- 有界会话去重。
- 脱敏通知构造和发送。
- 失败日志降级。

修改现有通知中心：

- 在面板顶部增加“系统通知”开关。
- 显示关闭、开启、需要授权和配置不可用状态。
- 不增加独立设置页。
- 不改变通知历史、未读、删除和清空语义。

在 Review 实时任务入口调用系统通知协调方法。直接恢复分支缓存或加载历史终态时不得调用。

### 6.7 自动验证

```powershell
cargo test -p revier-tauri system_settings
pnpm generate:bindings:check
pnpm test -- tests/unit/systemSettings.test.ts tests/unit/systemNotifications.test.ts tests/unit/notificationCenter.test.ts tests/unit/rendererReviewStore.test.ts tests/unit/reviewWorkspace.test.ts
pnpm typecheck
cargo deny check
pnpm audit:deps
git diff --check
```

### 6.8 Windows 安装包人工验收

```powershell
pnpm build
```

使用生成的 Windows 安装包完成阻塞验收：

1. 首次启动生成默认 `system.toml`，不主动申请通知权限。
2. 用户开启系统通知后才申请权限。
3. 前台任务完成或失败不显示系统通知。
4. 窗口最小化或失焦后，实时分析完成显示一次脱敏通知。
5. 窗口最小化或失焦后，实时分析失败显示一次脱敏通知。
6. 取消任务、恢复缓存和重复终态事件不显示通知。
7. 拒绝权限、撤销权限和损坏配置时应用仍可正常分析。

### 6.9 提交

```text
feat(notification): 增加可配置的系统任务通知
```

## 7. 阶段四：Diff Benchmark 与评估

### 7.1 基准数据集

- 小文件：10 到 100 行。
- 大文件：1 万行以上生成文本。
- 高重复行。
- 完全不同文本。
- 尾换行差异。
- 仓库内既有 fixture。

不得使用真实用户仓库。

### 7.2 实现与文档

- 补齐 `crates/revier-analysis/benches/line_diff.rs`。
- 增加当前算法与 `imara-diff` 对照。
- 保持生产代码不切换算法。
- 新增 `docs/superpowers/verification/2026-08-06-revier-line-diff-benchmark.md`。

### 7.3 验证

```powershell
cargo bench -p revier-analysis --bench line_diff
cargo test -p revier-analysis --test line_diff_contract --test line_diff_property
```

### 7.4 提交

```text
test(benchmark): 增加 Diff 算法基准评估
```

## 8. 阶段五：VueUse 与 Pinia Colada 深化

### 8.1 VueUse

- 盘点剩余手写浏览器 API。
- 保持存储 key、默认值、延迟和卸载行为不变。
- 不升级 VueUse 主版本。
- `system.toml` 设置不迁移到 localStorage。

验证：

```powershell
pnpm test -- tests/unit/reviewLayoutSizes.test.ts tests/unit/reviewLayoutResizer.test.ts
```

### 8.2 Pinia Colada

扩展范围：

- 分支列表查询。
- 作者列表查询。
- 分支缓存状态查询。

保留 Store 范围：

- 长任务和系统通知终态事实。
- Tauri 事件流。
- 请求取消。
- 文件归因和回退状态机。

验证：

```powershell
pnpm test -- tests/unit/rendererProjectStore.test.ts tests/unit/rendererReviewStore.test.ts tests/unit/reviewWorkspace.test.ts
pnpm typecheck
```

### 8.3 提交

```text
refactor(renderer): 深化普通查询与浏览器工具封装
```

## 9. 阶段六：候选库评估

新增：

```text
docs/superpowers/verification/2026-08-06-revier-third-party-integration-phase-3-evaluation.md
```

评估候选：

- `moka`
- `dashmap`
- `parking_lot`
- `lru`
- `garde`
- `validator`
- `directories`
- `path-clean`
- `dunce`
- `camino`

每项记录目标代码位置、当前问题、收益、兼容风险、测试要求和“接入、暂缓或拒绝”结论。`tauri-plugin-notification` 已进入阶段三，不再参与候选评估。

提交：

```text
docs(evaluation): 评估第三期候选库接入
```

## 10. 阶段七：CI 门禁复核与总验证

### 10.1 CI 调整

- 将 `pnpm audit:deps` 的 high 等级检查改为阻塞门禁。
- `cargo deny check` 暂时保持报告模式，记录新插件许可证和 duplicate warnings。
- `pnpm check:deps` 暂时保持报告模式。

### 10.2 完整验证

```powershell
cargo fmt --all -- --check
cargo test --workspace
cargo deny check
cargo +nightly fuzz build line_diff
cargo +nightly fuzz build text_decode
cargo +nightly fuzz build migration_manifest
pnpm generate:bindings:check
pnpm typecheck
pnpm test
pnpm lint
pnpm audit:deps
pnpm check:deps
pnpm build
git diff --check
git status --short --untracked-files=all
```

如果磁盘空间不足，可以执行：

```powershell
cargo clean
```

### 10.3 验证记录

新增：

```text
docs/superpowers/verification/2026-08-06-revier-third-party-integration-phase-3.md
```

记录：

- 各命令结果。
- Windows 安装包通知人工验收结果。
- 漏洞和许可证结果。
- 报告模式剩余 warning。
- 候选库结论。
- 未完成项和第四期触发条件。

### 10.4 提交

```text
ci(quality): 提升高危依赖审计门禁
docs(verification): 记录第三方库接入第三期验证结果
```

## 11. 预计变更范围

```text
docs/superpowers/specs/2026-08-06-revier-third-party-integration-phase-3-design.md
docs/superpowers/plans/2026-08-06-revier-third-party-integration-phase-3-implementation.md
docs/superpowers/verification/2026-08-06-revier-line-diff-benchmark.md
docs/superpowers/verification/2026-08-06-revier-third-party-integration-phase-3-evaluation.md
docs/superpowers/verification/2026-08-06-revier-third-party-integration-phase-3.md
crates/revier-analysis/src/contracts.rs
crates/revier-analysis/src/bindings.rs
crates/revier-analysis/src/error.rs
crates/revier-analysis/src/index/*
crates/revier-analysis/src/cache/repository.rs
crates/revier-analysis/benches/*
src-tauri/Cargo.toml
src-tauri/capabilities/default.json
src-tauri/src/lib.rs
src-tauri/src/error.rs
src-tauri/src/state.rs
src-tauri/src/commands/mod.rs
src-tauri/src/commands/system_settings.rs
src-tauri/src/services/mod.rs
src-tauri/src/services/logging.rs
src-tauri/src/services/system_settings.rs
src-tauri/src/services/review.rs
src-tauri/src/services/projects.rs
package.json
pnpm-lock.yaml
src/renderer/api/logger.ts
src/renderer/api/revierClient.ts
src/renderer/contracts/runtime/schemas.ts
src/renderer/generated/bindings.ts
src/renderer/composables/useNotifications.ts
src/renderer/composables/useSystemNotifications.ts
src/renderer/components/NotificationCenter.vue
src/renderer/queries/*
src/renderer/stores/*
src/renderer/pages/ReviewWorkspace.vue
tests/unit/*
.github/workflows/quality.yml
```

实施每个阶段前重新核查精确文件列表，不因预计范围修改无关文件。

## 12. 回滚策略

- 日志：恢复统一入口调用，不删除用户日志配置。
- 错误：保持 DTO 契约，按错误分支回滚 source 调整。
- 系统通知：先关闭通知触发入口，再移除 Capability、前端包和 Rust 插件；保留用户 `system.toml`，不删除本地配置。
- Benchmark：移除 bench 对照和评估文档，不影响生产算法。
- VueUse 和 Pinia Colada：按查询或 composable 独立回滚。
- CI：恢复第二期报告模式。

禁止通过删除用户数据、覆盖损坏配置、重写数据库迁移历史或 `git reset --hard` 回滚。
