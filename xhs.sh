#!/usr/bin/env bash
set -euo pipefail

###############################################################################
# xhs.sh — 开发者构建 & 分发脚本
#
# 用法:
#   ./xhs.sh compile   <version> [--target <target>]   编译 (默认当前平台)
#   ./xhs.sh compile   <version> --all                 交叉编译所有支持平台
#   ./xhs.sh compress  <version> [--target <target>]   打包 tar.gz
#   ./xhs.sh compress  <version> --all                 打包所有平台
#   ./xhs.sh upload    <version> [--target <target>]   上传 OSS
#   ./xhs.sh upload    <version> --all                 上传所有平台
#   ./xhs.sh full      <version> [--all]               编译 + 打包 + 上传
#   ./xhs.sh upload_scripts                             上传安装脚本和模型目录
#
# 支持平台:
#   aarch64-apple-darwin    (macOS ARM64)
#   x86_64-apple-darwin     (macOS x86_64)
#   aarch64-unknown-linux-gnu   (Linux ARM64)
#   x86_64-unknown-linux-gnu    (Linux x86_64)
#
# 环境依赖:
#   - rustup & cargo
#   - 交叉编译 Linux 目标需要安装 cross: cargo install cross
#   - ossutil (阿里云 OSS 命令行)
###############################################################################

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# 确保 cargo/rustc 在 PATH 中
# 1. rustup 标准位置: ~/.cargo/bin (proxy 二进制)
if [[ -d "$HOME/.cargo/bin" ]]; then
    export PATH="$HOME/.cargo/bin:$PATH"
fi
# 2. 如果 proxy 不存在，尝试直接引用 rustup 管理的当前 toolchain
if ! command -v rustc &>/dev/null && command -v rustup &>/dev/null; then
    RUSTUP_TOOLCHAIN_BIN="$(rustup which rustc 2>/dev/null | xargs dirname 2>/dev/null || true)"
    if [[ -n "$RUSTUP_TOOLCHAIN_BIN" && -d "$RUSTUP_TOOLCHAIN_BIN" ]]; then
        export PATH="${RUSTUP_TOOLCHAIN_BIN}:$PATH"
    fi
fi

CLI_NAME="codex-cli"
CARGO_BIN_NAME="codex"
OSS_PATH="oss://lsh-oss-it-log/codex"
OSS_CDN_BASE="https://lsh-oss-it-log.oss-cn-shanghai.aliyuncs.com/codex"

# 所有支持的编译目标
ALL_TARGETS=(
    "aarch64-apple-darwin"
    "x86_64-unknown-linux-gnu"
    "x86_64-pc-windows-gnu"
)

# ─── 工具函数 ────────────────────────────────────────────────────────────────

log_info()  { echo -e "\033[32m[INFO]\033[0m  $*"; }
log_warn()  { echo -e "\033[33m[WARN]\033[0m  $*"; }
log_error() { echo -e "\033[31m[ERROR]\033[0m $*"; }

# 检测当前机器的 Rust target triple
detect_host_target() {
    if ! command -v rustc &>/dev/null; then
        log_error "找不到 rustc，请确认已安装 Rust 工具链"
        log_error "  安装方法: curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
        log_error "  安装后运行: source \"\$HOME/.cargo/env\""
        exit 1
    fi
    rustc -vV | awk '/^host:/ { print $2 }'
}

# 将 Rust target triple 转换为简短的平台标签 (用于文件名)
# 例: aarch64-apple-darwin -> darwin-arm64
target_to_label() {
    local target="$1"
    case "$target" in
        aarch64-apple-darwin)        echo "darwin-arm64"   ;;
        x86_64-apple-darwin)         echo "darwin-x86_64"  ;;
        aarch64-unknown-linux-gnu)   echo "linux-arm64"    ;;
        x86_64-unknown-linux-gnu)    echo "linux-x86_64"   ;;
        x86_64-pc-windows-gnu)       echo "windows-x86_64" ;;
        *) log_error "未知目标平台: $target"; exit 1 ;;
    esac
}

# 判断是否需要使用 cross 来编译 (本机 target 用 cargo，否则用 cross)
needs_cross() {
    local target="$1"
    local host
    host="$(detect_host_target)"
    if [[ "$target" == "$host" ]]; then
        return 1  # 不需要 cross
    fi
    # macOS 上 darwin 之间可以用 cargo --target 直接编译 (universal)
    if [[ "$host" == *"-apple-darwin" && "$target" == *"-apple-darwin" ]]; then
        return 1
    fi
    return 0  # 需要 cross
}

# 获取编译产物路径
get_binary_path() {
    local target="$1"
    local suffix=""
    if [[ "$target" == *"-windows-"* ]]; then
        suffix=".exe"
    fi
    echo "$SCRIPT_DIR/codex-rs/target/${target}/release/${CARGO_BIN_NAME}${suffix}"
}

# 获取 tar.gz 文件名
get_archive_name() {
    local version="$1"
    local target="$2"
    local label
    label="$(target_to_label "$target")"
    echo "codex-${version}-${label}.tar.gz"
}

# ─── 编译 ──────────────────────────────────────────────────────────────────

ensure_target_installed() {
    local target="$1"
    if ! rustup target list --installed | grep -q "^${target}$"; then
        log_info "安装 Rust 编译目标: $target"
        rustup target add "$target"
    fi
}

compile_one() {
    local version="$1"
    local target="$2"
    local label nproc
    label="$(target_to_label "$target")"

    # 并行编译核心数 (预留部分核心给系统)
    nproc=10

    log_info "编译 ${CLI_NAME} [${label}] (target: ${target}, 并行: ${nproc} jobs) ..."

    ensure_target_installed "$target"

    if needs_cross "$target"; then
        # 使用 cross 进行 Linux 交叉编译
        if ! command -v cross &>/dev/null; then
            log_error "交叉编译 ${target} 需要 cross，请先安装: cargo install cross"
            exit 1
        fi
        (cd "$SCRIPT_DIR/codex-rs" && cross build -p "${CLI_NAME}" --release --target "$target" -j "$nproc")
    else
        (cd "$SCRIPT_DIR/codex-rs" && cargo build -p "${CLI_NAME}" --release --target "$target" -j "$nproc")
    fi

    local binary
    binary="$(get_binary_path "$target")"
    if [[ ! -f "$binary" ]]; then
        log_error "编译产物不存在: $binary"
        exit 1
    fi
    log_info "编译完成: $binary ($(du -h "$binary" | cut -f1))"
}

compile() {
    local version="$1"
    shift
    local targets=("$@")
    for t in "${targets[@]}"; do
        compile_one "$version" "$t"
    done
}

# ─── 打包 ──────────────────────────────────────────────────────────────────

compress_one() {
    local version="$1"
    local target="$2"
    local label archive binary tmpdir bin_name

    label="$(target_to_label "$target")"
    archive="$(get_archive_name "$version" "$target")"
    binary="$(get_binary_path "$target")"
    tmpdir="$(mktemp -d)"

    # Windows 二进制带 .exe 后缀
    if [[ "$target" == *"-windows-"* ]]; then
        bin_name="${CLI_NAME}.exe"
    else
        bin_name="${CLI_NAME}"
    fi

    log_info "打包 ${label} -> /tmp/${archive} ..."

    if [[ ! -f "$binary" ]]; then
        log_error "找不到编译产物: $binary — 请先执行 compile"
        exit 1
    fi

    cp "$binary" "${tmpdir}/${bin_name}"
    chmod +x "${tmpdir}/${bin_name}"

    # 重命名后 linker-signed 签名失效，需要先移除再重签，否则 macOS 会 SIGKILL (Code Signature Invalid)
    if [[ "$target" == *"-apple-darwin"* ]] && command -v codesign &>/dev/null; then
        log_info "重新签名 ${bin_name} (ad-hoc) ..."
        codesign --remove-signature "${tmpdir}/${bin_name}"
        codesign --force --sign - "${tmpdir}/${bin_name}"
    fi

    cp "$SCRIPT_DIR/config.toml.example" "${tmpdir}/config.toml"

    tar -czvf "/tmp/${archive}" -C "$tmpdir" "${bin_name}" config.toml
    rm -rf "$tmpdir"

    log_info "打包完成: /tmp/${archive} ($(du -h "/tmp/${archive}" | cut -f1))"
}

compress() {
    local version="$1"
    shift
    local targets=("$@")
    for t in "${targets[@]}"; do
        compress_one "$version" "$t"
    done
}

# ─── 上传 OSS ──────────────────────────────────────────────────────────────

upload_one() {
    local version="$1"
    local target="$2"
    local label archive oss_versioned oss_latest

    label="$(target_to_label "$target")"
    archive="$(get_archive_name "$version" "$target")"
    oss_versioned="${OSS_PATH}/${archive}"
    oss_latest="${OSS_PATH}/codex-latest-${label}.tar.gz"

    if [[ ! -f "/tmp/${archive}" ]]; then
        log_error "找不到归档包: /tmp/${archive} — 请先执行 compress"
        exit 1
    fi

    log_info "上传 ${archive} -> OSS ..."

    # 上传版本号归档
    ossutil cp "/tmp/${archive}" "$oss_versioned" --force
    ossutil set-acl "$oss_versioned" public-read

    # 上传 latest 链接 (覆盖)
    ossutil cp "/tmp/${archive}" "$oss_latest" --force
    ossutil set-acl "$oss_latest" public-read

    log_info "上传完成:"
    log_info "  版本: ${OSS_CDN_BASE}/${archive}"
    log_info "  最新: ${OSS_CDN_BASE}/codex-latest-${label}.tar.gz"
}

upload() {
    local version="$1"
    shift
    local targets=("$@")
    for t in "${targets[@]}"; do
        upload_one "$version" "$t"
    done
}

# ─── 上传脚本文件 ────────────────────────────────────────────────────────────

upload_scripts() {
    local install_script model_catalog oss_install_script oss_model_catalog

    install_script="$SCRIPT_DIR/install-codex-cli.sh"
    model_catalog="$SCRIPT_DIR/model_catalog.json"
    oss_install_script="${OSS_PATH}/install-codex-cli.sh"
    oss_model_catalog="${OSS_PATH}/model_catalog.json"

    if [[ ! -f "$install_script" ]]; then
        log_error "找不到文件: $install_script"
        exit 1
    fi
    if [[ ! -f "$model_catalog" ]]; then
        log_error "找不到文件: $model_catalog"
        exit 1
    fi

    log_info "上传 install-codex-cli.sh -> OSS ..."
    ossutil cp "$install_script" "$oss_install_script" --force
    ossutil set-acl "$oss_install_script" public-read

    log_info "上传 model_catalog.json -> OSS ..."
    ossutil cp "$model_catalog" "$oss_model_catalog" --force
    ossutil set-acl "$oss_model_catalog" public-read

    log_info "脚本文件上传完成:"
    log_info "  安装脚本: ${OSS_CDN_BASE}/install-codex-cli.sh"
    log_info "  模型目录: ${OSS_CDN_BASE}/model_catalog.json"
}

# ─── 参数解析 ──────────────────────────────────────────────────────────────

usage() {
    cat <<EOF
用法:
  $0 <command> <version> [options]
  $0 upload_scripts

命令:
  compile   编译 codex-cli
  compress  将编译产物打包为 tar.gz
  upload    上传 tar.gz 到阿里云 OSS
  full      执行完整流程 (compile + compress + upload)
  upload_scripts  上传 install-codex-cli.sh 和 model_catalog.json

选项:
  --all                编译所有支持的平台
  --target <triple>    指定 Rust target triple (可多次使用)

支持的 target:
  aarch64-apple-darwin       macOS ARM64 (M1/M2/M3)
  x86_64-unknown-linux-gnu   Linux x86_64
  x86_64-pc-windows-gnu      Windows x86_64

示例:
  $0 full 0.1.0 --all                     # 全平台构建 + 上传
  $0 compile 0.1.0                        # 只编译当前平台
  $0 compile 0.1.0 --target x86_64-unknown-linux-gnu  # 编译指定平台
  $0 full 0.1.0 --target aarch64-apple-darwin --target x86_64-apple-darwin
  $0 upload_scripts
EOF
    exit 1
}

# ─── 主入口 ────────────────────────────────────────────────────────────────

main() {
    if [[ $# -lt 1 ]]; then
        usage
    fi

    local command="$1"
    shift

    if [[ "$command" == "upload_scripts" ]]; then
        if [[ $# -ne 0 ]]; then
            log_error "upload_scripts 不接受额外参数"
            usage
        fi
        log_info "命令: ${command}"
        echo ""
        upload_scripts
        log_info '全部完成!'
        return
    fi

    if [[ $# -lt 1 ]]; then
        usage
    fi

    local version="$1"
    shift

    # 先验证 rustc 可用 (在主 shell 中执行，错误信息不会被吞掉)
    detect_host_target >/dev/null

    # 解析目标平台列表 (这里不再需要子 shell)
    local -a targets=()
    local all_flag=false

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --all)
                all_flag=true
                shift
                ;;
            --target)
                if [[ $# -lt 2 ]]; then
                    log_error "--target 需要一个参数"
                    exit 1
                fi
                targets+=("$2")
                shift 2
                ;;
            *)
                log_error "未知选项: $1"
                usage
                ;;
        esac
    done

    if $all_flag; then
        targets=("${ALL_TARGETS[@]}")
    elif [[ ${#targets[@]} -eq 0 ]]; then
        targets=("$(detect_host_target)")
    fi

    if [[ ${#targets[@]} -eq 0 ]]; then
        log_error "无法确定编译目标平台"
        exit 1
    fi

    log_info "命令: ${command}  版本: ${version}"
    log_info "目标平台: ${targets[*]}"
    echo ""

    case "$command" in
        compile)
            compile "$version" "${targets[@]}"
            ;;
        compress)
            compress "$version" "${targets[@]}"
            ;;
        upload)
            upload "$version" "${targets[@]}"
            ;;
        full)
            compile  "$version" "${targets[@]}"
            compress "$version" "${targets[@]}"
            upload   "$version" "${targets[@]}"
            ;;
        *)
            usage
            ;;
    esac

    log_info '全部完成!'
}

main "$@"
