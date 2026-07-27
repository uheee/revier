# Revier 归因修复执行计划（已按顺序执行）

## 目标

修复“多父 merge 归因中误判提交来源”的问题：当 merge 提交的目标文件内容同时来源于多个父提交，或嵌套 merge 形成链路时，仍应保留 merge-trace 路径；当仅由单一非第一父提交承载变化时，继续排除该 merge 作为第一父来源。

## 已执行顺序与结论

1. 修改 `crates/revier-analysis/src/attribution/source_validation.rs` 的 `first_parent_only_change` 规则。
   - 现状：当前仅要有任一非第一父 blob 与 merge 结果一致就直接否定，导致多父一致/嵌套场景被误杀。
   - 调整后：统计匹配的非第一父数量；当存在多个匹配时不直接否定，保留 merge 作为候选候选来源；仅当单一匹配时继续按原安全约束处理。
   - 说明：本次实现仅新增该模块内部逻辑，不修改公开契约。

2. 运行定向回归（按既定顺序）：
   - `cargo test -p revier-analysis --test overlay_attribution ambiguous_merge_sources_are_marked_partial`
   - `cargo test -p revier-analysis --test overlay_attribution nested_merge_source_tracks_real_source_and_chain`
   - `cargo test -p revier-analysis --test overlay_attribution ambiguous_nested_merge_sources_mark_partial_and_keep_branch_sources_only`
   - 均已通过。

3. 运行相关完整测试组：
   - `cargo test -p revier-analysis --test overlay_attribution`
   - `cargo test -p revier-analysis source_validation`
   - 均已通过，覆盖了 `viaMergeHashes` 空数组和置信度误判场景。

4. 尝试全量回归：
   - `cargo test -p revier-analysis`
   - 首次触发 `no_git_process` 门禁：测试区块中的 Git 夹具直接引用 `std::process::Command`，而门禁按 `src` 文件全文扫描，无法区分生产代码与 `#[cfg(test)]` 代码。
   - 已将通用 Git 测试进程辅助函数迁移到 `crates/revier-analysis/tests/support/git_process.rs`，并通过 `#[cfg(test)]` 模块复用；未修改测试断言、门禁规则或生产归因逻辑。
   - `cargo test -p revier-analysis --test no_git_process` 已通过。
   - `cargo test -p revier-analysis -- --test-threads=1` 已通过，包内单元测试、集成测试和文档测试均无失败。

## 最终验收结果

- 原 `production_source_does_not_spawn_git_process` 阻塞已解除。
- 来源校验单元测试：6 项通过。
- merge-trace 单元测试：5 项通过。
- overlay 归因集成测试：21 项通过。
- `revier-analysis` 包级全量回归：通过。
- 已执行 `cargo fmt --all`，全工作区 `cargo fmt --all -- --check` 通过。
- 已删除 `tests/fixtures.rs` 中未使用且无副作用的 `m1` 获取，相关 Rust 编译警告已消除。
- 格式与警告修复后再次执行 `cargo test -p revier-analysis -- --test-threads=1`，包内单元测试、集成测试和文档测试均无失败。
