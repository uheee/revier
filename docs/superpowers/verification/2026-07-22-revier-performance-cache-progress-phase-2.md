# Revier 性能缓存与进度改造：阶段二验收记录

## 1. 验收范围

本阶段仅交付数据库迁移与缓存仓储，不接入项目打开、文件打开、作者轨或提交下钻界面流程。

- DuckDB schema 从版本 1 迁移到版本 2。
- 建立按项目、分支保存单一成功快照的规范化缓存表。
- 建立项目快照、文件分析和提交下钻的原子读写仓储。
- 提供分支缓存状态判断、当前文件下钻删除和孤立缓存清理能力。
- 缓存只保存 Git blob ID，不保存源码正文。

## 2. 数据结构结果

新增 11 张缓存表：

- `analysis_snapshots`
- `analysis_filter_authors`
- `analysis_filter_globs`
- `analysis_files`
- `file_analyses`
- `file_blocks`
- `file_block_commits`
- `file_block_commit_ranges`
- `file_block_merge_sources`
- `commit_overlays`
- `commit_overlay_blocks`

提交元数据继续只由已有 `commits` 表保存，块与提交的关联表仅保存提交哈希和归因字段。文件内容不进入 DuckDB，文件与提交下钻只保存旧、新 Git blob ID。

## 3. 行为验证

- schema 1 到 schema 2 的迁移保留提交索引数据。
- migration 重复执行保持幂等；迁移失败时 schema 版本仍为 1。
- 同一项目、同一分支再次成功发布后只保留一个快照。
- 不同分支分别保留一个快照。
- 项目快照、文件分析和提交下钻替换失败时保留旧数据。
- 当前文件刷新或显式清理会删除该文件全部提交下钻。
- 缓存状态可区分未命中、命中和 HEAD／算法版本过期。
- 孤立缓存行可清理，仍被快照引用的数据保持不变。

两分支样本落盘结果：

- `analysis_snapshots`：2 行。
- `analysis_filter_authors`：2 行。
- `analysis_filter_globs`：2 行。
- `analysis_files`：2 行。
- DuckDB 文件大小：3,944,448 字节。

## 4. 自动化验证

以下命令均通过：

```powershell
cargo test -p revier-analysis --test cache_schema --test cache_repository -j 1 --config 'profile.test.debug=0' -- --nocapture
cargo test --workspace -j 1 --config 'profile.test.debug=0'
pnpm typecheck
pnpm test
pnpm lint
```

结果：

- 缓存仓储与迁移测试：15 项通过，0 项失败。
- Rust 全工作区单元、集成和文档测试：全部通过，0 项失败。
- TypeScript 类型检查：通过。
- 前端测试：30 个测试文件、184 项测试通过。
- ESLint：通过。

## 5. 构建空间处理

首次构建时 `target` 中重复的 DuckDB 调试产物导致 E 盘空间耗尽。经用户确认后，仅删除了 `E:\Projects\revier\target` 可再生构建目录，释放约 78.79 GiB；源码、缓存数据库和 Git 数据均未删除。后续使用单并发并关闭测试调试符号完成重建，构建产物可由 Cargo 重新生成。

## 6. 阶段边界

阶段二已经具备后续接入所需的数据结构和仓储 API。增量索引、打开项目优先恢复缓存、文件与作者轨缓存命中、进度事件接线及刷新按钮仍属于后续已规划阶段，本阶段未提前实现。
