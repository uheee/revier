# Revier 缓存 Schema 与迁移说明

## 1. 存储位置与边界

每个 Git 仓库按稳定 `repo_id` 使用独立 DuckDB 文件。数据库位于应用数据目录的 `revier/indexes/<repo_id>.duckdb`，项目元数据和界面偏好仍保存在 `projects.json`。

缓存不保存源码正文。文件内容通过快照中的 Git Blob ID 从对象库读取；DuckDB 只保存提交索引、筛选快照、文件元数据、块坐标、作者归因和提交下钻关系。

## 2. Schema v3

索引基础表：

- `metadata`：schema 版本、仓库身份和更新时间。
- `commits`、`commit_parents`、`commit_files`：增量提交索引。
- `index_runs`：索引运行状态、计数和耗时。

项目快照表：

- `analysis_snapshots`：每个 `repo_id + branch` 唯一，保存范围、筛选指纹、算法版本、耗时和最后选中文件。
- `analysis_filter_authors`、`analysis_filter_globs`：按序保存多值筛选条件。
- `analysis_files`：最终净变化文件、统计、预览能力和两侧 Blob ID。

文件归因表：

- `file_analyses`：每个 `analysis_id + path` 唯一，保存编码、规范块签名、算法版本及内容/归因耗时。
- `file_blocks`：紧凑块坐标、变化类型、置信度和警告位。
- `file_block_commits`、`file_block_commit_ranges`、`file_block_merge_sources`：块与提交、触达范围和 merge 来源关系。

提交下钻表：

- `commit_overlays`：每个 `file_analysis_id + commit_hash` 唯一，保存历史路径、父提交、两侧 Blob ID、编码和耗时。
- `commit_overlay_blocks`：下钻 Diff 的紧凑块坐标。

## 3. 保留与替换规则

- 不同分支独立保存；每个分支仅保留最近一次成功项目分析。
- 项目重新分析成功后原子发布新快照，并清理已无引用的旧快照数据。
- 当前文件重新分析成功后原子替换该文件归因，同时删除该文件的旧提交下钻；失败时保留旧数据。
- 同一文件分析下，不同提交分别保存；同一提交切换编码时替换原记录，不叠加多个编码副本。
- 失败或取消的项目分析不会发布半成品，也不会删除最近成功快照。

## 4. 命中与失效

- 分支 HEAD、分析算法版本、编码或规范块签名不一致时不得复用对应缓存。
- HEAD 变化时分支快照标记为 `stale`，仍可浏览旧结果，但必须先重新分析项目才能刷新单文件。
- 缓存引用的 Git Blob 丢失时返回稳定的 `CACHE_INVALID`，不降级为可能错误的内容。
- 可选的文件归因或提交下钻记录缺失时走冷计算，并在成功后原子写入。

## 5. 迁移

当前 `CURRENT_SCHEMA_VERSION` 为 3：

- v1 → v2：在事务内创建规范化项目快照、文件归因和提交下钻表，保留既有提交索引。
- v2 → v3：在事务内为 `analysis_snapshots` 增加 `last_selected_path`，保留既有缓存。
- v1 会依次执行 v1 → v2 → v3；迁移可重复调用。
- 任一迁移失败都会回滚，原 schema 版本与数据保持不变。
- 高于当前支持版本的数据库会返回 schema 不兼容错误，不会尝试降级或破坏性重建。

新数据库由初始化流程直接创建为 v3。迁移正确性、幂等性、失败回滚和“不保存源码正文”由 `crates/revier-analysis/tests/cache_schema.rs` 覆盖。
