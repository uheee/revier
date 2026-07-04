#!/usr/bin/env bash
set -euo pipefail

platform="linux"
arch="x64"
package=false
universal=false
rust_test=false
prepare_icons_only=false

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

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
  local build_dir="$repo_root/build"
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
  local build_dir="$repo_root/build"
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

  rm -rf "$iconset_dir"
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

prepare_icons

if [[ "$prepare_icons_only" == true ]]; then
  echo "仅准备图标模式已完成。"
  exit 0
fi

echo "Rust 构建流程将在后续任务中补充。当前参数：platform=$platform，arch=$arch，package=$package，universal=$universal，rust_test=$rust_test"
