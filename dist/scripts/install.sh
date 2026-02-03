#!/bin/bash
set -euo pipefail

REPO="Augani/stormdl"
BINARY_NAME="storm"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[OK]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }
error() { echo -e "${RED}[ERROR]${NC} $1"; exit 1; }

detect_platform() {
    local os arch

    case "$(uname -s)" in
        Linux*)  os="unknown-linux-musl" ;;
        Darwin*) os="apple-darwin" ;;
        *)       error "Unsupported operating system: $(uname -s)" ;;
    esac

    case "$(uname -m)" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)            error "Unsupported architecture: $(uname -m)" ;;
    esac

    echo "${arch}-${os}"
}

get_latest_version() {
    curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/'
}

main() {
    echo ""
    echo -e "${BLUE}╔═══════════════════════════════════════╗${NC}"
    echo -e "${BLUE}║${NC}     ${GREEN}StormDL Installer${NC}                  ${BLUE}║${NC}"
    echo -e "${BLUE}║${NC}     Lightning-fast downloads          ${BLUE}║${NC}"
    echo -e "${BLUE}╚═══════════════════════════════════════╝${NC}"
    echo ""

    if ! command -v curl &> /dev/null; then
        error "curl is required but not installed"
    fi

    info "Detecting platform..."
    PLATFORM=$(detect_platform)
    success "Platform: ${PLATFORM}"

    info "Fetching latest version..."
    VERSION=$(get_latest_version)
    if [ -z "$VERSION" ]; then
        error "Failed to fetch latest version"
    fi
    success "Latest version: ${VERSION}"

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/storm-${VERSION}-${PLATFORM}.tar.gz"
    info "Downloading from: ${DOWNLOAD_URL}"

    TMP_DIR=$(mktemp -d)
    trap "rm -rf ${TMP_DIR}" EXIT

    curl -fsSL "${DOWNLOAD_URL}" -o "${TMP_DIR}/storm.tar.gz" || error "Download failed"
    success "Download complete"

    info "Extracting..."
    tar -xzf "${TMP_DIR}/storm.tar.gz" -C "${TMP_DIR}"
    success "Extracted"

    info "Installing to ${INSTALL_DIR}..."
    if [ -w "${INSTALL_DIR}" ]; then
        mv "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
        chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    else
        warn "Elevated permissions required for ${INSTALL_DIR}"
        sudo mv "${TMP_DIR}/${BINARY_NAME}" "${INSTALL_DIR}/${BINARY_NAME}"
        sudo chmod +x "${INSTALL_DIR}/${BINARY_NAME}"
    fi

    success "Installed ${BINARY_NAME} to ${INSTALL_DIR}"

    echo ""
    echo -e "${GREEN}Installation complete!${NC}"
    echo ""
    echo "Usage:"
    echo "  storm <URL>                    # Download a file"
    echo "  storm <URL> -s 16              # Download with 16 segments"
    echo "  storm <URL> -o ~/Downloads     # Specify output directory"
    echo "  storm --help                   # Show all options"
    echo ""

    if command -v storm &> /dev/null; then
        info "Verifying installation..."
        storm --version
    fi
}

main "$@"
