# Revier Monaco 编辑器最终验收记录

**验收日期：** 2026-07-19  
**设计文档：** [2026-07-16-revier-monaco-editor-design.md](../specs/2026-07-16-revier-monaco-editor-design.md)  
**视觉稿：** [2026-07-16-revier-monaco-editor-visual.html](../specs/2026-07-16-revier-monaco-editor-visual.html)  
**实施计划：** [2026-07-16-revier-monaco-editor-implementation.md](../plans/2026-07-16-revier-monaco-editor-implementation.md)

## 结论

Monaco 编辑器、Shiki 高亮、TOML 设置、外置 AuthorRail、临时草稿、编码原子重载、通知中心及统一浅深色视觉均已按确认方案落地。最终自动化、静态审计和真实 Tauri 交互验收通过。

## 自动化验证

| 命令 | 结果 |
| --- | --- |
| `cargo fmt --all -- --check` | 通过 |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | 通过，0 告警 |
| `cargo test --workspace --all-features` | 通过，161 项测试，0 失败 |
| `pnpm test` | 通过，30 个测试文件、177 项测试 |
| `pnpm typecheck` | 通过 |
| `pnpm lint` | 通过 |
| `pnpm generate:bindings:check` | 通过，生成后无差异 |
| `pnpm vite:build` | 通过 |
| `git diff --check` | 通过 |

生产构建仍会提示 Monaco 主包和 Worker 分块大于 500 kB；这是非阻塞体积提示，不影响本次功能与验收结果。

## 真实 Tauri 验收

通过 `pnpm tauri dev` 启动真实 Windows 桌面窗口，并以 WebView2 页面计算值和截图交叉核对：

- 项目页与 Review 页均使用完整深色主题，没有浅色外壳与深色编辑器混搭。
- 项目页通知入口按“通知 → 刷新 → 打开项目”排列；Review 页铃铛位于“详情”标题右端，空通知列表可打开。
- 左栏保留分支、时间范围、作者、提交信息、文件规则、分析状态和变更文件列表。
- 编辑器顶部只显示路径、提交范围和块数，没有重复的文件类型或 Minimap 按钮。
- Monaco 原始侧与修改侧均可聚焦和临时编辑；实际页面存在两侧 Minimap，AuthorRail 位于其外侧且宽度为 112px。
- 点击作者轨变更块后，右栏显示真实行范围与相关提交；作者标签不覆盖代码。
- 首次输入后状态栏切换为“临时草稿”，AuthorRail 消失，右栏显示“临时草稿不提供归因”；恢复后临时输入不存在，未发生保存。
- 状态栏编码菜单位于状态栏内部并向上展开；实际显示 `resolvedEncoding`，菜单保存请求编码。
- Monaco 文本层计算字体为 `JetBrainsMono Nerd Font Mono` → `Microsoft YaHei` → `monospace`，字号 13px、行高 22px。
- 工作台实际网格为 `320px 6px minmax(0, 1fr) 6px 320px`；编辑区内部为 Monaco + 112px AuthorRail，右栏没有被推出视口。
- 通知队列始终保留铃铛入口；信息、警告和用户操作错误统一进入会话内通知历史，局部错误与重试入口仍保留。

## 配置与异常行为

Rust 临时目录测试覆盖以下行为：

- 首次缺少设置文件时，以独占创建方式写入完整 `editor.toml`。
- 完整合法 TOML 可在下次启动读取；运行中不热更新。
- 损坏或包含非法值的 TOML 保留原文件，应用回退默认设置并返回包含配置路径的警告。
- 设置警告由前端发布到通知中心，不写回 TOML。
- 默认字体、13px/22px、1 MiB 和 5000 行阈值与设计文档一致。

## 生命周期与非目标审计

- 草稿仅存在于 Monaco Model 与组件内存，没有本地存储、IPC 或文件写入路径。
- 编辑器实现只使用 Monaco 公开 API，不查询或修改 Monaco 私有 DOM。
- Editor、Model、Decoration、ResizeObserver、监听器和 rAF 均有对应释放路径。
- 不存在旧自绘 Diff 运行时回退、`DiffBlockAuthors.vue` 或代码区作者标签。
- 未增加保存、设置页面、热更新配置或旧 Diff 回退。
- 运行时错误通知具有请求代次门禁、32 个候选任务缓存上限和终态副作用幂等，不会由过期请求产生重复或陈旧通知。

## 关键修复提交

- `97d67bd`：统一收纳文件、提交和 Monaco 运行时错误。
- `ceb0732`：补齐 Review、项目操作与元数据错误入口。
- `1cb839e`：用分析代次隔离早到与过期任务事件。
- `dce50dd`：合并分析快照并限制候选缓存。
- `553e4e5`：保证终态任务副作用幂等。
- `f0bcdc0`：清理 Rust 全工作区严格 Clippy 告警。

