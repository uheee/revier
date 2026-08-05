# Revier 第三方库接入第三期设计

- 日期：2026-08-06
- 状态：已确认，待实施
- 依据：
  - `docs/third-party-integration-backlog.md`
  - `docs/superpowers/verification/2026-07-29-revier-third-party-integration-phase-2.md`
- 相关计划：
  - `docs/superpowers/plans/2026-08-06-revier-third-party-integration-phase-3-implementation.md`

## 1. 背景

第二期已经完成供应链治理、DuckDB 版本化迁移、前端运行时契约校验、性质测试、基础 fuzz target 和质量 CI。第三期不重复这些基础设施建设，重点转为诊断链路收敛、现有试点深化、候选库评估和系统通知闭环。

第三期对原计划做一项明确调整：TODO-B04 不再只做调研，而是正式接入 `tauri-plugin-notification`，并新增 `system.toml` 管理系统级设置。远程遥测仍不在本期范围内。

## 2. Backlog 范围

### 2.1 第二期已完成或基本完成

以下条目不再作为第三期主开发项：

- TODO-C03：项目配置文件原子写入。当前已使用 `atomic-write-file`。
- TODO-B02：DuckDB 迁移执行器增强。第二期已落地 migration manifest、checksum 和 `schema_migrations`。
- TODO-B03：运行时契约校验。第二期已落地 Zod 边界校验。
- TODO-B06：性质测试、模糊测试和前端生成测试。第二期已落地 proptest、fast-check 和 cargo-fuzz。
- TODO-B07：依赖、许可证和供应链治理。第二期已落地 cargo-deny、Knip、pnpm audit 和质量 CI。

### 2.2 第三期实施

- TODO-C01：统一 Rust、Tauri 与前端日志链路。
- TODO-C02：保留错误源并补充应用边界上下文。
- TODO-B04：接入可配置、可降级的系统通知。
- TODO-C04：建立 Diff 算法基准并评估 `imara-diff`。
- TODO-C05：使用现有 VueUse 收敛浏览器工具代码。
- TODO-C06：使用 Pinia Colada 扩展普通异步查询。
- TODO-C07：评估缓存、校验和路径库。

### 2.3 第三期延后

- TODO-B05：指标和遥测。保留本地日志与 Benchmark，不启用远程遥测或自动上报。
- TODO-B01：DuckDB ORM 复评。当前触发条件未满足，继续延后。

## 3. 目标

1. 让本地诊断信息进入统一日志链路，并默认避免源码正文、完整 payload、作者邮箱等敏感内容。
2. 保留底层错误 `source` 链，同时维持前端稳定错误码和用户可见消息。
3. 通过 `system.toml` 提供系统设置入口，完成后台分析任务终态通知闭环。
4. 形成可重复的 Diff benchmark 与 `imara-diff` 评估报告。
5. 补齐 VueUse 和 Pinia Colada 试点边界，减少手写异步状态和浏览器 API 管理代码。
6. 对缓存、校验和路径候选库形成“接入、暂缓或拒绝”的明确结论。
7. 基于第二期 CI 结果复核报告模式与阻塞门禁。

## 4. 非目标

- 不引入远程错误采集、远程遥测或自动上报。
- 不替换生产 Diff 算法，除非 benchmark 和兼容性报告另行确认。
- 不迁移 DuckDB 到其他数据库或 ORM。
- 不删除 Knip 发现的依赖，除非有独立确认和回归结果。
- 不改变前端公开错误码、项目 JSON 契约或现有长任务取消语义。
- 不为系统通知增加自定义点击导航、通知动作、附件、声音或通知渠道管理。
- 不监听 `system.toml` 的外部文件变化。

## 5. 总体架构

第三期保持现有 Tauri、Vue、Pinia 和分析库边界。系统设置和系统通知分别承担持久化与运行期投递职责：

```text
<app_config_dir>/system.toml
            |
            v
SystemSettingsService -- system_settings_get ------------------+
            ^                                                  |
            | system_notifications_set_enabled                 v
通知中心开关 <--------------------------------------- useSystemNotifications
                                                               |
实时任务 pending/running -> completed/failed                   |
            |                                                  |
            +--> 终态判定 -> 焦点判定 -> 会话去重 -> 权限判定 --+
                                                               |
                                                               v
                                                tauri-plugin-notification
```

应用内通知中心始终独立可用。系统通知是附加投递通道，失败时不得影响任务状态、应用内通知或后续分析。

## 6. 技术设计

### 6.1 日志链路收敛

现状：

- `tauri-plugin-log` 和 `log` 已接入。
- `src-tauri/src/services/logging.rs` 已负责配置加载和插件构建。
- `src/renderer/api/logger.ts` 已封装前端日志入口。
- Tauri 启动期仍有零散 `eprintln!`，部分测试仍直接输出性能诊断。

设计：

- 保留 `tauri-plugin-log + log` 作为主方案。
- 启动期在日志插件可用前允许 stderr 兜底，但集中到单一入口。
- 运行期 Rust 代码统一使用 `log::{error,warn,info,debug,trace}`。
- 前端业务异常统一通过 renderer logger；非 Tauri 测试环境保留 console 兜底。
- 发布构建默认 `warn`，调试构建默认 `debug`。
- 文件轮转使用 1 MiB，保留 5 个文件。
- WebView 输出在调试构建开启，在发布构建关闭。
- context 只记录稳定标识和非敏感统计值，例如 `task_id`、`operation_id`、阶段、耗时和数量。

验收：

- 非测试运行路径中没有零散 `eprintln!`。
- 前端业务代码不直接调用 `console.error` 记录运行期异常。
- 配置损坏时应用仍可启动，并通过安全降级入口报告问题。

### 6.2 错误源与边界上下文

- 分析库公共错误继续使用 `thiserror`。
- DuckDB、I/O、JSON 和 gix 错误在可保留 `source` 时不得提前转为纯字符串。
- Tauri 启动和组合边界使用 `anyhow::Context` 补充上下文，不吞掉原始错误。
- Tauri IPC 继续返回稳定的 `code/message/detail`。
- 第三期不扩展错误 DTO 关联 ID；诊断关联通过日志 context 完成。
- 发布构建的 `detail` 只包含安全摘要，完整 source 链只进入本地日志。

验收：

- 关键底层错误可以通过 `std::error::Error::source` 追踪。
- 前端错误码和用户可见消息保持兼容。
- source 保留和 DTO 稳定性有测试覆盖。

### 6.3 `system.toml` 与系统设置服务

配置文件位于 `<app_config_dir>/system.toml`，初始格式固定为：

```toml
version = 1

[notifications.system]
enabled = false
```

约束：

- `version` 必须为 `1`。
- `notifications.system.enabled` 必须存在且为布尔值。
- 第三期不开放事件列表、通知正文模板和隐私字段配置。
- 首次启动缺少文件时，使用排他创建写入默认配置，不覆盖并发创建的文件。
- 配置必须是 UTF-8，并经过 TOML 解析和完整字段校验。

新增 `SystemSettingsService`，职责限定为：

- 加载、校验和快照化 `system.toml`。
- 在文件缺失时生成默认配置。
- 使用 `atomic-write-file` 原子保存已验证的完整配置。
- 写入成功后才更新内存快照。
- 暴露配置路径、当前设置、warning 和可写状态。

配置损坏时：

- 保留原文件，不自动覆盖或修复。
- 运行时降级为 `enabled = false`。
- 通过应用内通知中心显示一次警告。
- 系统通知开关显示不可写状态。
- 外部修复文件并重启后恢复，不增加文件 watcher。

IPC 接口：

```text
system_settings_get() -> SystemSettingsSnapshot
system_notifications_set_enabled(enabled: bool) -> SystemSettingsSnapshot
```

`SystemSettingsSnapshot` 进入现有 Rust 到 TypeScript 绑定生成流程。设置命令失败时返回稳定 `AppError`，不得只返回字符串。

### 6.4 系统通知投递

依赖与权限：

- 使用与当前 Tauri 2 兼容且通过供应链检查的 `tauri-plugin-notification` 2.x。
- 前端使用匹配的 `@tauri-apps/plugin-notification` 2.x。
- 锁文件固定实际解析版本。
- Capability 只开放检查权限、请求权限和发送通知，不使用插件完整默认权限。

前端新增 `useSystemNotifications` 作为系统通知的单一入口，负责：

- 初始化 `SystemSettingsSnapshot`。
- 管理用户配置意愿、操作系统权限和有效启用状态。
- 处理通知中心开关。
- 判断当前窗口是否聚焦或最小化。
- 识别实时分析任务的终态转换。
- 以 `taskId + status` 做有界会话去重。
- 构造脱敏通知并调用插件。
- 将权限或发送失败写入统一日志。

开关流程：

1. 默认关闭，不在应用启动时主动请求权限。
2. 用户主动开启时先检查操作系统权限。
3. 未授权时调用 `requestPermission()`。
4. 只有授权成功后才通过 IPC 写入 `enabled = true`。
5. 用户关闭时直接写入 `enabled = false`。
6. 如果权限之后被系统撤销，保留配置意愿，但有效状态为关闭，界面显示“需要系统授权”。

通知触发条件必须同时满足：

- `system.toml` 中用户意愿为开启。
- 操作系统权限已授予。
- 当前是本次运行中的实时任务，而不是缓存恢复或历史快照。
- 任务确实由 `pending/running` 转为 `completed/failed`。
- 主窗口未聚焦或已最小化。
- 当前 `taskId + status` 尚未发送。

以下情况不发送系统通知：

- 任务取消。
- 普通查询、元数据加载、文件打开和归因操作。
- 应用启动后的历史终态或分支缓存恢复。
- 主窗口处于前台。
- 配置损坏、权限拒绝或插件不可用。

通知内容：

- 标题仅使用“分析完成”或“分析失败”。
- 正文只显示安全统计，例如耗时和变更文件数量。
- 不显示项目名称、仓库路径、分支名、作者、邮箱、提交、源码、完整错误或 IPC payload。
- 失败通知不得包含底层错误详情。

应用内通知中心与系统通知共用任务终态事实，但保持独立展示。系统通知失败只写脱敏 `warn`，不得再生成一条应用内错误通知，避免递归和重复提示。

### 6.5 Diff Benchmark 与 `imara-diff` 评估

- 基准数据集覆盖小文件、大文件、高重复行、完全不同文本、尾换行差异和仓库内 fixture。
- 数据集不得来自真实用户仓库。
- 对比当前实现和 `imara-diff` 的耗时、内存倾向和语义差异。
- 第三期只输出评估文档，不切换生产算法。
- 如果建议替换，必须另起独立阶段并确认兼容性方案。

验收：

- `cargo bench -p revier-analysis --bench line_diff` 可重复运行。
- 评估报告记录数据集、结果、语义差异和建议。
- 生产 Diff 合同测试保持原语义。

### 6.6 VueUse 收敛

- 盘点剩余手写监听器、计时器、防抖和 localStorage 访问。
- 只在键名、默认值、延迟时间和卸载行为不变时替换。
- 不升级 VueUse 主版本。
- `system.toml` 不使用 localStorage，因此系统通知设置不属于 VueUse 存储迁移范围。

验收：

- 组件卸载后监听器和计时器被释放。
- 布局和设置持久化键不变。
- 现有交互测试全部通过。

### 6.7 Pinia Colada 普通查询扩展

- 扩展到分支列表、作者列表和分支缓存状态。
- 长任务分析、Tauri 事件流、取消、文件归因和回退状态机继续保留在 Store。
- 查询错误副作用集中处理，避免重复通知。
- 保持 Store 对组件暴露的公共方法兼容。

验收：

- 迟到普通查询不会覆盖新上下文。
- 查询失效时机有测试覆盖。
- 长任务状态、进度事件和系统通知终态判断不受普通查询迁移影响。

### 6.8 候选库评估

第三期只评估以下候选，不默认增加生产依赖：

- 缓存：`moka`、`dashmap`、`parking_lot`、`lru`。
- 校验：`garde`、`validator`。
- 目录：`directories`。
- 路径：`path-clean`、`dunce`、`camino`。

每个候选必须记录目标代码位置、当前问题、收益、兼容风险、测试要求和“接入、暂缓或拒绝”结论。系统通知已进入实施范围，不再列入候选评估。

## 7. 数据与隐私

- 日志不得记录源码正文、完整 diff 或完整 IPC payload。
- 仓库路径默认只记录规范化项目 ID 或哈希，不记录完整路径。
- 作者邮箱默认不记录；如需关联，只允许不可逆哈希或隐藏用户名。
- 系统通知不得显示任何仓库身份信息或底层错误详情。
- `system.toml` 只保存本机功能偏好，不保存仓库、账号或遥测数据。
- B05 远程遥测必须另行设计并取得明确确认。

## 8. CI 与质量策略

- 保留第二期 `Quality` workflow。
- `pnpm audit:deps` 的 high 等级升级为阻塞门禁。
- `cargo deny check` 暂时保持报告模式；依据 duplicate warnings 和新插件许可证结果复核。
- `pnpm check:deps` 暂时保持报告模式，直到依赖删除项独立确认。
- 新通知依赖必须通过 Cargo、pnpm 和许可证检查。
- Windows 安装包中的系统通知完成和失败路径属于阻塞验收。
- macOS 和 Linux 本期要求单元测试和可用环境中的编译检查，不要求人工通知验收。

## 9. 已确认决策

1. B04 在第三期正式接入，不再只做调研。
2. 新增 `system.toml`，系统通知配置归入其中。
3. 系统通知默认关闭，只在用户主动开启时请求权限。
4. 只通知后台实时分析任务的完成和失败状态。
5. 不设置最低耗时阈值，不通知取消和普通查询。
6. 历史终态和缓存恢复不发送系统通知。
7. 第三期不实现自定义点击导航。
8. B05 继续保持本地日志和 Benchmark，不启用远程遥测。
9. Diff 生产算法、错误 DTO 和 Store 公共接口在第三期保持兼容。
10. Windows 安装包通知属于阻塞验收。

## 10. 验收标准

- 日志和错误链路有统一入口、脱敏规则和测试覆盖。
- `system.toml` 可安全创建、读取、校验和原子更新。
- 配置损坏、权限拒绝和插件失败均可安全降级。
- 应用内通知中心与系统通知不会重复弹出同一错误。
- 后台实时分析完成或失败时，Windows 安装包能按配置发送脱敏系统通知。
- Diff benchmark 可重复运行，并形成评估报告。
- VueUse 与 Pinia Colada 完成已确认范围且保持现有行为。
- 候选库评估文档有明确结论。
- CI 门禁策略有第三期验证记录。
- 不提交 `docs/third-party-integration-backlog.md`、构建产物、日志、benchmark 原始大文件、本地配置或 fuzz corpus。
