# Revier 第三方库接入一期验证记录

## 范围

本阶段按已确认的方案完成第三方库接入一期：

- `tauri-plugin-log` 与前端日志插件：统一 Tauri、WebView、stdout、stderr 日志链路。
- `anyhow`：仅用于应用入口和 CLI 边界层。
- `atomic-write-file`：用于项目配置文件原子写入。
- `criterion` 与 `imara-diff`：建立行差异算法基准与契约对照，不替换生产算法。
- `@vueuse/core` 与 `@vueuse/shared`：收敛浏览器事件、存储、定时器和防抖工具代码。
- `@pinia/colada`：先在普通项目列表查询中试点，不迁移带请求 ID、取消、进度和本地写流程的复杂 review 状态。

## 依赖版本

Rust：

- `tauri-plugin-log = 2.9.0`
- `anyhow = 1.0.104`
- `atomic-write-file = 0.3.0`
- `criterion = 0.8.2`
- `imara-diff = 0.2.0`

前端：

- `@tauri-apps/plugin-log = ^2.9.0`
- `@vueuse/core = ^10.11.0`
- `@vueuse/shared = ^10.11.1`
- `@pinia/colada = ^1.4.2`

`@vueuse/shared` 是直接依赖，因为当前 `@vueuse/core 10.11` 的类型入口不导出本阶段需要的 `useIntervalFn` 和 `useDebounceFn`。

## 实现确认

### 日志

配置文件使用全局默认值和 per-channel 覆盖：

```toml
[logging]
enabled = true
level = "info"
format = "[${time:yyyy-MM-ddTHH:mm:ss.ms}][${level:short}] ${content}"

[logging.channels]
file = false
console = true
webview = true
stdout = true
stderr = true
```

对象形式 channel 支持覆盖 `enabled`、`level`、`format`，并支持 channel 独有字段。`file` 支持 `path`、`max-size`，`console` 支持 `color`。拼错 `enabled` 会被 TOML 反序列化拒绝。

`tauri-plugin-log` 不直接支持当前配置模型中的 `channels` 对象和格式模板，因此已在应用侧做配置解析、归一化和插件目标映射。

### 错误边界

`anyhow` 只保留在：

- `src-tauri/src/lib.rs`
- `crates/revier-analysis/src/main.rs`

核心库错误仍使用 `thiserror`，DuckDB、IO、JSON 等错误源保留 `source`，不再提前转成字符串。

### 原子写入

项目配置保存通过 `atomic-write-file` 完成临时文件写入和提交。新增测试覆盖写入失败、提交失败、连续快速保存后不残留临时文件。

### 行差异基准

生产算法未替换。原因是 `imara-diff` 的行 tokenization 会保留尾随换行差异，而当前生产算法忽略最终尾随换行存在性。该语义会影响 UI 和缓存签名，不能只按性能结果直接替换。

本机基准环境：

- `rustc 1.96.0 (ac68faa20 2026-05-25)`
- Host：`x86_64-pc-windows-msvc`
- LLVM：`22.1.2`
- CPU：`AMD Ryzen 9 9950X 16-Core Processor`

最近一次 `cargo bench -p revier-analysis --bench line_diff` 结果：

| 场景 | 当前 LCS | imara Histogram | imara Myers |
| --- | ---: | ---: | ---: |
| small_code，148 bytes，9 lines，80% | 941.85 ns - 1.0205 us | 348.60 ns - 356.97 ns | 480.23 ns - 496.98 ns |
| medium_repeated，11198 bytes，1600 lines，92% | 1.5868 ms - 1.6624 ms | 196.72 us - 204.10 us | 34.090 us - 35.292 us |
| large_local_change，46180 bytes，4000 lines，2% | 14.772 ms - 15.414 ms | 42.808 us - 45.417 us | 42.910 us - 44.268 us |
| large_disjoint，22980 bytes，2400 lines，100% | 3.1096 ms - 3.1715 ms | 26.518 us - 27.548 us | 29.100 us - 31.410 us |

## 候选库评估

| 候选库 | 结论 | 原因 |
| --- | --- | --- |
| `moka` / `lru` | 下一阶段可评估接入 | `progress_by_file_operation` 当前手写 1 小时 TTL 和 128 容量上限，适合抽成有界缓存。但运行中操作、取消 token 和事件 sink 不能被缓存策略误驱逐，需要先明确生命周期边界。 |
| `dashmap` | 暂缓 | `ReviewService` 的任务、过滤器、文件、上下文、取消 token、进度分散在多张 `Mutex<HashMap<...>>` 中，很多更新需要跨 map 一致性。直接替换成并发 map 可能把一致性问题隐藏得更深。 |
| `parking_lot` | 暂缓 | 可以降低 `std::sync::Mutex` 的 poison 处理噪音，但会改变锁污染语义。本阶段没有足够收益支撑全局替换。 |
| `garde` / `validator` | 暂缓 | 当前编辑器配置校验是私有嵌套结构，手写校验短且测试精确。等配置表面继续扩大后，再评估 derive 校验是否能减少重复。 |
| `directories` | 暂缓 | Tauri 侧已用 `app.path()` 获取应用目录。CLI 默认数据目录目前可用手写逻辑覆盖，等 CLI 配置正式用户化时再统一。 |
| `path-clean` / `dunce` | 暂缓 | 当前路径主要保存真实 `PathBuf` 或用户选择路径。Windows 显示归一化可以局部评估，但不应改变存储语义。 |
| `camino` | 不接入 | 项目需要保留 Windows 和非 UTF-8 路径兼容性，`Utf8PathBuf` 不适合作为核心路径类型。 |

## 验证命令

已通过：

```powershell
cargo fmt --all -- --check
$env:RUSTFLAGS='-C debuginfo=0'; cargo test --workspace
pnpm typecheck
pnpm test
pnpm lint
$env:RUSTFLAGS='-C debuginfo=0'; cargo bench -p revier-analysis --bench line_diff
git diff --check
```

补充说明：

- clean 后第一次 `cargo test --workspace` 曾在集成测试构建阶段报 `libduckdb_sys` 缺少 rlib 产物；单独运行相关测试通过，随后重跑 `cargo test --workspace` 完整通过，未复现。
- `pnpm test` 结果为 33 个测试文件、203 个测试用例全部通过。
- Vitest 中 `mainBootstrap` 的 stderr 是测试主动模拟 Monaco 语法初始化失败，用于验证启动流程继续挂载。

## 静态扫描残留

已扫描：

```powershell
rg -n "eprintln!|console\.error" crates/revier-analysis/src src-tauri/src src/renderer
rg -n "fs::write\(&self\.file_path|localStorage|addEventListener|removeEventListener|setInterval|setTimeout|AppError::DuckDb\(error\.to_string\(\)\)" src-tauri/src src/renderer crates/revier-analysis/src
```

残留说明：

- `src/renderer/api/logger.ts` 的 `console.error` 是前端日志 IPC 不可用时的兜底输出。
- `src-tauri/src/lib.rs` 与 `crates/revier-analysis/src/main.rs` 的 `eprintln!` 是日志插件初始化前或 CLI 顶层错误输出。
- `src-tauri/src/services/review.rs` 的 `eprintln!` 位于测试模块，用于输出性能测量信息。
- `src/renderer/composables/useReviewLayoutSizes.ts` 的 `window.localStorage` 是 VueUse `useStorage` 的显式后端。
- 未发现旧的 `fs::write(&self.file_path...)` 项目配置直写。
- 未发现旧的 `AppError::DuckDb(error.to_string())` 映射。

## 下一阶段建议

1. 不继续扩大 Pinia Colada 范围，除非先确认每类 review 请求的取消、请求 ID、防旧响应覆盖和进度模型。
2. 若要替换行差异算法，先新增 UI 快照或契约确认尾随换行差异、重复行锚点和缓存签名语义。
3. 可优先把文件操作进度注册抽成专用结构，再评估 `moka` 或 `lru` 是否替代手写 TTL/容量清理。
4. 在考虑 `dashmap` 前，先把多张任务相关 map 合并为单个任务记录，避免跨 map 一致性问题。
