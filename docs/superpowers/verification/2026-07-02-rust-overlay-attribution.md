# Rust Overlay 与归因准确性验证记录

## 环境

- 日期：2026-07-03T04:23:45.1127955+08:00
- Shell：PowerShell 7.5.5
- Node：`v24.13.0`。沙盒外通过 WinGet Links 中的 `fnm.exe` 初始化后可用。
- pnpm：`10.28.1`。通过 `fnm exec --using-file pnpm.CMD` 执行。
- fnm：`fnm 1.39.0`。当前受控沙盒 PATH 不含 `fnm`，沙盒外完整路径可用。
- Rust：`cargo 1.96.0 (30a34c682 2026-05-25)`，`rustc 1.96.0 (ac68faa20 2026-05-25)`。
- DuckDB：路径 2，Rust 构建与测试通过 `DUCKDB_DOWNLOAD_LIB=1` 下载并链接官方预编译动态库。
- Smoke 仓库：`E:/Projects/revier`
- Smoke 分支：`develop`

## 功能验证

`$env:DUCKDB_DOWNLOAD_LIB='1'; cargo test --workspace`

- 退出状态：0
- 摘要：Rust 全工作区测试通过。
- 覆盖：`cli_contract` 7 通过，`file_overlay_cli` 7 通过，`git_blob` 1 通过，`gix_capabilities` 5 通过，`index_git_commits` 4 通过，`index_git_diff` 3 通过，`index_query_files` 3 通过，`index_schema` 4 通过，`line_diff_memory` 1 通过，`no_git_process` 1 通过，`overlay_attribution` 16 通过，`overlay_rows` 3 通过，`trace_block_cli` 9 通过，库单元测试 8 通过，doc tests 0。

`& '<fnm.exe>' exec --using-file pnpm.CMD typecheck`

- 退出状态：0
- 摘要：`vue-tsc --noEmit -p tsconfig.web.json && tsc --noEmit -p tsconfig.node.json` 通过。
- 备注：`fnm` 输出 `--using-file is deprecated. This is now the default.`，不影响验证结果。

`& '<fnm.exe>' exec --using-file pnpm.CMD test -- tests/unit/reviewIpc.test.ts`

- 退出状态：0
- 摘要：Vitest 实际运行 27 个测试文件、74 个测试，全部通过；其中 `tests/unit/reviewIpc.test.ts` 12 个测试通过，`tests/unit/scaffold.test.ts` 5 个测试通过。
- 备注：`tests/unit/blockDetailPanel.test.ts` 仍输出既有 Vue 组件解析 warning；未导致测试失败。

`& '<fnm.exe>' exec --using-file pnpm.CMD test`

- 退出状态：0
- 摘要：27 个测试文件通过，74 个测试通过。
- 备注：`tests/unit/blockDetailPanel.test.ts` 仍输出既有 Vue 组件解析 warning；未导致测试失败。

## 运行时 smoke 验证

`$env:DUCKDB_DOWNLOAD_LIB='1'; cargo build -p revier-analysis`

- 退出状态：0
- 摘要：Rust CLI dev profile 构建通过。

直接运行已构建二进制前，将 `target/debug/deps` 加入本次进程 `PATH`，确保 Windows 可加载 DuckDB 动态库。

`target/debug/revier-analysis.exe index build --repo E:/Projects/revier --branch develop --db target/revier-runtime-smoke.duckdb --format json`

- 退出状态：0
- JSON 摘要：`status=completed`，`indexedCommitCount=74`，`indexedFileCount=393`。

`target/debug/revier-analysis.exe file-overlay --repo E:/Projects/revier --db target/revier-runtime-smoke.duckdb --base <Base> --head <Head> --branch develop --file docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md --format json`

- 退出状态：0
- JSON 摘要：`version=1`，`overlay.mode=range`，`overlay.rows=79`，`overlay.blocks=9`，`firstBlock=block-1`。

`target/debug/revier-analysis.exe trace-block --repo E:/Projects/revier --db target/revier-runtime-smoke.duckdb --base <Base> --head <Head> --branch develop --file docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md --block-id block-1 --format json`

- 退出状态：0
- JSON 摘要：`version=1`，`relatedCommits=1`，`confidence=precise`。

`REVIER_USE_RUST_OVERLAY=1` 下运行 Electron dev smoke：

- 首次运行失败，原因是当前 shell 环境残留 `ELECTRON_RUN_AS_NODE=1`，Electron 被当作 Node 运行，导致 main 进程加载 `electron` 模块时报 `Cannot read properties of undefined (reading 'exports')`。
- 对照验证：不设置 `REVIER_USE_RUST_OVERLAY` 也同样失败，因此该失败不是 Rust overlay 开关导致。
- 清理 `ELECTRON_RUN_AS_NODE` 后重新运行，main、preload、renderer dev 构建成功，renderer dev server 启动，Electron app 进入 `start electron app...` 阶段，未再出现 ESM/CJS 加载错误。
- 本次自动 smoke 未覆盖人工 UI 点击文件后的可视化对照；后续需要在真实窗口中打开项目文件，核对 overlay 行、块、作者和 warning。

## CLI 冒烟验证

首次不传 `--db` 运行 `file-overlay` 时，CLI 默认索引路径位于 Electron userData 下的 `revier/indexes`，当前沙箱无法打开该 DuckDB 文件，返回“拒绝访问”。随后改用工作区 `target/` 下的临时 `--db` 路径重跑。

通用参数：

- Base：`11830b238d1924ea6142e5d835db897820a900d4`
- Head：`a756dade6f9ae4a2727fd5ddffed7f32cb614aa5`
- DB：`E:/Projects/revier/target/revier-overlay-smoke-61977d03df2b4f1fb731a7f21eb6c620.duckdb`

`$env:DUCKDB_DOWNLOAD_LIB='1'; cargo run -q -p revier-analysis -- index build --repo E:/Projects/revier --branch develop --db <DB> --format json`

- 退出状态：0
- JSON 摘要：`status=completed`，`indexedCommitCount=71`，`indexedFileCount=387`。

`$env:DUCKDB_DOWNLOAD_LIB='1'; cargo run -q -p revier-analysis -- file-overlay --repo E:/Projects/revier --db <DB> --base <Base> --head <Head> --branch develop --file src/main/ipc/reviewIpc.ts --format json`

- 退出状态：0
- JSON 摘要：`version=1`，`overlay.mode=range`，`overlay.rows=533`，`overlay.blocks=12`，`firstBlock=block-1`，`overlay.warnings=0`，顶层 `warnings=0`。

`$env:DUCKDB_DOWNLOAD_LIB='1'; cargo run -q -p revier-analysis -- trace-block --repo E:/Projects/revier --db <DB> --base <Base> --head <Head> --branch develop --file src/main/ipc/reviewIpc.ts --block-id block-1 --format json`

- 退出状态：0
- JSON 摘要：`version=1`，`file=src/main/ipc/reviewIpc.ts`，`blockId=block-1`，`relatedCommits=1`，`confidence=precise`。

## 归因验证

- 线性新增/修改：`overlay_attribution::added_or_modified_block_uses_blame_as_precise_attribution` 通过，CLI smoke 中 `block-1` 返回 `confidence=precise`。
- merge conflict：`overlay_attribution::merge_conflict_resolution_attributes_new_content_to_merge_commit` 通过。
- 多父歧义：`overlay_attribution::ambiguous_merge_sources_are_marked_partial` 通过。
- 删除块：`overlay_attribution::linear_deleted_block_uses_precise_deletion_trace`、`overlay_attribution::deleted_block_uses_deletion_trace_or_inference`、`trace_block_cli::trace_block_can_select_deleted_block_by_old_line_range_and_returns_deletion_trace` 通过。
- rename：`overlay_attribution::rename_overlay_uses_old_path_and_does_not_drop_blocks`、`overlay_attribution::deletion_trace_follows_multi_hop_rename_middle_path`、`overlay_attribution::file_overlay_does_not_attach_unrelated_commit_after_rename` 通过。
- 生产 Rust 源码不调用 Git 进程：`no_git_process` 通过。

## 结论

Rust overlay 与 `trace-block` 的 Rust 测试、CLI 契约、fixture 归因验证和命令级 smoke 验证均通过。路径 2 已在验证命令中使用 `DUCKDB_DOWNLOAD_LIB=1`。

Node 侧验证已补充完成：通过沙盒外完整路径 `fnm.exe` 进入 Node 24 / pnpm 10.28.1 环境后，`pnpm typecheck`、`pnpm test -- tests/unit/reviewIpc.test.ts` 和 `pnpm test` 均通过。

Electron dev runtime smoke 已完成到应用启动阶段：清理 `ELECTRON_RUN_AS_NODE` 后，`REVIER_USE_RUST_OVERLAY=1` 不再阻塞 Electron dev 启动。人工 UI 交互对照仍待单独执行。

## 后续构建脚本调整

2026-07-04 后，`scripts/cargo-duckdb-download.mjs` 已由平台构建脚本替代。本文中的历史命令表示当时验证方式；后续验证优先使用 `scripts/build.ps1` 或 `scripts/build.sh`，由平台脚本负责设置 `DUCKDB_DOWNLOAD_LIB=1`、构建 Rust CLI，并暂存 DuckDB 动态库。
