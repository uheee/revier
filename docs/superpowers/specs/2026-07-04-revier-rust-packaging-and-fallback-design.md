# Revier Rust 打包与 Fallback 策略设计

> 状态：已实施。本文记录 Rust 分析 CLI、DuckDB 动态库、应用图标、平台构建脚本和 fallback 策略的设计与最终行为；仍需观察或后续调整的事项见文末。

## 背景

Revier 当前已经接入 Rust 分析 CLI：

- `crates/revier-analysis` 提供 `index status`、`index build`、`index query-files`、`file-overlay` 和 `trace-block`。
- `src/main/analysis/rustAnalysisClient.ts` 负责从 Electron 主进程 spawn Rust CLI。
- `src/main/ipc/reviewIpc.ts` 默认启用 Rust overlay，设置 `REVIER_USE_RUST_OVERLAY=0` 时关闭。
- `electron-builder.yml` 通过 `extraResources` 把 `build/revier-analysis/current` 打包到 Electron resources 下的 `revier-analysis` 目录。
- `scripts/build.ps1` 和 `scripts/build.sh` 已接管图标准备、Rust CLI 构建、DuckDB 动态库 staging、`pnpm build` 和可选 electron-builder 打包。

历史上，`scripts/cargo-duckdb-download.mjs` 曾用于给 Cargo 命令注入 `DUCKDB_DOWNLOAD_LIB=1`，`scripts/generate-win-icon.mjs` 曾用于程序化生成 Windows 图标。两者已删除，职责已由平台构建脚本替代。

开发态仍可能遇到 Windows loader 找不到 `target/debug/deps/duckdb.dll` 的问题。该问题只属于直接运行开发态 CLI 的终端环境：VS Code 调试配置会把 `target\debug\deps` 加入 `Path`；手动运行 `target/debug/revier-analysis.exe` 时，需要开发者自行把 `target/debug/deps` 加入当前终端 `PATH`。生产运行逻辑不注入开发态 `target/debug/deps`。

## 实施目标与结果

1. Windows x64、Linux x64 和 macOS universal 的 Rust CLI 打包路径已统一为 `build/revier-analysis/current` 暂存，再由 electron-builder 放入 resources。
2. 运行时 Rust CLI 定位规则已支持开发态 `target/debug` 和打包态 `process.resourcesPath/revier-analysis`。
3. `DUCKDB_DOWNLOAD_LIB=1` 只在平台脚本执行 Cargo 时设置，不写入生产运行逻辑。
4. `icon.png` 已成为构建期图标源文件，Windows、Linux、macOS 由平台脚本生成或复制对应资源。
5. 旧 Node 包装脚本已删除，平台脚本成为本地与 CI 的统一构建入口。
6. fallback-like 行为已梳理并收紧：`query-files` fallback 短期保留且通过 `console.warn` 可观测；`file-overlay` 不再默认自动切回 TypeScript。
7. GitHub Actions release workflow 已改为调用平台构建脚本，macOS job 会验证 Rust CLI 和 DuckDB dylib 双架构。

## 非目标

- 本阶段不实现跨平台交叉编译。
- 本阶段不把 Rust CLI 改成 N-API、WASM 或长期驻留服务。
- 本阶段不改变 Renderer、Pinia store 或 Vue 组件消费的 `FileOverlay` 契约。
- 本阶段不改变 Rust CLI 的 JSON 输出契约。
- 本阶段不把 `target/debug/deps` 写入生产 `RustAnalysisClient`。
- 本阶段不移除 TypeScript overlay 源码；只收紧是否自动调用它。
- 本阶段不维护多份平台专用 `Cargo.toml`。Cargo 依赖保持单一配置，平台差异放在构建脚本和 staging 规则中。

## 当前打包架构

当前采用“构建后暂存，再交给 electron-builder extraResources”的方式：

```text
pnpm dist:<platform>
  -> 平台脚本准备图标资源
  -> 平台脚本构建 Rust release CLI
     -> DUCKDB_DOWNLOAD_LIB=1 cargo build -p revier-analysis --release
     -> 复制 Rust CLI
     -> 复制 DuckDB 动态库
     -> 写入 build/revier-analysis/current/
  -> pnpm build
  -> electron-builder --<platform>
     -> extraResources: build/revier-analysis/current -> resources/revier-analysis
```

不带 `-Package` 或 `--package` 调用平台脚本时，流程会停在 `pnpm build` 之后，不执行 electron-builder。带打包参数时，脚本才继续生成安装包。

Windows 发布包内目标结构：

```text
resources/
  revier-analysis/
    revier-analysis.exe
    duckdb.dll
```

macOS 发布包内目标结构：

```text
Resources/
  revier-analysis/
    revier-analysis
    libduckdb.dylib
```

Linux 发布包内目标结构：

```text
resources/
  revier-analysis/
    revier-analysis
    libduckdb.so
```

Rust CLI 启动时从自身所在目录或平台 loader/rpath 规则加载 DuckDB 动态库，不依赖全局环境变量。

## 平台脚本

### Windows

`scripts/build.ps1` 普通构建/打包命令：

```powershell
scripts/build.ps1 -Platform win -Arch x64
scripts/build.ps1 -Platform win -Arch x64 -Package
```

主要行为：

1. 从 `icon.png` 裁剪非白像素主体并生成 `build/icon.ico`。
2. 设置 `DUCKDB_DOWNLOAD_LIB=1` 执行 Cargo。
3. 构建 `revier-analysis` release CLI。
4. 复制 `revier-analysis.exe` 和 `duckdb.dll` 到 `build/revier-analysis/current`。
5. 执行 `pnpm build`。
6. 仅在传入 `-Package` 时执行 electron-builder，生成 Windows x64 MSI。

### Linux

`scripts/build.sh` Linux 普通构建/打包命令：

```bash
scripts/build.sh --platform linux --arch x64
scripts/build.sh --platform linux --arch x64 --package
```

主要行为：

1. 复制 `icon.png` 到 `build/icon.png`。
2. 设置 `DUCKDB_DOWNLOAD_LIB=1` 执行 Cargo。
3. 构建 Linux release CLI。
4. 复制 `revier-analysis` 和 `libduckdb.so` 到 `build/revier-analysis/current`。
5. 使用 `patchelf` 把 staged Rust CLI 的 rpath 设置为 `$ORIGIN`。
6. 执行 `pnpm build`。
7. 仅在传入 `--package` 时执行 electron-builder，生成 Linux deb/rpm。

Linux 构建和打包必须安装 `patchelf`；CI 中 Linux release job 已安装 `rpm` 和 `patchelf`。

### macOS

`scripts/build.sh` 当前要求 macOS 使用 universal 构建：

```bash
scripts/build.sh --platform mac --universal
scripts/build.sh --platform mac --universal --package
```

主要行为：

1. 使用 `sips` 和 `iconutil` 从 `icon.png` 生成 `build/icon.icns`。
2. 设置 `DUCKDB_DOWNLOAD_LIB=1` 分别构建 `x86_64-apple-darwin` 和 `aarch64-apple-darwin` Rust CLI。
3. 使用 `lipo` 合并 Rust CLI 为 universal 二进制。
4. staging `libduckdb.dylib`；如果输入不是 universal，则用 `lipo` 合并。
5. 使用 `install_name_tool` 把 DuckDB 引用修正为 `@executable_path/libduckdb.dylib`。
6. 通过 `lipo -info` 验证 staged Rust CLI 和 DuckDB dylib 架构。
7. 执行 `pnpm build`。
8. 仅在传入 `--package` 时执行 electron-builder，生成 macOS dmg/zip。

GitHub Actions macOS release job 还会执行 `lipo -verify_arch x86_64 arm64`，确保 `build/revier-analysis/current/revier-analysis` 和 `build/revier-analysis/current/libduckdb.dylib` 都包含双架构。

### Rust 测试模式

Windows：

```powershell
scripts/build.ps1 -RustTest
```

Unix：

```bash
scripts/build.sh --platform linux --arch x64 --rust-test
```

Rust 测试模式只设置 `DUCKDB_DOWNLOAD_LIB=1` 后运行 `cargo test --workspace`，不会准备 staging，不会运行 `pnpm build`，也不会执行 electron-builder 打包。Linux 测试模式不设置 staged Rust CLI rpath，因此不要求安装 `patchelf`。

## electron-builder 配置

`electron-builder.yml` 当前包含：

```yaml
extraResources:
  - from: build/revier-analysis/current
    to: revier-analysis
    filter:
      - "**/*"
```

`extraResources` 会把文件放在 Electron `process.resourcesPath` 下，不进入 asar，因此可执行文件和动态库都能被操作系统 loader 访问。

## package scripts

当前 `package.json` 使用平台脚本作为发布入口：

```json
{
  "scripts": {
    "dist": "pnpm dist:win",
    "dist:win": "pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -Platform win -Arch x64 -Package",
    "dist:linux": "bash scripts/build.sh --platform linux --arch x64 --package",
    "dist:mac": "bash scripts/build.sh --platform mac --universal --package",
    "rust:test:win": "pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -RustTest",
    "rust:test:unix": "bash scripts/build.sh --platform linux --arch x64 --rust-test"
  }
}
```

`pnpm build` 保持既有语义：类型检查后执行 Electron/Vite 构建。平台脚本内部调用 `pnpm build`，避免改变常规开发习惯。

历史上的 `prepare:win-icon` 脚本项已删除；图标准备由平台脚本负责。

## 已删除旧入口

- `scripts/cargo-duckdb-download.mjs`：已删除。它曾用于给 Cargo 注入 `DUCKDB_DOWNLOAD_LIB=1`，现在由 `scripts/build.ps1` 和 `scripts/build.sh` 的 Cargo 包装逻辑承担。
- `scripts/generate-win-icon.mjs`：已删除。它曾用于程序化生成 Windows 图标，现在由平台脚本基于仓库根目录 `icon.png` 生成平台图标资源。
- `prepare:win-icon`：已从 `package.json` 删除。发布入口统一走 `pnpm dist:win`、`pnpm dist:linux` 和 `pnpm dist:mac`。

## GitHub Actions

`.github/workflows/release.yml` 当前状态：

- checks job 继续运行 `pnpm lint`、`pnpm test` 和 `pnpm build`。
- Windows release job 安装 Rust target 后运行 `./scripts/build.ps1 -Platform win -Arch x64 -Package`。
- Linux release job 安装 `rpm` 与 `patchelf` 后运行 `./scripts/build.sh --platform linux --arch x64 --package`。
- macOS release job 安装两个 Rust target 后运行 `./scripts/build.sh --platform mac --universal --package`，并验证 staged Rust CLI 与 DuckDB dylib 均包含 `x86_64` 和 `arm64`。

本地和 CI 的发布构建入口已经保持一致。

## 运行时二进制定位规则

`RustAnalysisClient` 当前把 Rust CLI 路径解析拆成可测试逻辑，解析顺序为：

1. `REVIER_ANALYSIS_BIN` 存在时直接使用该路径。
2. Electron 打包态使用 `process.resourcesPath/revier-analysis/<binaryName>`。
3. 开发态使用 `process.cwd()/target/debug/<binaryName>`。

平台文件名：

```text
Windows: revier-analysis.exe
Linux: revier-analysis
macOS: revier-analysis
```

注意事项：

- 开发态 DuckDB DLL 搜索仍由调试配置或终端环境负责，不在生产代码中注入 `target/debug/deps`。
- 打包态 Rust CLI 与 DuckDB 动态库位于同一 resources 子目录，不依赖开发态 `target` 路径。
- `REVIER_ANALYSIS_BIN` 是显式调试或运维 override，不视为自动 fallback。

## Fallback 现状清单

| 编号 | 位置 | 当前行为 | 分类 | 当前策略 |
| --- | --- | --- | --- | --- |
| F1 | `REVIER_ANALYSIS_BIN` | 指定 Rust CLI 路径 | 显式 override | 保留。仅由开发者或运维显式设置。 |
| F2 | `REVIER_USE_RUST_OVERLAY=0` | 关闭 Rust overlay，走 TypeScript overlay | 显式 feature gate | 保留。作为排障和临时回退开关。 |
| F3 | `resolveAnalysisScope()` | 有作者或消息筛选时，Rust `query-files` 可恢复失败后走 TypeScript 文件列表筛选 | 自动 fallback | 短期保留；当前通过 `console.warn` 记录“Rust 文件列表查询不可用，已使用 TypeScript 路径继续分析。” |
| F4 | `buildFileOverlayForTask()` | Rust `file-overlay` 失败时不再自动走 TypeScript overlay | 已收紧行为 | 默认失败即失败；只有显式设置 `REVIER_USE_RUST_OVERLAY=0` 才走 TypeScript overlay。 |
| F5 | `mapRustError()` | Rust 退出码按命令映射 recoverable | 错误分类 | 保留分类，但调用方不对所有 recoverable 自动 fallback。 |
| F6 | `buildFileOverlayForTask()` | 二进制或不可预览文件直接返回 warning，不调用 Rust | 前置 guard | 保留。这不是 fallback，是业务保护。 |
| F7 | `buildTypescriptFileOverlay()` | `blameFileRange/listParents/getCommit` 缺失时使用空结果函数 | TypeScript 内部降级 | 保留为测试或无能力客户端保护；生产 `GitService` 应提供完整方法。 |
| F8 | `buildCommitOverlayForTask()` | 提交级下钻仍走 TypeScript 链路 | 未迁移路径 | 标记为未迁移，不称为 fallback。 |
| F9 | Rust overlay 内部索引读取 | 索引缺失时可以即时计算，`--require-index` 时失败 | 数据源策略 | 保留，通过 Rust CLI 语义表达是否强制索引。 |
| F10 | `.vscode/launch.json` | 调试时把 `target/debug/deps` 加入 `Path` | 调试环境配置 | 保留。仅限 VS Code 调试，不进入生产代码。 |

## Fallback 策略原则

所有 fallback 必须满足四条规则：

1. **显式性**：用户或开发者能知道当前走的是 Rust 还是 TypeScript。
2. **可测试性**：每个 fallback 都有单元测试或 smoke 验证。
3. **可观测性**：自动 fallback 必须产生 warning、日志或任务状态信息。
4. **可移除性**：每个临时 fallback 必须有清晰的移除条件。

### 长期保留

- `REVIER_ANALYSIS_BIN`：显式指定 Rust CLI。
- `REVIER_USE_RUST_OVERLAY=0`：显式关闭 Rust overlay。
- 二进制文件前置 guard：避免不可预览文件进入 overlay。
- 打包态 resources 路径：正常运行路径，不是 fallback。

### 短期保留

- `query-files` Rust 可恢复失败后走 TypeScript 文件列表筛选。

保留理由：文件列表是分析入口，索引缺失或 DuckDB 暂不可用时，TypeScript 路径仍可提供等价结果，只是更慢。当前行为已经通过 `console.warn` 可观测。

移除条件：索引构建入口、索引状态提示和打包 Rust CLI 均稳定后，把该 fallback 改为显式开关或用户操作。

### 已收紧

- `file-overlay` Rust 可恢复失败后自动走 TypeScript overlay 的行为已经移除。

当前行为：

1. 默认启用 Rust overlay。
2. Rust overlay 失败时向调用方暴露错误。
3. 用户或开发者需要回退时显式设置 `REVIER_USE_RUST_OVERLAY=0`。
4. Rust 进程不存在或打包资源缺失属于部署错误，不自动切回 TypeScript。

## 错误处理策略

Rust CLI 错误分为三类：

| 类型 | 示例 | recoverable | UI/调用方行为 |
| --- | --- | --- | --- |
| 部署错误 | Rust binary 不存在、DuckDB 动态库缺失、权限不足 | false | 显示明确错误，提示检查打包或调试配置。 |
| 数据源错误 | 索引缺失、索引 stale、DuckDB 查询失败 | `query-files` 可 true；overlay 默认 false | `query-files` 短期 fallback；overlay 默认显示错误。 |
| 分析输入错误 | 文件不可分析、仓库读取失败、参数错误 | 按命令区分 | 二进制 guard 保留；其他错误显示给调用方。 |

`3221225781 / 0xC0000135` 代表 Windows DLL 加载失败。它的根因是部署或调试环境缺少动态库路径，不应被当成普通 overlay 分析失败。

## 设计验收结果

- 文档中 fallback-like 行为已明确分类。
- 生产代码设计中不注入 `target/debug/deps`。
- Windows、Linux、macOS 打包态 Rust CLI 和 DuckDB 动态库位于同一个 resources 子目录。
- macOS Rust CLI 和 `libduckdb.dylib` 会验证 universal 双架构，覆盖 release workflow 当前的 x64/arm64 输出。
- Windows `.ico`、macOS `.icns` 和 Linux 图标资源均从 `icon.png` 派生，不再使用程序化图标生成器。
- GitHub Actions release workflow 调用与本地一致的平台构建脚本。
- 单一 `Cargo.toml` 适配所有目标平台，不维护平台专用依赖清单。
- `REVIER_ANALYSIS_BIN` 和 `REVIER_USE_RUST_OVERLAY=0` 的语义明确且互不混淆。

## 平台打包验证

```bash
pnpm dist:win
pnpm dist:linux
pnpm dist:mac
```

Windows 打包至少检查：

- `build/revier-analysis/current/revier-analysis.exe` 存在。
- `build/revier-analysis/current/duckdb.dll` 存在。
- Electron builder 输出 resources 中包含 `revier-analysis/revier-analysis.exe` 和 `revier-analysis/duckdb.dll`。
- 打包态不依赖 `target/debug/deps`。

macOS 打包至少检查：

- `build/revier-analysis/current/revier-analysis` 存在。
- `build/revier-analysis/current/libduckdb.dylib` 存在。
- Rust CLI 和 `libduckdb.dylib` 均包含 `x86_64` 与 `arm64`。
- Electron builder 输出 app resources 中包含 `revier-analysis/revier-analysis` 和 `revier-analysis/libduckdb.dylib`。

Linux 打包至少检查：

- `build/revier-analysis/current/revier-analysis` 存在。
- `build/revier-analysis/current/libduckdb.so` 存在。
- Electron builder 输出 app resources 中包含 `revier-analysis/revier-analysis` 和 `revier-analysis/libduckdb.so`。

## 后续观察项

- `query-files` 的 TypeScript fallback 目前短期保留，后续在索引构建、状态提示和打包 Rust CLI 稳定后，应改为显式开关或用户操作。
- 提交级下钻仍走 TypeScript 链路，这是未迁移能力，不是 fallback；后续如迁移到 Rust，需要单独设计。
- checks job 当前验证 `pnpm build`，release 平台 job 验证 Rust staging 与打包；如需要更早暴露 staging 问题，可在 checks job 增加不打包的平台脚本验证。
