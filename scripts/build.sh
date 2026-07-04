#!/usr/bin/env bash
set -euo pipefail

platform="linux"
arch="x64"
package=false
universal=false
rust_test=false
prepare_icons_only=false

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
build_root="$repo_root/build"

usage() {
  echo "用法：scripts/build.sh --platform linux|mac --arch x64|arm64 [--package] [--universal] [--rust-test] [--prepare-icons-only]"
}

require_value() {
  local option="$1"
  local value="${2:-}"

  if [[ -z "$value" || "$value" == --* ]]; then
    echo "${option} 缺少参数值" >&2
    exit 2
  fi
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --platform)
      require_value "$1" "${2:-}"
      platform="${2:-}"
      shift 2
      ;;
    --arch)
      require_value "$1" "${2:-}"
      arch="${2:-}"
      shift 2
      ;;
    --package)
      package=true
      shift
      ;;
    --universal)
      universal=true
      shift
      ;;
    --rust-test)
      rust_test=true
      shift
      ;;
    --prepare-icons-only)
      prepare_icons_only=true
      shift
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "未知参数：$1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

if [[ "$platform" != "linux" && "$platform" != "mac" ]]; then
  echo "不支持的平台：$platform" >&2
  usage >&2
  exit 1
fi

if [[ "$arch" != "x64" && "$arch" != "arm64" ]]; then
  echo "不支持的架构：$arch" >&2
  usage >&2
  exit 1
fi

prepare_linux_icon() {
  local source_path="$repo_root/icon.png"
  local build_dir="$build_root"
  local output_path="$build_dir/icon.png"

  if [[ ! -f "$source_path" ]]; then
    echo "未找到源图标：$source_path" >&2
    exit 1
  fi

  mkdir -p "$build_dir"
  rm -f "$output_path"
  cp "$source_path" "$output_path"
  echo "已生成 Linux 图标：$output_path"
}

prepare_mac_icon() {
  local source_path="$repo_root/icon.png"
  local build_dir="$build_root"
  local iconset_dir="$build_dir/icon.iconset"
  local output_path="$build_dir/icon.icns"

  if [[ ! -f "$source_path" ]]; then
    echo "未找到源图标：$source_path" >&2
    exit 1
  fi

  if ! command -v sips >/dev/null 2>&1; then
    echo "未找到 sips，无法生成 macOS 图标。" >&2
    exit 1
  fi

  if ! command -v iconutil >/dev/null 2>&1; then
    echo "未找到 iconutil，无法生成 macOS 图标。" >&2
    exit 1
  fi

  safe_remove_dir "$iconset_dir"
  rm -f "$output_path" "$build_dir/icon-source.png"
  mkdir -p "$iconset_dir"
  sips -z 16 16 "$source_path" --out "$iconset_dir/icon_16x16.png" >/dev/null
  sips -z 32 32 "$source_path" --out "$iconset_dir/icon_16x16@2x.png" >/dev/null
  sips -z 32 32 "$source_path" --out "$iconset_dir/icon_32x32.png" >/dev/null
  sips -z 64 64 "$source_path" --out "$iconset_dir/icon_32x32@2x.png" >/dev/null
  sips -z 128 128 "$source_path" --out "$iconset_dir/icon_128x128.png" >/dev/null
  sips -z 256 256 "$source_path" --out "$iconset_dir/icon_128x128@2x.png" >/dev/null
  sips -z 256 256 "$source_path" --out "$iconset_dir/icon_256x256.png" >/dev/null
  sips -z 512 512 "$source_path" --out "$iconset_dir/icon_256x256@2x.png" >/dev/null
  sips -z 512 512 "$source_path" --out "$iconset_dir/icon_512x512.png" >/dev/null
  sips -z 1024 1024 "$source_path" --out "$iconset_dir/icon_512x512@2x.png" >/dev/null
  iconutil -c icns "$iconset_dir" -o "$output_path"
  echo "已生成 macOS 图标：$output_path"
}

prepare_icons() {
  case "$platform" in
    linux)
      prepare_linux_icon
      ;;
    mac)
      prepare_mac_icon
      ;;
  esac
}

safe_remove_dir() {
  local target_dir="$1"
  if [[ ! -d "$target_dir" ]]; then
    return
  fi

  local resolved_build_root
  resolved_build_root="$(cd "$build_root" && pwd -P)"

  local resolved_target
  resolved_target="$(cd "$target_dir" && pwd -P)"
  case "$resolved_target" in
    "$resolved_build_root"/*)
      rm -rf -- "$resolved_target"
      ;;
    *)
      echo "拒绝删除非仓库 build 下的目录：$resolved_target" >&2
      exit 1
      ;;
  esac
}

clear_staging_dir() {
  local staging_dir="$repo_root/build/revier-analysis/current"
  mkdir -p "$build_root"
  safe_remove_dir "$staging_dir"
  mkdir -p "$staging_dir"
}

rust_target_for_linux() {
  case "$arch" in
    x64) echo "x86_64-unknown-linux-gnu" ;;
    arm64) echo "aarch64-unknown-linux-gnu" ;;
    *)
      echo "不支持的 Linux 架构：$arch" >&2
      exit 1
      ;;
  esac
}

cargo_with_duckdb_download() {
  echo "正在执行 Cargo：cargo $*"
  (
    cd "$repo_root"
    DUCKDB_DOWNLOAD_LIB=1 cargo "$@"
  )
}

find_duckdb_library() {
  local target_triple="$1"
  local library_name="$2"
  local download_root="$repo_root/target/duckdb-download/$target_triple"

  if [[ ! -d "$download_root" ]]; then
    echo "未找到 DuckDB 下载目录：$download_root" >&2
    exit 1
  fi

  local library_path
  library_path="$(find "$download_root" -type f -name "$library_name" -print -quit)"
  if [[ -z "$library_path" ]]; then
    echo "未找到 DuckDB 动态库：$download_root/**/$library_name" >&2
    exit 1
  fi

  echo "$library_path"
}

build_linux_rust_resources() {
  local target_triple
  target_triple="$(rust_target_for_linux)"
  local binary_path="$repo_root/target/$target_triple/release/revier-analysis"
  local staging_dir="$repo_root/build/revier-analysis/current"

  echo "正在构建 Linux Rust 分析资源，目标：$target_triple"
  cargo_with_duckdb_download build -p revier-analysis --release --target "$target_triple"

  if [[ ! -f "$binary_path" ]]; then
    echo "未找到 Rust CLI 产物：$binary_path" >&2
    exit 1
  fi

  local duckdb_library_path
  duckdb_library_path="$(find_duckdb_library "$target_triple" "libduckdb.so")"

  clear_staging_dir
  cp "$binary_path" "$staging_dir/revier-analysis"
  cp "$duckdb_library_path" "$staging_dir/libduckdb.so"
  chmod +x "$staging_dir/revier-analysis"
  echo "已暂存 Rust 分析资源到：$staging_dir"
}

build_mac_rust_resources() {
  if [[ "$universal" != true ]]; then
    echo "macOS 构建必须使用 --universal。" >&2
    exit 1
  fi

  if ! command -v lipo >/dev/null 2>&1; then
    echo "未找到 lipo，无法生成 macOS universal 产物。" >&2
    exit 1
  fi

  local x64_target="x86_64-apple-darwin"
  local arm64_target="aarch64-apple-darwin"
  local staging_dir="$repo_root/build/revier-analysis/current"
  local x64_binary="$repo_root/target/$x64_target/release/revier-analysis"
  local arm64_binary="$repo_root/target/$arm64_target/release/revier-analysis"

  echo "正在构建 macOS Rust 分析资源，目标：$x64_target"
  cargo_with_duckdb_download build -p revier-analysis --release --target "$x64_target"
  echo "正在构建 macOS Rust 分析资源，目标：$arm64_target"
  cargo_with_duckdb_download build -p revier-analysis --release --target "$arm64_target"

  if [[ ! -f "$x64_binary" || ! -f "$arm64_binary" ]]; then
    echo "未找到 macOS Rust CLI 单架构产物。" >&2
    exit 1
  fi

  local x64_duckdb
  local arm64_duckdb
  x64_duckdb="$(find_duckdb_library "$x64_target" "libduckdb.dylib")"
  arm64_duckdb="$(find_duckdb_library "$arm64_target" "libduckdb.dylib")"

  clear_staging_dir
  lipo -create "$x64_binary" "$arm64_binary" -output "$staging_dir/revier-analysis"
  lipo -create "$x64_duckdb" "$arm64_duckdb" -output "$staging_dir/libduckdb.dylib"
  chmod +x "$staging_dir/revier-analysis"

  echo "正在验证 macOS universal Rust CLI："
  lipo -info "$staging_dir/revier-analysis"
  echo "正在验证 macOS universal DuckDB 动态库："
  lipo -info "$staging_dir/libduckdb.dylib"
  echo "已暂存 Rust 分析资源到：$staging_dir"
}

run_rust_tests() {
  echo "正在运行 Rust workspace 测试。"
  cargo_with_duckdb_download test --workspace
  echo "Rust workspace 测试已完成。"
}

if [[ "$rust_test" == true ]]; then
  run_rust_tests
  exit 0
fi

prepare_icons

if [[ "$prepare_icons_only" == true ]]; then
  echo "仅准备图标模式已完成。"
  exit 0
fi

case "$platform" in
  linux)
    build_linux_rust_resources
    ;;
  mac)
    build_mac_rust_resources
    ;;
esac

if [[ "$package" == true ]]; then
  echo "已保留 --package 参数；Electron 打包接入将在任务 3 实现。"
fi
