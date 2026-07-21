# Revier 性能与进度改造阶段一验证记录

## 范围

本阶段仅完成：

- 统一操作进度契约；
- 与 Tauri 无关的进度报告器边界；
- 取消与进度组合执行上下文；
- TypeScript 绑定生成；
- 索引耗时在数据库主体写入完成后采样；
- 索引输出与 `index_runs.elapsed_ms` 一致性验证。

本阶段未创建缓存表，未接入 Tauri 进度事件，也未改变项目、文件或作者归因行为。

## TDD 记录

先新增 `operation_progress` 和索引耗时测试并运行，得到预期编译失败：

- 缺少 `OperationKind`、`OperationStatus`、`OperationStage`、`CacheState`；
- 缺少 `OperationProgressSnapshot`；
- 缺少 `OperationProgressReporter`、`OperationProgressUpdate`；
- 缺少数据库写入后耗时采样边界。

完成最小实现后，新增测试全部通过。

## 验证结果

### Rust 定向测试

```powershell
cargo test -p revier-analysis --test index_elapsed --test operation_progress --test bindings -j 1
cargo test -p revier-analysis commands::index_build::tests::elapsed_time_is_sampled_after_database_write --lib -j 1
```

- 退出状态：0。
- 绑定测试：2 通过。
- 索引输出与数据库耗时测试：1 通过。
- 操作进度与执行上下文测试：4 通过。
- 数据库写入后采样测试：1 通过。

### Rust 全量测试

```powershell
cargo test --workspace -j 1
```

- 退出状态：0。
- 现有 workspace 测试全部通过。
- 首次使用默认并行度运行时，Windows 链接器写入 `line_diff_memory` PDB 出现 `LNK1201`；单作业重试通过，未发现代码或断言失败。
- 新增 `index_elapsed` 测试在全量测试后单独补充运行并通过。

### TypeScript 类型检查

```powershell
& 'C:\Users\Snowind\AppData\Local\Microsoft\WinGet\Links\fnm.exe' exec --using-file pnpm.CMD typecheck
```

- 退出状态：0。
- `vue-tsc --noEmit -p tsconfig.web.json` 通过。

### 前端测试

```powershell
& 'C:\Users\Snowind\AppData\Local\Microsoft\WinGet\Links\fnm.exe' exec --using-file pnpm.CMD test
```

- 退出状态：0。
- 30 个测试文件通过。
- 184 个测试通过。

### 绑定生成

绑定已通过仓库 Rust 导出器生成到 `src/renderer/generated/bindings.ts`。

`generate:bindings:check` 会在生成后执行 `git diff --exit-code`。由于本阶段有意新增操作进度绑定，生成文件相对当前 Git 基线存在预期差异，因此该脚本退出 1；Rust 绑定导出测试、生成命令和 TypeScript 类型检查均通过。

### 格式检查

```powershell
cargo fmt --all
git diff --check
```

- Rust 格式化完成。
- Git whitespace 检查通过。

## 结论

阶段一满足设计文档中的契约和可观测性基础要求，可提交节点审核。数据库缓存、增量索引和界面状态栏尚未开始实现。

