#!/bin/sh
# Install script for jr — a fast CLI for Jira Cloud
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh
#   curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh -s -- v0.3.0

set -eu

REPO="Zious11/jira-cli"
BINARY="jr"

err() {
    echo "Error: $1" >&2
    exit 1
}

# ── Check dependencies ────────────────────────────────────────────────

command -v curl >/dev/null 2>&1 || err "curl is required but not found. Install curl and try again."
command -v tar >/dev/null 2>&1 || err "tar is required but not found. Install tar and try again."

# ── Detect platform ───────────────────────────────────────────────────

OS="$(uname -s)"
ARCH="$(uname -m)"

case "${OS}" in
    Darwin)
        case "${ARCH}" in
            arm64)  TARGET="aarch64-apple-darwin" ;;
            x86_64) TARGET="x86_64-apple-darwin" ;;
            *)      err "Unsupported platform: ${OS} ${ARCH}. jr supports macOS and Linux (x86_64, ARM64)." ;;
        esac
        ;;
    Linux)
        case "${ARCH}" in
            aarch64) TARGET="aarch64-unknown-linux-gnu" ;;
            x86_64)  TARGET="x86_64-unknown-linux-gnu" ;;
            *)       err "Unsupported platform: ${OS} ${ARCH}. jr supports macOS and Linux (x86_64, ARM64)." ;;
        esac
        ;;
    *)
        err "Unsupported platform: ${OS} ${ARCH}. jr supports macOS and Linux (x86_64, ARM64)."
        ;;
esac

# ── Resolve version ───────────────────────────────────────────────────

if [ $# -gt 0 ]; then
    VERSION="$1"
else
    VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' \
        | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')"

    if [ -z "${VERSION}" ]; then
        err "Failed to fetch release info. Check your internet connection and try again."
    fi
fi

# ── Download and verify ───────────────────────────────────────────────

TARBALL="${BINARY}-${VERSION}-${TARGET}.tar.gz"
CHECKSUM_FILE="${TARBALL}.sha256"
BASE_URL="https://github.com/${REPO}/releases/download/${VERSION}"

TMP_DIR="$(mktemp -d 2>/dev/null || mktemp -d -t jr)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "Downloading ${BINARY} ${VERSION} for ${TARGET}..."

HTTP_CODE="$(curl -sSL -w "%{http_code}" -o "${TMP_DIR}/${TARBALL}" "${BASE_URL}/${TARBALL}")" || true
case "${HTTP_CODE}" in
    200) ;;
    404) err "Release ${VERSION} not found. See https://github.com/${REPO}/releases for available versions." ;;
    *)   err "Failed to download jr ${VERSION} (HTTP ${HTTP_CODE}). Check your internet connection and try again." ;;
esac

HTTP_CODE="$(curl -sSL -w "%{http_code}" -o "${TMP_DIR}/${CHECKSUM_FILE}" "${BASE_URL}/${CHECKSUM_FILE}")" || true
case "${HTTP_CODE}" in
    200) ;;
    404) err "Checksum file for ${VERSION} not found. The release may be incomplete. See https://github.com/${REPO}/releases." ;;
    *)   err "Failed to download checksum file (HTTP ${HTTP_CODE}). Check your internet connection and try again." ;;
esac

# Verify checksum (cd required — .sha256 contains bare filename)
cd "${TMP_DIR}"
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "${CHECKSUM_FILE}" >/dev/null 2>&1 \
        || err "Checksum verification failed. The download may be corrupted. Try again."
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "${CHECKSUM_FILE}" >/dev/null 2>&1 \
        || err "Checksum verification failed. The download may be corrupted. Try again."
else
    echo "Warning: No checksum tool found. Skipping verification."
fi

# ── Extract and install ───────────────────────────────────────────────

tar xzf "${TMP_DIR}/${TARBALL}" -C "${TMP_DIR}"

if [ -w /usr/local/bin ]; then
    INSTALL_DIR="/usr/local/bin"
else
    INSTALL_DIR="${HOME}/.local/bin"
    mkdir -p "${INSTALL_DIR}"
fi

cp "${TMP_DIR}/${BINARY}" "${INSTALL_DIR}/${BINARY}"
chmod +x "${INSTALL_DIR}/${BINARY}"

# ── Success ───────────────────────────────────────────────────────────

echo "Installed ${BINARY} ${VERSION} to ${INSTALL_DIR}/${BINARY}"

if [ "${INSTALL_DIR}" = "${HOME}/.local/bin" ]; then
    case ":${PATH}:" in
        *":${INSTALL_DIR}:"*) ;;
        *)
            echo ""
            echo "Add ~/.local/bin to your PATH:"
            echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
            echo ""
            ;;
    esac
fi

echo "Run \"jr init\" to get started."
