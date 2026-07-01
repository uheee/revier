# Rust 索引文件查询验证记录

## 环境

- 日期：2026-07-02T01:07:14.7856922+08:00
- Shell：PowerShell 7.5.5
- Node：v24.13.0（通过 `fnm exec --using-file node --version`）
- pnpm：10.28.1（通过 `fnm exec --using-file pnpm.CMD --version`）
- Rust：cargo 1.96.0 (30a34c682 2026-05-25)
- DuckDB：Windows 本地验证使用 `DUCKDB_DOWNLOAD_LIB=1`
- Smoke 仓库：`E:/Projects/revier`
- Smoke 分支：`develop`。计划示例中的 `main` 在该仓库不存在，因此使用当前实际分支。

## 功能验证

`cargo test --workspace`

- 退出状态：0
- 耗时：10358 ms
- 摘要：`cli_contract` 4 通过，`gix_capabilities` 5 通过，`index_git_commits` 2 通过，`index_git_diff` 3 通过，`index_query_files` 2 通过，`index_schema` 4 通过，`no_git_process` 1 通过，doc tests 0。

`pnpm typecheck`

- 实际命令：`fnm exec --using-file pnpm.CMD typecheck`
- 退出状态：0
- 耗时：4573 ms
- 摘要：`vue-tsc --noEmit -p tsconfig.web.json && tsc --noEmit -p tsconfig.node.json` 通过。

`pnpm test`

- 实际命令：`fnm exec --using-file pnpm.CMD test`
- 退出状态：0
- 耗时：6770 ms
- 摘要：27 个测试文件通过，61 个测试通过。
- 备注：`tests/unit/blockDetailPanel.test.ts` 仍输出既有 Vue 组件解析 warning；未导致测试失败。

## CLI 冒烟验证

Base：`c5c8280b6f3140255c2215334ea8cc1b1bf5249a`

Head：`743ea543e5ac038c4f6ca84337a480faf74c309b`

`cargo run -q -p revier-analysis -- index status --repo E:/Projects/revier --format json`

- 退出状态：0
- 耗时：548 ms
- JSON 摘要：`status=ready`，`schemaVersion=1`，`indexedCommitCount=48`，`indexedFileCount=284`

`cargo run -q -p revier-analysis -- index build --repo E:/Projects/revier --branch develop --format json`

- 退出状态：0
- 耗时：2352 ms
- JSON 摘要：`status=completed`，`indexedCommitCount=48`，`indexedFileCount=284`，`elapsedMs=219`，`warnings=[]`

`cargo run -q -p revier-analysis -- index query-files --repo E:/Projects/revier --base c5c8280b6f3140255c2215334ea8cc1b1bf5249a --head 743ea543e5ac038c4f6ca84337a480faf74c309b --branch develop --format json`

- 退出状态：0
- 耗时：549 ms
- JSON 摘要：`version=1`，`range.baseCommit=c5c8280b6f3140255c2215334ea8cc1b1bf5249a`，`range.headCommit=743ea543e5ac038c4f6ca84337a480faf74c309b`，`files` 为数组，文件数 144。
- 字段验证：输出使用 camelCase，例如 `baseCommit`、`headCommit`、`oldPath`、`isBinary`、`isPreviewable`。

## 性能记录

- TypeScript 原筛选参考：`fnm exec --using-file pnpm.CMD exec vitest run tests/integration/reviewAnalysis.test.ts` 退出状态 0；Vitest 单测试耗时 1255 ms，命令总耗时 3319 ms。
- Rust 冷索引：`cargo run -q ... index build` 命令 wall time 2352 ms；Rust 输出内部 `elapsedMs=219`。
- Rust 已建索引查询：`cargo run -q ... index query-files` 命令 wall time 549 ms。
- 已构建二进制查询：直接运行 `target/debug/revier-analysis.exe index query-files ...`，并将 `target/debug/deps` 加入 `PATH`，wall time 129 ms，文件数 144。
- Rust CLI 启动和 JSON 输出基线：直接运行 `target/debug/revier-analysis.exe index status ...`，wall time 113 ms。
- JSON 解析耗时：对直接 CLI 的 query-files 输出使用 PowerShell `ConvertFrom-Json` 解析，耗时 4 ms，文件数 144。

## 结论

已建索引查询达到 500 ms 参考目标：直接运行已构建 Rust CLI 的 query-files 为 129 ms。`cargo run -q` 形式的 query-files 为 549 ms，慢点主要来自 Cargo 包装启动开销，而不是 SQL 查询或 JSON 解析。PowerShell JSON 解析 144 个文件结果耗时 4 ms。

本次最终验收覆盖：

- `index status`、`index build`、`index query-files` 均可运行。
- Rust 生产源码通过 `tests/no_git_process.rs`，未调用 Git 进程。
- DuckDB schema 包含 `metadata`、`commits`、`commit_parents`、`commit_files`、`index_runs`。
- `query-files` 输出为 camelCase，文件对象兼容 TypeScript `ChangedFile`。
- Electron 在作者、作者搜索或提交信息筛选存在时调用 Rust 查询；可恢复错误降级到 TypeScript 路径。
- overlay 打开路径未迁移，仍使用现有 TypeScript overlay 和归因逻辑。
