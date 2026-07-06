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

  mkdir -p "$build_root"
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

cargo_run() {
  echo "正在执行 Cargo：cargo $*"
  (
    cd "$repo_root"
    cargo "$@"
  )
}

run_pnpm() {
  echo "正在执行 pnpm：pnpm $*"
  (
    cd "$repo_root"
    pnpm "$@"
  )
}

run_tauri_build() {
  local target_args=()

  case "$platform" in
    linux)
      local target_triple
      target_triple="$(rust_target_for_linux)"
      rustup_target_add "$target_triple"
      target_args=(--target "$target_triple")
      ;;
    mac)
      if [[ "$universal" == true ]]; then
        rustup_target_add "x86_64-apple-darwin"
        rustup_target_add "aarch64-apple-darwin"
        target_args=(--target universal-apple-darwin)
      else
        case "$arch" in
          x64) target_args=(--target x86_64-apple-darwin) ;;
          arm64) target_args=(--target aarch64-apple-darwin) ;;
        esac
        rustup_target_add "${target_args[1]}"
      fi
      ;;
  esac

  echo "正在执行 Tauri 构建：pnpm tauri build ${target_args[*]}"
  (
    cd "$repo_root"
    pnpm tauri build "${target_args[@]}"
  )
}

rustup_target_add() {
  local target_triple="$1"
  echo "正在确认 Rust target：$target_triple"
  rustup target add "$target_triple"
}

run_rust_tests() {
  echo "正在运行 Rust workspace 测试。"
  cargo_run test --workspace
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

if [[ "$package" == true ]]; then
  run_tauri_build
else
  run_pnpm build
fi
