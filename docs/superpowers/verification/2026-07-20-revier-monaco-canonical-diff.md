# Revier Monaco 单一 Diff 边界验收记录

**日期：** 2026-07-20

## 变更摘要

- Tauri 范围页初始 `review_get_file_overlay` 只返回文本、编码与元数据，`rows=None`、`blocks=[]`。
- 新增 `review_attribute_blocks`，按 Monaco 提供的块范围批量返回作者、相关提交和归因可信度。
- 前端新增 Monaco canonical block 适配层，点击命中、选中边界、状态栏块数和 AuthorRail 均使用同一批 Monaco 范围。
- `MonacoDiffSession` 显式使用 `diffAlgorithm: 'advanced'`、`ignoreTrimWhitespace: false`，并设置 `minimap.showSlider = 'always'`。
- Minimap 和滚动条滑块显式配置普通、hover、active 三态透明度。
- AuthorRail 改为卡片式框体，外壳非交互，内部选择按钮与 `…` 按钮同级；修改块几何使用左右侧可见区域纵向并集。

## 已执行验证

```text
cargo check -p revier-tauri --all-targets
通过

pnpm typecheck
通过

pnpm test -- tests/unit/monacoDiffSession.test.ts tests/unit/diffAuthorRail.test.ts tests/unit/diffBlockGeometry.test.ts tests/unit/rendererReviewStore.test.ts tests/unit/diffViewer.test.ts
通过：5 个测试文件，74 个用例

cargo test -p revier-analysis --test editor_contract --test overlay_attribution --test file_overlay_cli --test trace_block_cli
通过：2 + 14 + 16 + 9 个用例

cargo test -p revier-tauri
通过：42 个用例

cargo clippy --workspace --all-targets --all-features -- -D warnings
通过

git diff --check
通过，仅有 CRLF/LF 工作区提示
```

## 生成文件状态

```text
pnpm generate:bindings
通过

pnpm generate:bindings:check
在未提交状态下失败：src/renderer/generated/bindings.ts 相对 HEAD 存在本次新增契约类型差异。
```

该失败符合当前工作树状态；`bindings.ts` 已由生成命令更新，提交本次变更后该检查应恢复通过。

## 未完成项与风险

- 尚未运行真实 Tauri 视觉验收，未对截图中的具体 L203-L216 场景做手工复验。
- 本次实现保留 CLI 既有 Rust LCS overlay 行为；Tauri 范围页已切换为两阶段 Monaco 范围归因。
- Commit drilldown 当前仍由前端 Monaco 负责显示块，但未在本轮增加专门的下钻归因状态测试。
