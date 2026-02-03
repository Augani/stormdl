#!/bin/bash
set -euo pipefail

# StormDL Uninstaller

BINARY_NAME="storm"
INSTALL_DIR="${INSTALL_DIR:-/usr/local/bin}"
CONFIG_DIR="${HOME}/.config/storm-dl"

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

info() { echo -e "${BLUE}[INFO]${NC} $1"; }
success() { echo -e "${GREEN}[SUCCESS]${NC} $1"; }
warn() { echo -e "${YELLOW}[WARN]${NC} $1"; }

main() {
    info "StormDL Uninstaller"
    echo ""

    if [ -f "${INSTALL_DIR}/${BINARY_NAME}" ]; then
        info "Removing ${INSTALL_DIR}/${BINARY_NAME}"
        if [ -w "${INSTALL_DIR}" ]; then
            rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        else
            sudo rm -f "${INSTALL_DIR}/${BINARY_NAME}"
        fi
        success "Binary removed"
    else
        warn "Binary not found at ${INSTALL_DIR}/${BINARY_NAME}"
    fi

    if [ -d "${CONFIG_DIR}" ]; then
        read -p "Remove config directory ${CONFIG_DIR}? [y/N] " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            rm -rf "${CONFIG_DIR}"
            success "Config directory removed"
        fi
    fi

    success "StormDL uninstalled"
}

main "$@"
