#!/usr/bin/env bash
set -euo pipefail

###############################################################################
# install-codex-cli.sh — 用户一键安装脚本
#
# 用法:
#   curl -fsSL https://lsh-oss-it-log.oss-cn-shanghai.aliyuncs.com/codex/install-codex-cli.sh | bash
#   curl -fsSL ... | bash -s -- --version 0.1.0
#   curl -fsSL ... | bash -s -- --install-dir ~/.local/bin
#
# 自动检测 OS / 架构，下载对应预编译包并安装到 /usr/local/bin (或指定目录)。
###############################################################################

CDN_BASE="https://lsh-oss-it-log.oss-cn-shanghai.aliyuncs.com/codex"
CLI_NAME="codex-cli"
DEFAULT_INSTALL_DIR="/usr/local/bin"
CONFIG_DIR="$HOME/.codex"

# ─── 工具函数 ────────────────────────────────────────────────────────────────

log_info()  { echo -e "\033[32m[INFO]\033[0m  $*"; }
log_warn()  { echo -e "\033[33m[WARN]\033[0m  $*"; }
log_error() { echo -e "\033[31m[ERROR]\033[0m $*" >&2; }

command_exists() {
    command -v "$1" &>/dev/null
}

# ─── 平台检测 ────────────────────────────────────────────────────────────────

detect_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Darwin) echo "darwin" ;;
        Linux)  echo "linux"  ;;
        MINGW*|MSYS*|CYGWIN*)
            echo "windows"
            ;;
        *)
            log_error "不支持的操作系统: $os"
            exit 1
            ;;
    esac
}

detect_arch() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)   echo "x86_64" ;;
        arm64|aarch64)   echo "arm64"  ;;
        *)
            log_error "不支持的 CPU 架构: $arch"
            exit 1
            ;;
    esac
}

get_platform_label() {
    local os arch label
    os="$(detect_os)"
    arch="$(detect_arch)"
    label="${os}-${arch}"

    # 验证是否为支持的平台组合
    case "$label" in
        darwin-arm64|linux-x86_64|windows-x86_64)
            ;;
        darwin-x86_64)
            log_error "macOS x86_64 (Intel) 暂不提供预编译包"
            log_error "如需支持请联系维护者或自行从源码编译"
            exit 1
            ;;
        linux-arm64)
            log_error "Linux arm64 暂不提供预编译包"
            log_error "如需支持请联系维护者或自行从源码编译"
            exit 1
            ;;
        *)
            log_error "不支持的平台: ${label}"
            exit 1
            ;;
    esac

    echo "$label"
}

# ─── 下载工具封装 ────────────────────────────────────────────────────────────

download() {
    local url="$1"
    local output="$2"

    if command_exists curl; then
        curl -fSL --progress-bar "$url" -o "$output"
    elif command_exists wget; then
        wget -q --show-progress "$url" -O "$output"
    else
        log_error "需要 curl 或 wget，请先安装其中之一"
        exit 1
    fi
}

# ─── 校验已安装版本 ──────────────────────────────────────────────────────────

check_existing() {
    local install_dir="$1"
    local bin_name="$2"
    local target="${install_dir}/${bin_name}"

    if [[ -f "$target" ]]; then
        local existing_version=""
        existing_version="$("$target" --version 2>/dev/null || true)"
        if [[ -n "$existing_version" ]]; then
            log_warn "检测到已安装: $existing_version"
            log_info "将覆盖安装..."
        fi
    fi
}

# ─── 安装逻辑 ────────────────────────────────────────────────────────────────

install() {
    local version="$1"
    local install_dir="$2"
    local platform os_name archive_name url tmpdir needs_sudo
    local bin_name="${CLI_NAME}"

    platform="$(get_platform_label)"
    os_name="$(detect_os)"
    log_info "检测到平台: ${platform}"

    # Windows: 二进制带 .exe 后缀
    if [[ "$os_name" == "windows" ]]; then
        bin_name="${CLI_NAME}.exe"
    fi

    # 构造下载 URL
    if [[ "$version" == "latest" ]]; then
        archive_name="codex-latest-${platform}.tar.gz"
    else
        archive_name="codex-${version}-${platform}.tar.gz"
    fi
    url="${CDN_BASE}/${archive_name}"

    log_info "下载 ${archive_name} ..."
    log_info "URL: ${url}"

    # 创建临时目录 (退出时自动清理)
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' EXIT

    # 下载
    download "$url" "${tmpdir}/${archive_name}"

    # 解压
    log_info "解压中..."
    tar -xzf "${tmpdir}/${archive_name}" -C "$tmpdir"

    # 查找解压出的二进制 (兼容带/不带 .exe)
    local extracted_bin=""
    if [[ -f "${tmpdir}/${CLI_NAME}.exe" ]]; then
        extracted_bin="${tmpdir}/${CLI_NAME}.exe"
    elif [[ -f "${tmpdir}/${CLI_NAME}" ]]; then
        extracted_bin="${tmpdir}/${CLI_NAME}"
    else
        log_error "归档包中未找到 ${CLI_NAME}，包可能已损坏"
        exit 1
    fi

    # 判断是否需要 sudo (Windows/MSYS 下不用 sudo)
    needs_sudo=false
    if [[ "$os_name" != "windows" ]]; then
        if [[ ! -d "$install_dir" ]]; then
            if ! mkdir -p "$install_dir" 2>/dev/null; then
                needs_sudo=true
            fi
        elif [[ ! -w "$install_dir" ]]; then
            needs_sudo=true
        fi
    fi

    check_existing "$install_dir" "$bin_name"

    # 安装二进制
    log_info "安装 ${bin_name} 到 ${install_dir}/ ..."
    if $needs_sudo; then
        log_warn "需要 sudo 权限写入 ${install_dir}"
        sudo mkdir -p "$install_dir"
        sudo cp "$extracted_bin" "${install_dir}/${bin_name}"
        sudo chmod +x "${install_dir}/${bin_name}"
    else
        mkdir -p "$install_dir"
        cp "$extracted_bin" "${install_dir}/${bin_name}"
        chmod +x "${install_dir}/${bin_name}"
    fi

    # 安装默认配置 (不覆盖已有配置)
    if [[ -f "${tmpdir}/config.toml" ]]; then
        mkdir -p "$CONFIG_DIR"
        if [[ -f "${CONFIG_DIR}/config.toml" ]]; then
            log_warn "配置文件已存在: ${CONFIG_DIR}/config.toml — 跳过 (不覆盖)"
        else
            cp "${tmpdir}/config.toml" "${CONFIG_DIR}/config.toml"
            log_info "默认配置已写入: ${CONFIG_DIR}/config.toml"
        fi
    fi

    # 检查 PATH
    if ! echo "$PATH" | tr ':' '\n' | grep -qx "$install_dir"; then
        log_warn "${install_dir} 不在你的 PATH 中"
        if [[ "$os_name" == "windows" ]]; then
            log_warn "请将 ${install_dir} 添加到系统 PATH 环境变量中"
        else
            log_warn "请将以下内容添加到你的 shell 配置文件 (~/.bashrc 或 ~/.zshrc):"
            echo ""
            echo "    export PATH=\"${install_dir}:\$PATH\""
            echo ""
        fi
    fi

    echo ""
    log_info '安装完成!'
    log_info "运行 '${bin_name} --help' 查看使用方法"
}

# ─── 参数解析 ────────────────────────────────────────────────────────────────

main() {
    local version="latest"
    local install_dir="$DEFAULT_INSTALL_DIR"

    while [[ $# -gt 0 ]]; do
        case "$1" in
            --version|-v)
                if [[ $# -lt 2 ]]; then
                    log_error "--version 需要一个参数"
                    exit 1
                fi
                version="$2"
                shift 2
                ;;
            --install-dir|-d)
                if [[ $# -lt 2 ]]; then
                    log_error "--install-dir 需要一个参数"
                    exit 1
                fi
                install_dir="$2"
                shift 2
                ;;
            --help|-h)
                cat <<EOF
用法: curl -fsSL <url>/install-codex-cli.sh | bash [-s -- OPTIONS]

选项:
  --version, -v <version>       指定版本号 (默认: latest)
  --install-dir, -d <path>      安装目录 (默认: /usr/local/bin)
  --help, -h                    显示帮助

支持平台:
  macOS     arm64 (M1/M2/M3/M4)
  Linux     x86_64
  Windows   x86_64 (通过 Git Bash / MSYS2 安装)

示例:
  # 安装最新版
  curl -fsSL ${CDN_BASE}/install-codex-cli.sh | bash

  # 安装指定版本
  curl -fsSL ${CDN_BASE}/install-codex-cli.sh | bash -s -- --version 0.1.0

  # 安装到自定义目录 (免 sudo)
  curl -fsSL ${CDN_BASE}/install-codex-cli.sh | bash -s -- -d ~/.local/bin
EOF
                exit 0
                ;;
            *)
                log_error "未知选项: $1"
                exit 1
                ;;
        esac
    done

    echo ""
    echo "  ╔══════════════════════════════════════╗"
    echo "  ║       Codex CLI Installer            ║"
    echo "  ╚══════════════════════════════════════╝"
    echo ""

    install "$version" "$install_dir"
}

main "$@"
