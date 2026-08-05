# Revier 第三方库接入第二期验证记录

- 日期：2026-08-06
- 范围：供应链治理、DuckDB 版本化迁移、运行时契约校验、性质测试、基础 fuzz target、质量 CI。

## 新增依赖与工具

- Rust 运行依赖：`include_dir`、`sha2`、`hex`。
- Rust 测试依赖：`proptest`。
- 前端运行依赖：`zod`。
- 前端开发依赖：`fast-check`、`knip`。
- 本地与 CI 工具：`cargo-deny`、`cargo-fuzz`。
- 供应链修正：`brace-expansion` override 从 `5.0.8` 升级到 `5.0.9`，解除 `GHSA-rgw5-rvv9-x895` high advisory。

## 供应链规则

- `deny.toml` 已启用安全公告、许可证、来源和重复版本检查。
- 安全公告阻塞；已确认且暂无升级路径的 Tauri/Wry 间接依赖公告记录在 ignore 列表。
- 重复版本与通配依赖当前为 warn。
- Node 审计脚本为 `pnpm audit:deps`，等级为 high。
- Knip 脚本为 `pnpm check:deps`，阶段一保留候选依赖，不自动删除。

## 数据迁移

- 迁移目录通过 `include_dir` 编译期嵌入。
- 新增 `schema_migrations` 历史表，迁移 checksum 使用 SHA-256。
- 迁移执行器支持新库初始化、旧库接管、重复执行幂等、checksum 不一致失败和缺失迁移失败。
- 当前迁移版本包含 `0004_create_migration_history.sql`。

## 契约校验

- 前端运行时边界使用 Zod `safeParse`。
- 校验失败统一映射为 `CONTRACT_VALIDATION_FAILED`。
- 已覆盖项目列表、项目详情和 review 状态等首批边界。
- 大型 review 结果和长任务内部状态仍按阶段零结论默认不纳入本期。

## 生成式测试

- Rust `proptest`：
  - `line_diff_property.rs`
  - `text_encoding_property.rs`
  - 默认 case 数：128。
- 前端 `fast-check`：
  - `rendererProjectStore.property.test.ts`
  - 默认 case 数按测试内配置执行。

## Fuzz

- 新增 cargo-fuzz workspace：`fuzz/`。
- 首批 target：
  - `line_diff`
  - `text_decode`
  - `migration_manifest`
- `fuzz/Cargo.lock` 已将 `duckdb` 和 `libduckdb-sys` 收敛到根工作区一致的 `1.10504.0`。
- Windows 短时运行需要使用 Visual Studio 自带 ASan DLL 目录：

```powershell
$env:PATH = 'D:\Program Files\Microsoft Visual Studio\18\Enterprise\VC\Tools\MSVC\14.51.36231\bin\Hostx64\x64;' + $env:PATH
```

- 已短时运行通过：

```powershell
cargo +nightly fuzz run line_diff -- -max_total_time=1
cargo +nightly fuzz run text_decode -- -max_total_time=1
cargo +nightly fuzz run migration_manifest -- -max_total_time=1
```

## CI 策略

- 新增 `.github/workflows/quality.yml`。
- 触发范围：
  - pull request
  - `main`、`master`、`develop` push
  - workflow_dispatch
- 阻塞门禁：
  - `cargo fmt --all -- --check`
  - `cargo test --workspace`
  - `pnpm typecheck`
  - `pnpm lint`
  - `pnpm test`
  - `cargo +nightly fuzz build line_diff`
  - `cargo +nightly fuzz build text_decode`
  - `cargo +nightly fuzz build migration_manifest`
- 报告模式：
  - `cargo deny check`
  - `pnpm audit:deps`
  - `pnpm check:deps`

## 本地验证结果

2026-08-06 已执行：

```powershell
cargo fmt --all -- --check
git diff --check
git ls-files --error-unmatch docs/third-party-integration-backlog.md
cargo test --workspace
cargo deny check
cargo +nightly fuzz build line_diff
cargo +nightly fuzz build text_decode
cargo +nightly fuzz build migration_manifest
fnm exec --using-file pnpm.CMD typecheck
fnm exec --using-file pnpm.CMD test
fnm exec --using-file pnpm.CMD lint
fnm exec --using-file pnpm.CMD check:deps
fnm exec --using-file pnpm.CMD audit:deps
fnm exec --using-file pnpm.CMD generate:bindings:check
fnm exec --using-file pnpm.CMD build
```

结果：

- Rust 格式检查通过。
- Git 空白检查通过。
- `docs/third-party-integration-backlog.md` 未被 Git 跟踪，命令按预期失败。
- `cargo test --workspace` 串行执行通过；并发 Cargo 构建曾触发一次 `libduckdb_sys` rlib 形态错误，串行复跑确认不是代码失败。
- `cargo deny check` 通过，保留既有 duplicate warnings。
- 三个 fuzz target 构建通过。
- `pnpm typecheck` 通过。
- `pnpm test` 通过，35 个测试文件、209 个测试通过。
- `pnpm lint` 通过。
- `pnpm check:deps` 通过，仅报告 Knip 配置提示。
- `pnpm audit:deps` 在 `brace-expansion` 升级后通过；当前剩余 4 low 和 14 moderate，不达到 high 阻塞等级。
- `pnpm generate:bindings:check` 通过，生成绑定无差异。
- `pnpm build` 通过，生成 Windows `msi` 和 `nsis` 包；构建产物不提交。

## 当前限制

- 依赖治理检查首次进入 CI 时采用报告模式；切换为阻塞门禁需要单独确认。
- fuzz 普通 CI 只构建 target，不做长跑。
- fuzz corpus、artifacts 和 coverage 不提交。
- `docs/third-party-integration-backlog.md` 保持未跟踪且不提交。
