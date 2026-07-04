# Revier Rust Packaging Build Scripts Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 用平台构建脚本替换旧的图标生成器和 DuckDB Node 包装脚本，并让 Windows、Linux、macOS 发布包都携带 Rust 分析 CLI、DuckDB 动态库和从 `icon.png` 派生的平台图标。

**Architecture:** 构建分为“资源准备”和“Electron 打包”两层：`scripts/build.ps1` 负责 Windows，`scripts/build.sh` 负责 Linux/macOS，二者都把 Rust CLI 与 DuckDB 动态库暂存到 `build/revier-analysis/current`，Electron 通过 `extraResources` 打包该目录。运行时路径解析从开发态 `target/debug` 和打包态 `process.resourcesPath/revier-analysis` 两个明确入口选择，不把 `target/debug/deps` 注入生产逻辑。

**Tech Stack:** PowerShell、Bash、Cargo、DuckDB 官方预编译动态库、electron-builder、Vitest、TypeScript、GitHub Actions。

---

## 文件结构

- 保留：`icon.png`
  - 作为唯一图标源文件。

- 新建：`scripts/build.ps1`
  - Windows 构建入口。
  - 生成 `build/icon.ico`。
  - 设置 `DUCKDB_DOWNLOAD_LIB=1` 后构建 Rust。
  - 暂存 `revier-analysis.exe` 与 `duckdb.dll`。
  - 可选择执行 Electron Windows 打包。

- 新建：`scripts/build.sh`
  - Linux/macOS 构建入口。
  - Linux 准备 PNG 图标资源；macOS 生成 `build/icon.icns`。
  - 设置 `DUCKDB_DOWNLOAD_LIB=1` 后构建 Rust。
  - macOS 生成 universal Rust CLI。
  - 暂存 `revier-analysis` 与 `libduckdb.so` / `libduckdb.dylib`。
  - 可选择执行 Electron Linux/macOS 打包。

- 删除：`scripts/generate-win-icon.mjs`
  - 不再用程序生成图标。

- 删除：`scripts/cargo-duckdb-download.mjs`
  - DuckDB 下载环境变量改由平台构建脚本设置。

- 修改：`.gitignore`
  - 忽略 `build/icon.ico`、`build/icon.icns`、`build/icon.png`、`build/revier-analysis/`。

- 修改：`package.json`
  - 移除 `prepare:win-icon`。
  - `dist:win`、`dist:linux`、`dist:mac` 改为调用平台脚本。
  - Rust 调试脚本改为显式平台脚本或直接移除旧 Node 包装引用。

- 修改：`electron-builder.yml`
  - 增加 `extraResources`。
  - 增加 macOS 图标配置。
  - Linux 图标改用生成后的 PNG 资源。

- 修改：`.github/workflows/release.yml`
  - Release 平台 job 调用 `scripts/build.ps1` / `scripts/build.sh`。
  - macOS job 验证 universal Rust CLI。

- 修改：`src/main/analysis/rustAnalysisClient.ts`
  - 抽出可测试的 Rust binary path resolver。
  - 打包态使用 `process.resourcesPath/revier-analysis`。

- 修改：`src/main/ipc/reviewIpc.ts`
  - 收紧 `file-overlay` 自动 TypeScript fallback。
  - 为 `query-files` fallback 增加可观测 warning 或日志。

- 修改：`tests/unit/rustAnalysisClient.test.ts`
  - 覆盖打包态、开发态、环境变量 override 的路径解析。

- 修改：`tests/unit/reviewIpc.test.ts`
  - 覆盖 `file-overlay` 不再静默 fallback。
  - 覆盖 `REVIER_USE_RUST_OVERLAY=0` 仍显式走 TypeScript。

- 修改：`README.md`
  - 更新 Windows/macOS/Linux 构建说明。
  - 移除旧 Node 包装脚本文档。
  - 说明调试态 DuckDB 动态库路径和打包态 resources 路径。

---

### 任务 1：更新图标资源构建路径

**Files:**
- Create: `scripts/build.ps1`
- Create: `scripts/build.sh`
- Delete: `scripts/generate-win-icon.mjs`
- Modify: `.gitignore`
- Modify: `electron-builder.yml`
- Modify: `package.json`

- [ ] **Step 1: 写入 Windows 图标转换入口**

在 `scripts/build.ps1` 中先实现图标准备函数。第一步只写图标相关函数，Rust 构建函数在任务 2 补充：

```powershell
param(
  [ValidateSet('win')]
  [string]$Platform = 'win',
  [ValidateSet('x64', 'arm64')]
  [string]$Arch = 'x64',
  [switch]$Package,
  [switch]$RustTest,
  [switch]$PrepareIconsOnly
)

$ErrorActionPreference = 'Stop'
$RepoRoot = Split-Path -Parent $PSScriptRoot
$IconSource = Join-Path $RepoRoot 'icon.png'
$BuildDir = Join-Path $RepoRoot 'build'
$WinIconPath = Join-Path $BuildDir 'icon.ico'

function New-Directory([string]$Path) {
  if (-not (Test-Path -LiteralPath $Path)) {
    New-Item -ItemType Directory -Path $Path | Out-Null
  }
}

function Get-NonWhiteBounds([System.Drawing.Bitmap]$Bitmap) {
  $minX = $Bitmap.Width
  $minY = $Bitmap.Height
  $maxX = -1
  $maxY = -1
  for ($y = 0; $y -lt $Bitmap.Height; $y += 1) {
    for ($x = 0; $x -lt $Bitmap.Width; $x += 1) {
      $pixel = $Bitmap.GetPixel($x, $y)
      $delta = [Math]::Max([Math]::Max([Math]::Abs(255 - $pixel.R), [Math]::Abs(255 - $pixel.G)), [Math]::Abs(255 - $pixel.B))
      if ($delta -gt 20) {
        if ($x -lt $minX) { $minX = $x }
        if ($y -lt $minY) { $minY = $y }
        if ($x -gt $maxX) { $maxX = $x }
        if ($y -gt $maxY) { $maxY = $y }
      }
    }
  }
  if ($maxX -lt 0) {
    throw 'icon.png 未检测到非白色主体'
  }
  return @{ X = $minX; Y = $minY; Width = $maxX - $minX + 1; Height = $maxY - $minY + 1 }
}

function Write-IconFromPng {
  Add-Type -AssemblyName System.Drawing
  New-Directory $BuildDir
  if (-not (Test-Path -LiteralPath $IconSource)) {
    throw '缺少 icon.png'
  }

  $source = [System.Drawing.Bitmap]::FromFile($IconSource)
  try {
    $bounds = Get-NonWhiteBounds $source
    $padding = [Math]::Ceiling([Math]::Max($bounds.Width, $bounds.Height) * 0.12)
    $cropX = [Math]::Max(0, $bounds.X - $padding)
    $cropY = [Math]::Max(0, $bounds.Y - $padding)
    $cropRight = [Math]::Min($source.Width, $bounds.X + $bounds.Width + $padding)
    $cropBottom = [Math]::Min($source.Height, $bounds.Y + $bounds.Height + $padding)
    $cropSize = [Math]::Max($cropRight - $cropX, $cropBottom - $cropY)

    $sizes = @(16, 32, 48, 256)
    $images = New-Object System.Collections.Generic.List[byte[]]
    foreach ($size in $sizes) {
      $canvas = New-Object System.Drawing.Bitmap($size, $size, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
      $graphics = [System.Drawing.Graphics]::FromImage($canvas)
      try {
        $graphics.Clear([System.Drawing.Color]::Transparent)
        $graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
        $sourceRect = New-Object System.Drawing.Rectangle($cropX, $cropY, $cropSize, $cropSize)
        $targetRect = New-Object System.Drawing.Rectangle(0, 0, $size, $size)
        $graphics.DrawImage($source, $targetRect, $sourceRect, [System.Drawing.GraphicsUnit]::Pixel)
        $stream = New-Object System.IO.MemoryStream
        $canvas.Save($stream, [System.Drawing.Imaging.ImageFormat]::Png)
        $images.Add($stream.ToArray())
      } finally {
        if ($stream) { $stream.Dispose() }
        $graphics.Dispose()
        $canvas.Dispose()
      }
    }

    $file = [System.IO.File]::Create($WinIconPath)
    try {
      $writer = New-Object System.IO.BinaryWriter($file)
      $writer.Write([UInt16]0)
      $writer.Write([UInt16]1)
      $writer.Write([UInt16]$sizes.Length)
      $offset = 6 + (16 * $sizes.Length)
      for ($i = 0; $i -lt $sizes.Length; $i += 1) {
        $size = $sizes[$i]
        $bytes = $images[$i]
        $writer.Write([byte]$(if ($size -eq 256) { 0 } else { $size }))
        $writer.Write([byte]$(if ($size -eq 256) { 0 } else { $size }))
        $writer.Write([byte]0)
        $writer.Write([byte]0)
        $writer.Write([UInt16]1)
        $writer.Write([UInt16]32)
        $writer.Write([UInt32]$bytes.Length)
        $writer.Write([UInt32]$offset)
        $offset += $bytes.Length
      }
      foreach ($bytes in $images) {
        $writer.Write($bytes)
      }
    } finally {
      if ($writer) { $writer.Dispose() }
      $file.Dispose()
    }
  } finally {
    $source.Dispose()
  }
  Write-Host "已生成 $WinIconPath"
}

Write-IconFromPng
if ($PrepareIconsOnly) {
  exit 0
}
```

- [ ] **Step 2: 写入 macOS/Linux 图标转换入口**

在 `scripts/build.sh` 中先实现图标准备函数。第一步只写图标相关函数，Rust 构建函数在任务 2 补充：

```bash
#!/usr/bin/env bash
set -euo pipefail

PLATFORM=""
ARCH="x64"
PACKAGE=0
UNIVERSAL=0
RUST_TEST=0
PREPARE_ICONS_ONLY=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --platform) PLATFORM="$2"; shift 2 ;;
    --arch) ARCH="$2"; shift 2 ;;
    --package) PACKAGE=1; shift ;;
    --universal) UNIVERSAL=1; shift ;;
    --rust-test) RUST_TEST=1; shift ;;
    --prepare-icons-only) PREPARE_ICONS_ONLY=1; shift ;;
    *) echo "未知参数：$1" >&2; exit 2 ;;
  esac
done

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUILD_DIR="$REPO_ROOT/build"
ICON_SOURCE="$REPO_ROOT/icon.png"

prepare_linux_icon() {
  mkdir -p "$BUILD_DIR"
  if [[ ! -f "$ICON_SOURCE" ]]; then
    echo "缺少 icon.png" >&2
    exit 1
  fi
  cp "$ICON_SOURCE" "$BUILD_DIR/icon.png"
  echo "已准备 $BUILD_DIR/icon.png"
}

prepare_macos_icon() {
  mkdir -p "$BUILD_DIR/icon.iconset"
  if [[ ! -f "$ICON_SOURCE" ]]; then
    echo "缺少 icon.png" >&2
    exit 1
  fi
  if ! command -v sips >/dev/null 2>&1; then
    echo "缺少 sips，无法生成 macOS 图标" >&2
    exit 1
  fi
  if ! command -v iconutil >/dev/null 2>&1; then
    echo "缺少 iconutil，无法生成 macOS 图标" >&2
    exit 1
  fi

  TMP_PNG="$BUILD_DIR/icon-source.png"
  cp "$ICON_SOURCE" "$TMP_PNG"
  sips -z 16 16 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_16x16.png" >/dev/null
  sips -z 32 32 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_16x16@2x.png" >/dev/null
  sips -z 32 32 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_32x32.png" >/dev/null
  sips -z 64 64 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_32x32@2x.png" >/dev/null
  sips -z 128 128 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_128x128.png" >/dev/null
  sips -z 256 256 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_128x128@2x.png" >/dev/null
  sips -z 256 256 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_256x256.png" >/dev/null
  sips -z 512 512 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_256x256@2x.png" >/dev/null
  sips -z 512 512 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_512x512.png" >/dev/null
  sips -z 1024 1024 "$TMP_PNG" --out "$BUILD_DIR/icon.iconset/icon_512x512@2x.png" >/dev/null
  iconutil -c icns "$BUILD_DIR/icon.iconset" -o "$BUILD_DIR/icon.icns"
  echo "已生成 $BUILD_DIR/icon.icns"
}

prepare_icons() {
  case "$PLATFORM" in
    linux) prepare_linux_icon ;;
    mac) prepare_macos_icon ;;
    *) echo "scripts/build.sh 仅支持 linux 或 mac" >&2; exit 2 ;;
  esac
}

prepare_icons
if [[ "$PREPARE_ICONS_ONLY" == "1" ]]; then
  exit 0
fi
```

注意：macOS 版本第一轮先用 `sips` 缩放，不做非白像素裁剪。后续如果小图标留白仍过大，再用 `magick -trim` 或专用裁剪逻辑增强。Windows `.ico` 先实现裁剪，因为当前主要问题发生在 Windows 安装包图标。

- [ ] **Step 3: 运行图标准备命令验证**

Windows 本地运行：

```powershell
pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -PrepareIconsOnly
```

Expected:

```text
已生成 E:\Projects\revier\build\icon.ico
```

macOS CI 或 macOS 本地运行：

```bash
bash scripts/build.sh --platform mac --prepare-icons-only
```

Expected:

```text
已生成 <repo>/build/icon.icns
```

Linux CI 或 Linux 本地运行：

```bash
bash scripts/build.sh --platform linux --prepare-icons-only
```

Expected:

```text
已准备 <repo>/build/icon.png
```

- [ ] **Step 4: 更新构建配置引用图标资源**

修改 `electron-builder.yml`：

```yaml
win:
  icon: build/icon.ico
  target:
    - target: msi
      arch:
        - x64

linux:
  icon: build/icon.png
  category: Development
  maintainer: Revier Maintainers <noreply@example.com>
  target:
    - target: deb
      arch:
        - x64
    - target: rpm
      arch:
        - x64

mac:
  icon: build/icon.icns
  category: public.app-category.developer-tools
  target:
    - target: dmg
      arch:
        - x64
        - arm64
    - target: zip
      arch:
        - x64
        - arm64
```

修改 `.gitignore`：

```gitignore
build/icon.ico
build/icon.icns
build/icon.png
build/icon.iconset/
build/icon-source.png
```

- [ ] **Step 5: 删除旧图标生成器和 package 脚本引用**

删除文件：

```text
scripts/generate-win-icon.mjs
```

修改 `package.json`，移除：

```json
"prepare:win-icon": "node scripts/generate-win-icon.mjs"
```

- [ ] **Step 6: 提交图标路径改造**

```powershell
git add scripts/build.ps1 scripts/build.sh electron-builder.yml package.json .gitignore
git rm scripts/generate-win-icon.mjs
git commit -m "build: 使用源图生成平台图标"
```

---

### 任务 2：用平台脚本接管 Rust 与 DuckDB 构建

**Files:**
- Modify: `scripts/build.ps1`
- Modify: `scripts/build.sh`
- Delete: `scripts/cargo-duckdb-download.mjs`
- Modify: `package.json`
- Modify: `README.md`

- [ ] **Step 1: 在 PowerShell 脚本中实现 Rust 构建与 staging**

追加到 `scripts/build.ps1`：

```powershell
$RustStageDir = Join-Path $BuildDir 'revier-analysis\current'
$RustTarget = if ($Arch -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'aarch64-pc-windows-msvc' }
$RustReleaseDir = Join-Path $RepoRoot "target\$RustTarget\release"
$RustExe = Join-Path $RustReleaseDir 'revier-analysis.exe'
$DuckDbDll = Join-Path $RustReleaseDir 'deps\duckdb.dll'

function Invoke-CargoWithDuckDb([string[]]$CargoArgs) {
  $previous = $env:DUCKDB_DOWNLOAD_LIB
  $env:DUCKDB_DOWNLOAD_LIB = '1'
  try {
    & cargo @CargoArgs
    if ($LASTEXITCODE -ne 0) {
      throw "cargo 执行失败：$($CargoArgs -join ' ')"
    }
  } finally {
    $env:DUCKDB_DOWNLOAD_LIB = $previous
  }
}

function Invoke-RustTest {
  Invoke-CargoWithDuckDb @('test', '--workspace')
}

function Build-RustAnalysis {
  rustup target add $RustTarget
  Invoke-CargoWithDuckDb @('build', '-p', 'revier-analysis', '--release', '--target', $RustTarget)
}

function Stage-RustAnalysis {
  if (Test-Path -LiteralPath $RustStageDir) {
    Remove-Item -LiteralPath $RustStageDir -Recurse -Force
  }
  New-Directory $RustStageDir
  if (-not (Test-Path -LiteralPath $RustExe)) {
    throw "缺少 Rust CLI：$RustExe"
  }
  if (-not (Test-Path -LiteralPath $DuckDbDll)) {
    throw "缺少 DuckDB 动态库：$DuckDbDll"
  }
  Copy-Item -LiteralPath $RustExe -Destination (Join-Path $RustStageDir 'revier-analysis.exe')
  Copy-Item -LiteralPath $DuckDbDll -Destination (Join-Path $RustStageDir 'duckdb.dll')
  Write-Host "已暂存 Rust 分析资源到 $RustStageDir"
}

if ($RustTest) {
  Invoke-RustTest
  exit 0
}

Build-RustAnalysis
Stage-RustAnalysis
```

- [ ] **Step 2: 在 Bash 脚本中实现 Linux/macOS Rust 构建与 staging**

追加到 `scripts/build.sh`：

```bash
STAGE_DIR="$BUILD_DIR/revier-analysis/current"

run_cargo_with_duckdb() {
  DUCKDB_DOWNLOAD_LIB=1 cargo "$@"
}

rust_target_for_linux() {
  case "$ARCH" in
    x64) echo "x86_64-unknown-linux-gnu" ;;
    arm64) echo "aarch64-unknown-linux-gnu" ;;
    *) echo "不支持的 Linux 架构：$ARCH" >&2; exit 2 ;;
  esac
}

stage_file() {
  local source="$1"
  local destination="$2"
  if [[ ! -f "$source" ]]; then
    echo "缺少文件：$source" >&2
    exit 1
  fi
  cp "$source" "$destination"
}

stage_clean() {
  rm -rf "$STAGE_DIR"
  mkdir -p "$STAGE_DIR"
}

build_linux_rust() {
  local target
  target="$(rust_target_for_linux)"
  rustup target add "$target"
  run_cargo_with_duckdb build -p revier-analysis --release --target "$target"
  stage_clean
  stage_file "$REPO_ROOT/target/$target/release/revier-analysis" "$STAGE_DIR/revier-analysis"
  stage_file "$REPO_ROOT/target/$target/release/deps/libduckdb.so" "$STAGE_DIR/libduckdb.so"
  chmod +x "$STAGE_DIR/revier-analysis"
  echo "已暂存 Linux Rust 分析资源到 $STAGE_DIR"
}

build_macos_rust() {
  if [[ "$UNIVERSAL" != "1" ]]; then
    echo "macOS release 构建必须使用 --universal" >&2
    exit 2
  fi
  rustup target add x86_64-apple-darwin aarch64-apple-darwin
  run_cargo_with_duckdb build -p revier-analysis --release --target x86_64-apple-darwin
  run_cargo_with_duckdb build -p revier-analysis --release --target aarch64-apple-darwin
  stage_clean
  lipo -create \
    "$REPO_ROOT/target/x86_64-apple-darwin/release/revier-analysis" \
    "$REPO_ROOT/target/aarch64-apple-darwin/release/revier-analysis" \
    -output "$STAGE_DIR/revier-analysis"
  stage_file "$REPO_ROOT/target/x86_64-apple-darwin/release/deps/libduckdb.dylib" "$STAGE_DIR/libduckdb.dylib"
  chmod +x "$STAGE_DIR/revier-analysis"
  lipo -info "$STAGE_DIR/revier-analysis"
  lipo -info "$STAGE_DIR/libduckdb.dylib"
  echo "已暂存 macOS Rust 分析资源到 $STAGE_DIR"
}

if [[ "$RUST_TEST" == "1" ]]; then
  run_cargo_with_duckdb test --workspace
  exit 0
fi

case "$PLATFORM" in
  linux) build_linux_rust ;;
  mac) build_macos_rust ;;
  *) echo "scripts/build.sh 仅支持 linux 或 mac" >&2; exit 2 ;;
esac
```

- [ ] **Step 3: 更新 package scripts**

修改 `package.json`：

```json
{
  "scripts": {
    "dev": "electron-vite dev --outDir dist",
    "build": "pnpm typecheck && electron-vite build --outDir dist",
    "preview": "electron-vite preview --outDir dist",
    "typecheck": "vue-tsc --noEmit -p tsconfig.web.json && tsc --noEmit -p tsconfig.node.json",
    "test": "vitest run",
    "test:watch": "vitest",
    "test:e2e": "pnpm build && playwright test",
    "test:perf": "vitest run -c vitest.perf.config.ts",
    "lint": "pnpm typecheck && eslint .",
    "dist": "pnpm dist:win",
    "dist:win": "pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -Platform win -Arch x64 -Package",
    "dist:linux": "bash scripts/build.sh --platform linux --arch x64 --package",
    "dist:mac": "bash scripts/build.sh --platform mac --universal --package",
    "build:cli": "tsc -p tsconfig.cli.json",
    "revier-analysis": "pnpm build:cli && node dist/cli/cli/revier-analysis.js",
    "rust:test:win": "pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -RustTest",
    "rust:test:unix": "bash scripts/build.sh --platform linux --arch x64 --rust-test"
  }
}
```

说明：删除旧的 `rust:index:*`、`rust:file-overlay`、`rust:trace-block` pnpm 包装脚本。需要运行 Rust CLI 时，使用平台脚本完成构建后直接运行 `target/<target>/release/revier-analysis`，或设置 `DUCKDB_DOWNLOAD_LIB=1` 后直接运行 `cargo run`。

- [ ] **Step 4: 删除 DuckDB Node 包装脚本**

```powershell
git rm scripts/cargo-duckdb-download.mjs
```

- [ ] **Step 5: 验证 Windows Rust staging**

```powershell
pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -Platform win -Arch x64
```

Expected:

```text
已生成 ...\build\icon.ico
已暂存 Rust 分析资源到 ...\build\revier-analysis\current
```

并确认：

```powershell
Test-Path .\build\revier-analysis\current\revier-analysis.exe
Test-Path .\build\revier-analysis\current\duckdb.dll
```

Expected:

```text
True
True
```

- [ ] **Step 6: 提交 Rust 构建脚本改造**

```powershell
git add scripts/build.ps1 scripts/build.sh package.json README.md
git rm scripts/cargo-duckdb-download.mjs
git commit -m "build: 使用平台脚本构建 Rust 分析资源"
```

---

### 任务 3：接入 electron-builder resources 与平台发布脚本

**Files:**
- Modify: `electron-builder.yml`
- Modify: `scripts/build.ps1`
- Modify: `scripts/build.sh`
- Modify: `.github/workflows/release.yml`

- [ ] **Step 1: 增加 extraResources**

修改 `electron-builder.yml`：

```yaml
extraResources:
  - from: build/revier-analysis/current
    to: revier-analysis
    filter:
      - "**/*"
```

- [ ] **Step 2: 在 Windows 脚本中调用 Electron 打包**

在 `scripts/build.ps1` 末尾追加：

```powershell
Push-Location $RepoRoot
try {
  & pnpm build
  if ($LASTEXITCODE -ne 0) {
    throw 'pnpm build 失败'
  }
  if ($Package) {
    & pnpm dlx --allow-build=electron-winstaller electron-builder@26.15.1 --win msi --x64 --publish=never
    if ($LASTEXITCODE -ne 0) {
      throw 'electron-builder Windows 打包失败'
    }
  }
} finally {
  Pop-Location
}
```

- [ ] **Step 3: 在 Bash 脚本中调用 Electron 打包**

在 `scripts/build.sh` 末尾追加：

```bash
cd "$REPO_ROOT"
pnpm build
if [[ "$PACKAGE" == "1" ]]; then
  case "$PLATFORM" in
    linux) pnpm dlx --allow-build=electron-winstaller electron-builder@26.15.1 --linux deb rpm --x64 --publish=never ;;
    mac) pnpm dlx --allow-build=electron-winstaller electron-builder@26.15.1 --mac dmg zip --x64 --arm64 --publish=never ;;
    *) echo "不支持的打包平台：$PLATFORM" >&2; exit 2 ;;
  esac
fi
```

- [ ] **Step 4: 更新 GitHub Actions release workflow**

修改 `.github/workflows/release.yml`：

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

在 macOS job 打包后追加验证：

```yaml
- name: Verify macOS Rust universal binary
  shell: bash
  run: |
    lipo -info build/revier-analysis/current/revier-analysis
    lipo -info build/revier-analysis/current/libduckdb.dylib
```

- [ ] **Step 5: 验证 release workflow YAML 仍可解析**

运行：

```powershell
Get-Content -Raw .github\workflows\release.yml | ConvertFrom-Yaml | Out-Null
```

如果本地 PowerShell 没有 `ConvertFrom-Yaml`，用 Ruby 验证：

```powershell
ruby -e "require 'yaml'; YAML.load_file('.github/workflows/release.yml')"
```

Expected: 命令退出码为 `0`。

- [ ] **Step 6: 提交打包接入**

```powershell
git add electron-builder.yml scripts/build.ps1 scripts/build.sh .github/workflows/release.yml
git commit -m "build: 将 Rust 分析资源打入发布包"
```

---

### 任务 4：实现打包态 Rust CLI 路径解析

**Files:**
- Modify: `src/main/analysis/rustAnalysisClient.ts`
- Modify: `tests/unit/rustAnalysisClient.test.ts`

- [ ] **Step 1: 写路径解析失败测试**

修改 `tests/unit/rustAnalysisClient.test.ts`，导入新函数：

```ts
import {
  resolveRustAnalysisBinaryPath,
  RustAnalysisClient,
  type RustAnalysisExecutor
} from '../../src/main/analysis/rustAnalysisClient';
```

新增测试：

```ts
it('uses REVIER_ANALYSIS_BIN before packaged and development defaults', () => {
  const result = resolveRustAnalysisBinaryPath({
    env: { REVIER_ANALYSIS_BIN: 'E:/tools/revier-analysis.exe' },
    cwd: 'E:/repo/revier',
    platform: 'win32',
    isPackaged: true,
    resourcesPath: 'E:/repo/revier/resources'
  });

  expect(result).toBe('E:/tools/revier-analysis.exe');
});

it('uses Electron resources path when the app is packaged', () => {
  const result = resolveRustAnalysisBinaryPath({
    env: {},
    cwd: 'E:/repo/revier',
    platform: 'win32',
    isPackaged: true,
    resourcesPath: 'E:/Program Files/Revier/resources'
  });

  expect(result).toBe('E:/Program Files/Revier/resources/revier-analysis/revier-analysis.exe');
});

it('uses target debug path when the app is not packaged', () => {
  const result = resolveRustAnalysisBinaryPath({
    env: {},
    cwd: 'E:/repo/revier',
    platform: 'win32',
    isPackaged: false,
    resourcesPath: undefined
  });

  expect(result).toBe('E:/repo/revier/target/debug/revier-analysis.exe');
});
```

- [ ] **Step 2: 运行测试确认失败**

```powershell
fnm exec --using-file pnpm.CMD exec vitest run tests/unit/rustAnalysisClient.test.ts
```

Expected: 失败，提示 `resolveRustAnalysisBinaryPath` 未导出。

- [ ] **Step 3: 实现路径解析**

修改 `src/main/analysis/rustAnalysisClient.ts`：

```ts
import { app } from 'electron';
import { join } from 'node:path';
```

新增类型和函数：

```ts
interface RustBinaryPathContext {
  env: NodeJS.ProcessEnv;
  cwd: string;
  platform: NodeJS.Platform;
  isPackaged: boolean;
  resourcesPath?: string;
}

export function resolveRustAnalysisBinaryPath(context: RustBinaryPathContext): string {
  if (context.env.REVIER_ANALYSIS_BIN) {
    return context.env.REVIER_ANALYSIS_BIN;
  }

  const binaryName = context.platform === 'win32' ? 'revier-analysis.exe' : 'revier-analysis';
  if (context.isPackaged) {
    if (!context.resourcesPath) {
      throw new RustAnalysisError('RUST_BINARY_UNAVAILABLE', '打包态缺少 Electron resourcesPath', false);
    }
    return join(context.resourcesPath, 'revier-analysis', binaryName);
  }

  return join(context.cwd, 'target', 'debug', binaryName);
}
```

替换 `defaultBinaryPath()`：

```ts
function defaultBinaryPath(): string {
  return resolveRustAnalysisBinaryPath({
    env: process.env,
    cwd: process.cwd(),
    platform: process.platform,
    isPackaged: app.isPackaged,
    resourcesPath: process.resourcesPath
  });
}
```

- [ ] **Step 4: 运行测试确认通过**

```powershell
fnm exec --using-file pnpm.CMD exec vitest run tests/unit/rustAnalysisClient.test.ts
```

Expected: `RustAnalysisClient` 测试全部通过。

- [ ] **Step 5: 提交运行时路径解析**

```powershell
git add src/main/analysis/rustAnalysisClient.ts tests/unit/rustAnalysisClient.test.ts
git commit -m "fix: 支持打包态 Rust 分析路径"
```

---

### 任务 5：收紧 overlay fallback 并保留显式回退

**Files:**
- Modify: `src/main/ipc/reviewIpc.ts`
- Modify: `tests/unit/reviewIpc.test.ts`

- [ ] **Step 1: 写 `file-overlay` 不再静默 fallback 的失败测试**

在 `tests/unit/reviewIpc.test.ts` 中新增或调整测试：

```ts
it('does not fall back to TypeScript overlay when Rust overlay fails and Rust overlay is enabled', async () => {
  const rustError = Object.assign(new Error('Rust overlay 失败'), {
    recoverable: true,
    code: 'RUST_DUCKDB_ERROR'
  });
  const rust = {
    getFileOverlay: vi.fn(async () => {
      throw rustError;
    })
  };
  const git = createAnalysisGitClient();

  await expect(
    buildFileOverlayForTask({
      project: createProject(),
      file: createChangedFile(),
      range: createRange(),
      rangeCommits: [],
      filters: createFilters(),
      git,
      rust
    })
  ).rejects.toMatchObject({
    code: 'RUST_DUCKDB_ERROR'
  });

  expect(git.readFileAtCommit).not.toHaveBeenCalled();
});
```

若测试 helper 名称与现有文件不同，使用现有 `reviewIpc.test.ts` 中已有的 project、range、file、filters 构造方式，不新增测试专用生产 API。

- [ ] **Step 2: 运行测试确认失败**

```powershell
fnm exec --using-file pnpm.CMD exec vitest run tests/unit/reviewIpc.test.ts
```

Expected: 失败，因为当前 `buildFileOverlayForTask()` 会捕获 recoverable Rust 错误并继续 TypeScript overlay。

- [ ] **Step 3: 收紧 `buildFileOverlayForTask()`**

修改 `src/main/ipc/reviewIpc.ts` 中 Rust overlay 分支：

```ts
  if (rust) {
    const overlay = await rust.getFileOverlay({
      repoPath: project.repoPath,
      baseCommit: range.baseCommit,
      headCommit: range.headCommit,
      branch: range.branch,
      filePath: file.path,
      globRules: filters.globRules,
      authorKeys: filters.authorKeys,
      authorQuery: filters.authorQuery,
      messageQuery: filters.messageQuery
    });
    return applyDisplayCommitFilters(overlay, filters);
  }
```

保留 `REVIER_USE_RUST_OVERLAY=0` 控制逻辑：

```ts
rust: isRustOverlayEnabled() ? rust : undefined
```

这样显式关闭 Rust overlay 时仍走 TypeScript overlay。

- [ ] **Step 4: 为 `query-files` fallback 增加可观测性**

在 `resolveAnalysisScope()` 的 catch 中增加 warning 文案返回。最小改动是扩展 `ResolvedAnalysisScope`：

```ts
interface ResolvedAnalysisScope {
  range: ResolvedCommitRange;
  files: ChangedFile[];
  rangeCommits: GitCommitSummary[];
  warnings: ReturnType<typeof createAppError>[];
}
```

Rust query-files recoverable catch 中：

```ts
const warnings = [
  createAppError(
    'RUST_QUERY_FILES_FALLBACK',
    'Rust 文件列表查询不可用，已使用 TypeScript 路径继续分析。',
    true,
    error instanceof Error ? error.message : undefined
  )
];
```

返回成功路径使用 `warnings: []`，fallback 路径返回 `warnings`。

如果当前 `AnalysisTaskManager` 没有承载 task warning 的字段，本任务只在主进程 `console.warn()` 中记录：

```ts
console.warn('Rust 文件列表查询不可用，已使用 TypeScript 路径继续分析。', error);
```

实施时优先选择不改共享 task 类型的 `console.warn()`，避免扩大 UI 契约。

- [ ] **Step 5: 运行回归测试**

```powershell
fnm exec --using-file pnpm.CMD exec vitest run tests/unit/reviewIpc.test.ts
```

Expected: `reviewIpc` 测试全部通过。

- [ ] **Step 6: 提交 fallback 收紧**

```powershell
git add src/main/ipc/reviewIpc.ts tests/unit/reviewIpc.test.ts
git commit -m "fix: 收紧 Rust overlay 自动回退"
```

---

### 任务 6：更新文档并清理旧引用

**Files:**
- Modify: `README.md`
- Modify: `docs/superpowers/specs/2026-07-04-revier-rust-packaging-and-fallback-design.md`
- Modify: `docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md`

- [ ] **Step 1: 查找旧脚本引用**

```powershell
rg -n "cargo-duckdb-download|generate-win-icon|prepare:win-icon|pnpm rust:index|pnpm rust:file-overlay|pnpm rust:trace-block" README.md docs package.json .github scripts
```

Expected: 命中旧引用，供后续逐项改写。

- [ ] **Step 2: 更新 README Windows DuckDB 与构建说明**

把 README 的 “Windows DuckDB 处理” 改为：

```markdown
### Rust 与 DuckDB 构建

Rust 侧使用 `duckdb` crate，但不启用 `bundled` 特性。构建脚本会设置 `DUCKDB_DOWNLOAD_LIB=1`，由 `libduckdb-sys` 下载 DuckDB 官方预编译动态库。

Windows：

```powershell
pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -Platform win -Arch x64
```

Linux：

```bash
bash scripts/build.sh --platform linux --arch x64
```

macOS universal：

```bash
bash scripts/build.sh --platform mac --universal
```

直接运行开发态 `target/debug/revier-analysis.exe` 时，如遇到 DuckDB 动态库加载失败，请将 `target/debug/deps` 加入当前终端的 `PATH`。VS Code 的 `Revier: 启动开发调试` 配置会自动处理调试 PATH。
```

- [ ] **Step 3: 更新常用脚本表**

删除旧 `pnpm rust:index:*`、`pnpm rust:file-overlay`、`pnpm rust:trace-block` 行，新增：

```markdown
| `pnpm rust:test:win` | Windows 下设置 DuckDB 下载环境后运行 Rust workspace 测试 |
| `pnpm rust:test:unix` | Linux/macOS 下设置 DuckDB 下载环境后运行 Rust workspace 测试 |
| `pnpm dist:win` | 使用 PowerShell 平台脚本构建 Windows MSI |
| `pnpm dist:linux` | 使用 Bash 平台脚本构建 Linux deb/rpm |
| `pnpm dist:mac` | 使用 Bash 平台脚本构建 macOS x64/arm64 包 |
```

- [ ] **Step 4: 更新验证记录中的历史说明**

在 `docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md` 末尾追加：

```markdown
## 后续构建脚本调整

2026-07-04 的打包设计确认后，`scripts/cargo-duckdb-download.mjs` 将被平台构建脚本替代。本文中历史命令仍表示当时验证方式，后续验证应优先使用 `scripts/build.ps1` 或 `scripts/build.sh`。
```

- [ ] **Step 5: 再次查找旧引用**

```powershell
rg -n "cargo-duckdb-download|generate-win-icon|prepare:win-icon" README.md docs package.json .github scripts
```

Expected: 只允许在历史验证记录或设计文档的“旧脚本将删除”语境中命中。

- [ ] **Step 6: 提交文档更新**

```powershell
git add README.md docs/superpowers/specs/2026-07-04-revier-rust-packaging-and-fallback-design.md docs/superpowers/verification/2026-07-02-rust-overlay-attribution.md
git commit -m "docs: 更新跨平台构建说明"
```

---

### 任务 7：最终验证

**Files:**
- Verify only

- [ ] **Step 1: 运行 TypeScript 类型检查**

```powershell
fnm exec --using-file pnpm.CMD typecheck
```

Expected: 退出码 `0`。

- [ ] **Step 2: 运行单元与集成测试**

```powershell
fnm exec --using-file pnpm.CMD test
```

Expected: 退出码 `0`。如仍出现既有 Vue component warning，记录 warning，但不能有失败测试。

- [ ] **Step 3: 运行 Windows Rust 构建脚本**

```powershell
pwsh -NoLogo -ExecutionPolicy Bypass -File scripts/build.ps1 -Platform win -Arch x64
```

Expected:

```text
已生成 ...\build\icon.ico
已暂存 Rust 分析资源到 ...\build\revier-analysis\current
```

- [ ] **Step 4: 验证 staging 文件**

```powershell
Test-Path .\build\revier-analysis\current\revier-analysis.exe
Test-Path .\build\revier-analysis\current\duckdb.dll
```

Expected:

```text
True
True
```

- [ ] **Step 5: 运行 Windows 打包 smoke**

```powershell
fnm exec --using-file pnpm.CMD dist:win
```

Expected:

- `release/*.msi` 存在。
- 打包过程没有 Rust binary 或 DuckDB 动态库缺失错误。

- [ ] **Step 6: 检查最终状态**

```powershell
git status --short --branch
git log --oneline -8
```

Expected:

- 工作区干净。
- 最近提交包含构建脚本、运行时路径、fallback、文档更新。
