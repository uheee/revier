# Revier Rust 打包与 Fallback 策略设计

> 状态：方向已确认，待实施计划。本文记录 Rust 分析 CLI、DuckDB 动态库和应用图标进入 Electron 发布包的设计，以及当前 Rust/TypeScript 分析链路中的 fallback 行为梳理。本文只定义后续实现方案，不修改运行时代码。

## 背景

Revier 当前已经接入 Rust 分析 CLI：

- `crates/revier-analysis` 提供 `index status`、`index build`、`index query-files`、`file-overlay` 和 `trace-block`。
- `src/main/analysis/rustAnalysisClient.ts` 负责从 Electron 主进程 spawn Rust CLI。
- `src/main/ipc/reviewIpc.ts` 默认启用 Rust overlay，设置 `REVIER_USE_RUST_OVERLAY=0` 时关闭。
- `scripts/cargo-duckdb-download.mjs` 当前会在 Cargo 构建时注入 `DUCKDB_DOWNLOAD_LIB=1`，让 Windows 开发环境下载 DuckDB 官方预编译动态库。该 Node 包装脚本后续应删除，由平台构建脚本统一承担。

当前开发态的问题已经暴露：`target/debug/revier-analysis.exe` 能启动，但 Windows loader 找不到 `target/debug/deps/duckdb.dll` 时会返回 `3221225781 / 0xC0000135`。调试配置已通过 `.vscode/launch.json` 把 `target\debug\deps` 加到调试进程 `Path`，但这个路径只属于开发构建产物，不能进入生产代码。

后续需要解决四个问题：

1. 发布包中 Rust CLI 和 DuckDB 动态库如何进入 Electron resources。
2. Rust 分析失败时哪些场景允许 fallback，哪些场景必须直接暴露错误。
3. Windows 和 macOS 打包图标如何改用仓库根目录的 `icon.png` 源图，避免继续使用程序化图标生成器。
4. GitHub Actions release workflow 如何复用本地平台构建脚本，避免 CI 与本地构建路径分叉。

## 目标

1. 定义 Windows x64、Linux x64 和 macOS universal 的 Rust CLI 打包路径。
2. 定义运行时 Rust CLI 定位规则，开发态继续使用 `target/debug`，打包态使用 `process.resourcesPath`。
3. 保持 `DUCKDB_DOWNLOAD_LIB=1` 只用于构建期，不把构建期路径写入生产运行逻辑。
4. 用 `icon.png` 作为唯一图标源文件，构建期裁剪过大白边并转换为 Windows `.ico`、macOS `.icns` 和 Linux 可用图标资源。
5. 删除 `scripts/cargo-duckdb-download.mjs` 和 `scripts/generate-win-icon.mjs`，用 `build.ps1`、`build.sh` 等平台脚本统一本地与 CI 构建入口。
6. 梳理所有 fallback 或 fallback-like 行为，明确保留、收紧或移除策略。
7. 为后续实现计划提供可测试的文件边界、命令和验收标准。

## 非目标

- 本阶段不实现跨平台交叉编译。
- 本阶段不把 Rust CLI 改成 N-API、WASM 或长期驻留服务。
- 本阶段不改变 Renderer、Pinia store 或 Vue 组件消费的 `FileOverlay` 契约。
- 本阶段不改变 Rust CLI 的 JSON 输出契约。
- 本阶段不把 `target/debug/deps` 写入生产 `RustAnalysisClient`。
- 本阶段不移除 TypeScript overlay 源码；只收紧是否自动调用它。
- 本阶段不维护多份平台专用 `Cargo.toml`。Cargo 依赖保持单一配置，平台差异放在构建脚本和 staging 规则中。

## 当前状态

### 打包状态

`electron-builder.yml` 当前只打包：

```yaml
files:
  - dist/**
  - package.json
```

没有把 Rust CLI 或 DuckDB 动态库加入 `extraResources`。`asar: true` 已启用，所以 Rust 可执行文件不能作为普通 `files` 塞进 asar 内部执行，必须作为 unpacked resource 随包分发。

`package.json` 当前发布脚本先运行 `pnpm build`，再运行 `electron-builder`。其中 `pnpm build` 只执行 TypeScript 类型检查和 Electron/Vite 构建，不会构建 Rust release CLI。

`.github/workflows/release.yml` 当前 Windows、Linux、macOS job 分别运行 `pnpm dist:win`、`pnpm dist:linux` 和 `pnpm dist:mac`。这些脚本目前没有统一调用 Rust staging，也没有统一调用图标资产准备逻辑。

### 图标状态

仓库根目录存在 `icon.png`，尺寸为 `1254x1254`。该图片主体位于画布中央，但白色背景留白较大。初步检测非白内容包围盒约为 `469x596`，需要在转换前按非白像素包围盒裁剪，并保留少量安全 padding。

当前 `scripts/generate-win-icon.mjs` 会用 Node `Buffer` 程序化绘制 `build/icon.ico`。后续应删除该生成器，改为从 `icon.png` 转换平台图标资源。

### 运行时定位状态

`RustAnalysisClient` 当前默认路径逻辑是：

```text
REVIER_ANALYSIS_BIN
  或 process.cwd()/target/debug/revier-analysis.exe
```

这适合开发态，但不适合打包态。打包后应用目录不会包含 `target`，也不应该要求用户安装 Rust toolchain。

### DuckDB 状态

Rust 依赖 `duckdb = "1.10504.0"`，不启用 `bundled`。当前 Windows 开发和验证路径通过 `DUCKDB_DOWNLOAD_LIB=1` 下载官方动态库。

`libduckdb-sys 1.10504.0` 的 `DUCKDB_DOWNLOAD_LIB=1` 已支持官方预编译动态库：

| 平台 | 官方压缩包 | 动态库 |
| --- | --- | --- |
| Windows x64 | `libduckdb-windows-amd64.zip` | `duckdb.dll` |
| Windows arm64 | `libduckdb-windows-arm64.zip` | `duckdb.dll` |
| macOS universal | `libduckdb-osx-universal.zip` | `libduckdb.dylib` |
| Linux x64 | `libduckdb-linux-amd64.zip` | `libduckdb.so` |
| Linux arm64 | `libduckdb-linux-arm64.zip` | `libduckdb.so` |

因此不需要为 Windows、macOS 或 Linux 拆分 `Cargo.toml`。同一份 `duckdb = "1.10504.0"` 依赖即可工作，平台差异由 Cargo target、构建脚本参数和动态库 staging 规则处理。

当前本地 debug 构建产物结构为：

```text
target/debug/revier-analysis.exe
target/debug/deps/duckdb.dll
```

Windows loader 会优先查可执行文件所在目录。因此打包产物中应该把 `revier-analysis.exe` 和 `duckdb.dll` 放在同一个资源目录内，而不是依赖全局 `Path`。

## 推荐打包架构

第一阶段采用“构建后暂存，再交给 electron-builder extraResources”的方式：

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

这样 Rust CLI 启动时可以从自身所在目录或平台 loader/rpath 规则加载 DuckDB 动态库，不需要额外修改全局环境变量。

### 平台脚本

建议新增两个平台入口：

```text
scripts/build.ps1
scripts/build.sh
```

两个脚本职责一致：

1. 准备图标资源。
2. 设置 Rust 构建环境变量。
3. 构建 Rust release CLI。
4. 收集 Rust CLI 和 DuckDB 动态库到 `build/revier-analysis/current`。
5. 调用 `pnpm build`。
6. 按参数决定是否调用 `electron-builder`。

脚本参数建议：

```text
scripts/build.ps1 -Platform win -Arch x64 -Package
scripts/build.sh --platform linux --arch x64 --package
scripts/build.sh --platform mac --universal --package
```

`-Package` / `--package` 存在时执行 Electron 打包；不存在时只完成图标、Rust staging 和 TypeScript/Electron 构建，便于 CI checks 复用。

### Windows 构建

Windows x64 构建流程：

```text
scripts/build.ps1 -Platform win -Arch x64 -Package
  -> 从 icon.png 生成 build/icon.ico
  -> 设置 DUCKDB_DOWNLOAD_LIB=1
  -> cargo build -p revier-analysis --release --target x86_64-pc-windows-msvc
  -> 复制 target/x86_64-pc-windows-msvc/release/revier-analysis.exe
  -> 复制 target/x86_64-pc-windows-msvc/release/deps/duckdb.dll
  -> pnpm build
  -> electron-builder --win msi --x64
```

如不显式传 `--target`，Cargo 会输出到 `target/release`。脚本可以支持 native fallback，但 release workflow 应显式传 target，避免路径不稳定。

### macOS 构建

macOS 当前 workflow 需要同时产出 x64 和 arm64 包。Rust CLI 也必须匹配 Electron 包的架构，不能只复制单架构二进制。

建议 macOS 第一阶段生成 universal Rust CLI：

```text
scripts/build.sh --platform mac --universal --package
  -> 从 icon.png 生成 build/icon.icns
  -> rustup target add x86_64-apple-darwin aarch64-apple-darwin
  -> DUCKDB_DOWNLOAD_LIB=1 cargo build -p revier-analysis --release --target x86_64-apple-darwin
  -> DUCKDB_DOWNLOAD_LIB=1 cargo build -p revier-analysis --release --target aarch64-apple-darwin
  -> lipo -create 两个 revier-analysis 二进制，输出 build/revier-analysis/current/revier-analysis
  -> 复制 libduckdb.dylib
  -> pnpm build
  -> electron-builder --mac dmg zip --x64 --arm64
```

`libduckdb-sys` 下载的是 `libduckdb-osx-universal.zip`，动态库本身是 universal，因此 staging 只需要保留一份 `libduckdb.dylib`。脚本应验证：

```text
lipo -info build/revier-analysis/current/revier-analysis
lipo -info build/revier-analysis/current/libduckdb.dylib
```

### Linux 构建

Linux x64 构建流程：

```text
scripts/build.sh --platform linux --arch x64 --package
  -> 可选从 icon.png 准备 Linux 图标资源
  -> 设置 DUCKDB_DOWNLOAD_LIB=1
  -> cargo build -p revier-analysis --release --target x86_64-unknown-linux-gnu
  -> 复制 target/x86_64-unknown-linux-gnu/release/revier-analysis
  -> 复制 target/x86_64-unknown-linux-gnu/release/deps/libduckdb.so
  -> pnpm build
  -> electron-builder --linux deb rpm --x64
```

Linux release workflow 已安装 `rpm`，后续仍保留该依赖安装步骤。

### 暂存目录

建议使用固定暂存目录：

```text
build/revier-analysis/current/
```

原因：

- `electron-builder.yml` 可以使用稳定 `from` 路径，不依赖平台变量宏是否支持。
- 该目录是生成产物，应加入 `.gitignore`。
- 每次 stage 前清空该目录，可以避免旧平台或旧 arch 的文件混入。

平台脚本每次运行前清理该目录。由于 GitHub Actions 每个 job 只构建一个平台，固定 `current` 目录足够稳定。后续如果要在同一个工作区同时产出多平台，可以扩展为：

```text
build/revier-analysis/win-x64/
build/revier-analysis/linux-x64/
build/revier-analysis/macos-x64/
build/revier-analysis/macos-arm64/
```

当前不需要提前做矩阵抽象。

### electron-builder 配置

建议在 `electron-builder.yml` 增加：

```yaml
extraResources:
  - from: build/revier-analysis/current
    to: revier-analysis
    filter:
      - "**/*"
```

`extraResources` 会把文件放在 Electron `process.resourcesPath` 下，不进入 asar，因此可执行文件和动态库都能被操作系统 loader 访问。

### package scripts

建议删除：

```json
{
  "scripts": {
    "prepare:win-icon": "node scripts/generate-win-icon.mjs"
  }
}
```

建议新增或调整为平台脚本入口：

```json
{
  "scripts": {
    "build:app": "pnpm typecheck && electron-vite build --outDir dist",
    "dist:win": "pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -Platform win -Arch x64 -Package",
    "dist:linux": "bash scripts/build.sh --platform linux --arch x64 --package",
    "dist:mac": "bash scripts/build.sh --platform mac --universal --package"
  }
}
```

`pnpm build` 是否改名为 `build:app` 需要实施时确认。推荐保留 `pnpm build` 作为现有 TypeScript/Electron 构建语义，平台脚本内部调用 `pnpm build`，避免影响现有开发习惯。

Rust 调试脚本可以改为直接调用平台脚本，或在 `package.json` 中使用 shell 原生命令。删除 `scripts/cargo-duckdb-download.mjs` 后，不再保留 Node 包装层。

### 图标资源转换

`icon.png` 是唯一源图。后续脚本应生成：

```text
build/icon.ico
build/icon.icns
build/icon.png
```

转换规则：

1. 读取 `icon.png`。
2. 按“非白像素包围盒”裁剪主体。
3. 在裁剪结果四周增加固定比例 padding，避免图标贴边。
4. 输出 1024、512、256、128、64、48、32、16 等平台需要的尺寸。
5. Windows 生成 `.ico`，macOS 生成 `.icns`，Linux 保留 PNG 或图标目录。

实现工具建议：

- Windows：PowerShell 可调用 .NET 图像 API 或 ImageMagick。若依赖 ImageMagick，脚本必须先检测 `magick` 是否存在并给出清晰错误。
- macOS：优先使用 `sips` 和 `iconutil`，它们是 macOS 构建机常规可用工具。
- Linux：可使用 ImageMagick 或保留 PNG 源图给 electron-builder。

如果选择引入 Node 图像库，应先确认依赖；当前设计不要求新增 Node 图像依赖。

### GitHub Actions 接入

`.github/workflows/release.yml` 应改为使用平台脚本：

```yaml
- name: Build Windows installer
  shell: pwsh
  run: ./scripts/build.ps1 -Platform win -Arch x64 -Package
```

```yaml
- name: Build Linux packages
  shell: bash
  run: ./scripts/build.sh --platform linux --arch x64 --package
```

```yaml
- name: Build macOS x64/arm64 packages
  shell: bash
  run: ./scripts/build.sh --platform mac --universal --package
```

checks job 可以继续运行 `pnpm lint`、`pnpm test`、`pnpm build`。如果希望 checks 也验证 Rust staging，可追加不打包命令：

```yaml
- name: Build Rust analysis resources
  shell: bash
  run: ./scripts/build.sh --platform linux --arch x64
```

该步骤会触发 DuckDB 官方动态库下载，CI 网络失败时会暴露为构建失败。

## 运行时二进制定位规则

后续实现应把 Rust CLI 路径解析从 `defaultBinaryPath()` 拆成可测试函数，例如：

```text
resolveRustAnalysisBinaryPath(context)
```

解析顺序：

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
- `REVIER_ANALYSIS_BIN` 是显式调试/运维 override，不视为自动 fallback。

## Fallback 现状清单

| 编号 | 位置 | 当前行为 | 分类 | 建议策略 |
| --- | --- | --- | --- | --- |
| F1 | `REVIER_ANALYSIS_BIN` | 指定 Rust CLI 路径 | 显式 override | 保留。仅由开发者或运维显式设置。 |
| F2 | `REVIER_USE_RUST_OVERLAY=0` | 关闭 Rust overlay，走 TypeScript overlay | 显式 feature gate | 保留。作为排障和临时回退开关。 |
| F3 | `resolveAnalysisScope()` | 有作者/消息筛选时 Rust `query-files` 可恢复失败后走 TypeScript 文件列表筛选 | 自动 fallback | 短期保留，但必须记录 warning 或调试日志；后续索引稳定后改为显式开关。 |
| F4 | `buildFileOverlayForTask()` | Rust `file-overlay` 可恢复失败后走 TypeScript overlay | 自动 fallback | 建议收紧。默认 Rust overlay 后不应静默掩盖 Rust overlay 问题。 |
| F5 | `mapRustError()` | Rust 退出码 `4/5/6/2/3` 映射为 recoverable | 错误分类 | 保留分类，但调用方不应对所有 recoverable 都自动 fallback。 |
| F6 | `buildFileOverlayForTask()` | 二进制或不可预览文件直接返回 warning，不调用 Rust | 前置 guard | 保留。这不是 fallback，是业务保护。 |
| F7 | `buildTypescriptFileOverlay()` | `blameFileRange/listParents/getCommit` 缺失时使用空结果函数 | TypeScript 内部降级 | 建议只保留在测试或明确无能力客户端场景；生产 `GitService` 应提供完整方法。 |
| F8 | `buildCommitOverlayForTask()` | 提交级下钻仍走 TypeScript 链路 | 未迁移路径 | 标记为未迁移，不称为 fallback。后续单独设计 Rust commit overlay。 |
| F9 | Rust overlay 内部索引读取 | 索引缺失时可以即时计算，`--require-index` 时失败 | 数据源策略 | 保留，但必须通过 warning 或退出码表达是否使用索引。 |
| F10 | `.vscode/launch.json` | 调试时把 `target/debug/deps` 加入 `Path` | 调试环境配置 | 保留。仅限 VS Code 调试，不进入生产代码。 |

## Fallback 策略原则

后续所有 fallback 必须满足四条规则：

1. **显式性**：用户或开发者能知道当前走的是 Rust 还是 TypeScript。
2. **可测试性**：每个 fallback 都有单元测试或 smoke 验证。
3. **可观测性**：自动 fallback 必须产生 warning、日志或任务状态信息。
4. **可移除性**：每个临时 fallback 必须有清晰的移除条件。

推荐分类如下：

### 允许长期保留

- `REVIER_ANALYSIS_BIN`：显式指定 Rust CLI。
- `REVIER_USE_RUST_OVERLAY=0`：显式关闭 Rust overlay。
- 二进制文件前置 guard：避免不可预览文件进入 overlay。
- 打包态 resources 路径：正常运行路径，不是 fallback。

### 短期保留但需要可观测

- `query-files` Rust 可恢复失败后走 TypeScript 文件列表筛选。

保留理由：文件列表是分析入口，索引缺失或 DuckDB 暂不可用时，TypeScript 路径仍可提供等价结果，只是更慢。该行为应在任务 warning 或主进程日志中记录，例如：

```text
Rust 文件列表查询不可用，已使用 TypeScript 路径继续分析。
```

移除条件：索引构建入口、索引状态提示和打包 Rust CLI 均稳定后，把该 fallback 改为显式开关或用户操作。

### 建议收紧

- `file-overlay` Rust 可恢复失败后自动走 TypeScript overlay。

收紧理由：现在 Rust overlay 已默认启用，打开文件时静默切回 TypeScript 会掩盖 Rust overlay 的正确性、性能和打包问题。后续建议改成：

1. 默认启用 Rust overlay。
2. Rust overlay 失败时显示明确错误或任务 warning。
3. 用户或开发者需要回退时显式设置 `REVIER_USE_RUST_OVERLAY=0`。
4. 仅对“Rust 进程不存在或打包资源缺失”这类部署错误给出可读错误，不自动转 TypeScript。

### 建议改名或澄清

- 提交级下钻继续走 TypeScript：这是未迁移能力，不是 fallback。
- Rust 内部索引缺失即时计算：这是数据源策略，不是 TypeScript fallback。
- `.vscode` PATH：这是调试环境配置，不是运行时 fallback。

## 错误处理策略

Rust CLI 错误应分为三类：

| 类型 | 示例 | recoverable | UI/调用方行为 |
| --- | --- | --- | --- |
| 部署错误 | Rust binary 不存在、DuckDB 动态库缺失、权限不足 | false | 显示明确错误，提示检查打包或调试配置。 |
| 数据源错误 | 索引缺失、索引 stale、DuckDB 查询失败 | query-files 可 true；overlay 默认 false | query-files 可短期 fallback；overlay 默认显示错误。 |
| 分析输入错误 | 文件不可分析、仓库读取失败、参数错误 | 按命令区分 | 二进制 guard 保留；其他错误显示给调用方。 |

`3221225781 / 0xC0000135` 应识别为 Windows DLL 加载失败。它的根因是部署或调试环境缺少动态库路径，不应被当成普通 overlay 分析失败。

## 实施边界建议

后续实现可以拆成三组独立任务：

1. **平台构建脚本与资源 staging**
   - 新增 `scripts/build.ps1`。
   - 新增 `scripts/build.sh`。
   - 删除 `scripts/cargo-duckdb-download.mjs`。
   - 删除 `scripts/generate-win-icon.mjs`。
   - 从 `icon.png` 准备平台图标资源。
   - 构建 Rust CLI 并收集 DuckDB 动态库。
   - 修改 `electron-builder.yml` 加 `extraResources`。
   - 修改 `package.json` 的 Windows、Linux、macOS 打包脚本。
   - 修改 `.github/workflows/release.yml`，让 release job 调用平台脚本。
   - 增加 staging 验证。

2. **运行时路径解析**
   - 抽出可测试的 Rust binary path resolver。
   - 打包态解析 `process.resourcesPath/revier-analysis`。
   - 开发态继续解析 `target/debug`。
   - 保留 `REVIER_ANALYSIS_BIN` 优先级。

3. **fallback 收紧**
   - 为 `query-files` fallback 增加 warning 或日志。
   - 收紧 `file-overlay` 自动 TypeScript fallback。
   - 更新旧设计文档和 README 中关于默认启用、回退方式的描述。

## 测试策略

### 单元测试

- `resolveRustAnalysisBinaryPath()`：
  - 环境变量优先。
  - 打包态使用 resources 路径。
  - 开发态使用 `target/debug`。
  - Windows 文件名带 `.exe`。

- 平台脚本：
  - Windows 缺少 `revier-analysis.exe` 时报错。
  - Windows 缺少 `duckdb.dll` 时报错。
  - macOS 缺少 `revier-analysis` 时报错。
  - macOS 缺少 `libduckdb.dylib` 时报错。
  - Linux 缺少 `revier-analysis` 时报错。
  - Linux 缺少 `libduckdb.so` 时报错。
  - macOS universal 输出需包含 `x86_64` 和 `arm64`。
  - staging 目录每次构建前会被清空。

- 图标转换：
  - `icon.png` 存在且可读取。
  - 非白像素包围盒裁剪后输出不为空。
  - Windows 生成 `build/icon.ico`。
  - macOS 生成 `build/icon.icns`。
  - Linux 保留 PNG 图标资源。

- `reviewIpc`：
  - `query-files` recoverable 错误产生可观测 warning 或日志。
  - `file-overlay` Rust 错误不再静默 TypeScript fallback，除非 `REVIER_USE_RUST_OVERLAY=0`。
  - `REVIER_USE_RUST_OVERLAY=0` 仍显式走 TypeScript overlay。

### 命令验证

```powershell
pnpm typecheck
pnpm test
pnpm dist:win
```

Windows 打包验证至少检查：

- `build/revier-analysis/current/revier-analysis.exe` 存在。
- `build/revier-analysis/current/duckdb.dll` 存在。
- Electron builder 输出 resources 中包含 `revier-analysis/revier-analysis.exe` 和 `revier-analysis/duckdb.dll`。
- 打包态不依赖 `target/debug/deps`。

macOS 打包验证至少检查：

```bash
pnpm dist:mac
lipo -info build/revier-analysis/current/revier-analysis
lipo -info build/revier-analysis/current/libduckdb.dylib
```

- `build/revier-analysis/current/revier-analysis` 存在。
- `build/revier-analysis/current/libduckdb.dylib` 存在。
- Rust CLI 和 `libduckdb.dylib` 均包含 `x86_64` 与 `arm64`。
- Electron builder 输出 app resources 中包含 `revier-analysis/revier-analysis` 和 `revier-analysis/libduckdb.dylib`。

Linux 打包验证至少检查：

```bash
pnpm dist:linux
```

- `build/revier-analysis/current/revier-analysis` 存在。
- `build/revier-analysis/current/libduckdb.so` 存在。
- Electron builder 输出 app resources 中包含 `revier-analysis/revier-analysis` 和 `revier-analysis/libduckdb.so`。

## 待确认决策

1. `file-overlay` Rust 可恢复失败后是否按本文建议改为默认不自动 TypeScript fallback。
2. `query-files` fallback 的可观测方式使用任务 warning、主进程日志，还是两者都做。
3. 是否在 README 明确说明：VS Code 调试自动配置 DuckDB DLL 路径，终端 `pnpm dev` 仍需手动配置或使用后续新增脚本。
4. 图标转换是否允许依赖 ImageMagick；如果不允许，Windows 侧必须使用 PowerShell/.NET 实现 `.ico` 输出。
5. checks job 是否也必须验证 Rust staging，还是只在 release 平台 job 中验证。

## 验收标准

- 文档中所有 fallback-like 行为都有明确分类。
- 生产代码设计中不出现 `target/debug/deps` 运行时注入。
- Windows、Linux、macOS 打包态 Rust CLI 和 DuckDB 动态库位于同一个 resources 子目录。
- macOS Rust CLI 和 `libduckdb.dylib` 均为 universal，能覆盖 release workflow 当前的 x64/arm64 输出。
- Windows `.ico`、macOS `.icns` 和 Linux 图标资源均从 `icon.png` 派生，不再使用程序化图标生成器。
- GitHub Actions release workflow 调用与本地一致的平台构建脚本。
- 单一 `Cargo.toml` 适配所有目标平台，不维护平台专用依赖清单。
- `REVIER_ANALYSIS_BIN` 和 `REVIER_USE_RUST_OVERLAY=0` 的语义明确且互不混淆。
- 后续实现计划可以按本文的三组任务拆分，并分别验证。
