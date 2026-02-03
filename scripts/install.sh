#!/bin/bash
set -euo pipefail

# StormDL Installer
# Usage: curl -fsSL https://raw.githubusercontent.com/augani/stormdl/main/scripts/install.sh | bash

REPO="augani/stormdl"
BINARY_NAME="storm"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

detect_os() {
    case "$(uname -s)" in
        Linux*)  echo "linux" ;;
        Darwin*) echo "darwin" ;;
        *)       error "Unsupported OS: $(uname -s)" ;;
    esac
}

detect_arch() {
    case "$(uname -m)" in
        x86_64|amd64)  echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *)             error "Unsupported architecture: $(uname -m)" ;;
    esac
}

get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | \
        grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
}

main() {
    info "StormDL Installer"
    echo ""

    OS=$(detect_os)
    ARCH=$(detect_arch)
    VERSION="${VERSION:-$(get_latest_version)}"

    if [ -z "$VERSION" ]; then
        error "Could not determine latest version"
    fi

    info "Detected: ${OS}/${ARCH}"
    info "Installing version: ${VERSION}"

    if [ "$OS" = "linux" ]; then
        TARGET="${ARCH}-unknown-linux-musl"
    else
        TARGET="${ARCH}-apple-darwin"
    fi

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/storm-${VERSION}-${TARGET}.tar.gz"

    info "Downloading from: ${DOWNLOAD_URL}"

    TMP_DIR=$(mktemp -d)
    trap "rm -rf ${TMP_DIR}" EXIT

    curl -fsSL "${DOWNLOAD_URL}" -o "${TMP_DIR}/storm.tar.gz"
    tar -xzf "${TMP_DIR}/storm.tar.gz" -C "${TMP_DIR}"

    if [ -w "$INSTALL_DIR" ]; then
        mv "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    else
        info "Requesting sudo to install to ${INSTALL_DIR}"
        sudo mv "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    chmod +x "${INSTALL_DIR}/${BINARY_NAME}"

    success "StormDL ${VERSION} installed successfully!"
    echo ""
    info "Run 'storm --help' to get started"

    if ! echo "$PATH" | grep -q "${INSTALL_DIR}"; then
        warn "${INSTALL_DIR} is not in your PATH"
        echo "  Add it with: export PATH=\"${INSTALL_DIR}:\$PATH\""
    fi
}

main "$@"
