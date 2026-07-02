# Rust Overlay 与归因准确性验证记录

## 环境

- 日期：2026-07-03T04:23:45.1127955+08:00
- Shell：PowerShell 7.5.5
- Node：未找到。`node --version` 退出状态 1，当前 `pwsh` 找不到 `node`。
- pnpm：未找到。`pnpm --version` 退出状态 1，当前 `pwsh` 找不到 `pnpm`。
- fnm：未找到。`fnm --version` 退出状态 1，当前 `pwsh` 找不到 `fnm`。
- Rust：`cargo 1.96.0 (30a34c682 2026-05-25)`，`rustc 1.96.0 (ac68faa20 2026-05-25)`。
- DuckDB：路径 2，Rust 构建与测试通过 `DUCKDB_DOWNLOAD_LIB=1` 下载并链接官方预编译动态库。
- Smoke 仓库：`E:/Projects/revier`
- Smoke 分支：`develop`

## 功能验证

`$env:DUCKDB_DOWNLOAD_LIB='1'; cargo test --workspace`

- 退出状态：0
- 摘要：Rust 全工作区测试通过。
- 覆盖：`cli_contract` 7 通过，`file_overlay_cli` 7 通过，`git_blob` 1 通过，`gix_capabilities` 5 通过，`index_git_commits` 4 通过，`index_git_diff` 3 通过，`index_query_files` 3 通过，`index_schema` 4 通过，`line_diff_memory` 1 通过，`no_git_process` 1 通过，`overlay_attribution` 16 通过，`overlay_rows` 3 通过，`trace_block_cli` 9 通过，库单元测试 8 通过，doc tests 0。

`pnpm test -- tests/unit/scaffold.test.ts`

- 退出状态：1
- 结果：未进入测试执行。
- 原因：当前 `pwsh` 找不到 `pnpm`。

`pnpm typecheck`

- 退出状态：1
- 结果：未执行类型检查。
- 原因：当前 `pwsh` 找不到 `pnpm`。

`pnpm test`

- 退出状态：1
- 结果：未执行 Vitest。
- 原因：当前 `pwsh` 找不到 `pnpm`。

## CLI 冒烟验证

首次不传 `--db` 运行 `file-overlay` 时，CLI 默认索引路径位于 `C:/Users/Snowind/AppData/Roaming/revier/indexes`，当前沙箱无法打开该 DuckDB 文件，返回“拒绝访问”。随后改用工作区 `target/` 下的临时 `--db` 路径重跑。

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

Rust overlay 与 `trace-block` 的 Rust 测试、CLI 契约、fixture 归因验证和 smoke 验证均通过。路径 2 已在验证命令中使用 `DUCKDB_DOWNLOAD_LIB=1`。

Node 侧验证未完成：当前主 `pwsh` 环境找不到 `node`、`pnpm` 和 `fnm`，因此 `pnpm test -- tests/unit/scaffold.test.ts`、`pnpm typecheck`、`pnpm test` 均未能执行。该结果是环境阻塞，不记录为通过。
