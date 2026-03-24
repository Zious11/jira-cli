# Install Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Create `install.sh` that downloads a pre-built `jr` binary from GitHub Releases, verifies its checksum, and installs it to the user's PATH.

**Architecture:** Single POSIX sh script at repo root. Downloads tarball + sha256 file from GitHub Releases, verifies integrity, extracts binary, installs to `/usr/local/bin` or `~/.local/bin`. No dependencies beyond `curl` and `tar`.

**Tech Stack:** POSIX sh, curl, tar, sha256sum/shasum

**Spec:** `docs/superpowers/specs/2026-03-23-install-script-design.md`

---

## File Structure

| File | Responsibility | Change |
|------|---------------|--------|
| `install.sh` | Install script | Create |
| `README.md:16-27` | Install instructions | Move install script from "Coming soon" to active |

---

### Task 1: Create `install.sh`

**Files:**
- Create: `install.sh`

- [ ] **Step 1: Create the install script**

Create `install.sh` at the repo root with the following complete content:

```sh
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

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "${TMP_DIR}"' EXIT

echo "Downloading ${BINARY} ${VERSION} for ${TARGET}..."

curl -fSL "${BASE_URL}/${TARBALL}" -o "${TMP_DIR}/${TARBALL}" \
    || err "Failed to download jr ${VERSION}. Check your internet connection and try again. See https://github.com/${REPO}/releases for available versions."

curl -fSL "${BASE_URL}/${CHECKSUM_FILE}" -o "${TMP_DIR}/${CHECKSUM_FILE}" \
    || err "Failed to download checksum file. Check your internet connection and try again."

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
```

- [ ] **Step 2: Make the script executable**

```bash
chmod +x install.sh
```

- [ ] **Step 3: Test the script locally (dry run — verify platform detection and version resolution)**

```bash
sh install.sh v0.2.0
```

Expected: Downloads `jr-v0.2.0-aarch64-apple-darwin.tar.gz` (on macOS ARM64), verifies checksum, installs to `/usr/local/bin/jr`, prints success message.

- [ ] **Step 4: Test error case — nonexistent version**

```bash
sh install.sh v99.99.99
```

Expected: Error message: `Release v99.99.99 not found. See https://github.com/Zious11/jira-cli/releases for available versions.`

- [ ] **Step 5: Test error case — corrupted download (checksum mismatch)**

Create a fake tarball and sha256 file to test checksum verification:

```bash
mkdir -p /tmp/jr-test && cd /tmp/jr-test
echo "corrupted" > jr-v0.2.0-aarch64-apple-darwin.tar.gz
echo "0000000000000000000000000000000000000000000000000000000000000000  jr-v0.2.0-aarch64-apple-darwin.tar.gz" > jr-v0.2.0-aarch64-apple-darwin.tar.gz.sha256
sha256sum -c jr-v0.2.0-aarch64-apple-darwin.tar.gz.sha256 2>&1 || echo "EXPECTED: checksum mismatch detected"
rm -rf /tmp/jr-test
```

Expected: Checksum mismatch is detected. The install script's error path for this case prints: `Checksum verification failed. The download may be corrupted. Try again.`

- [ ] **Step 6: Test — verify installed binary works**

```bash
jr --version
```

Expected: Prints version matching what was installed.

- [ ] **Step 7: Commit**

```bash
git add install.sh
git commit -m "feat: add install.sh for one-liner binary installation

Shell script that downloads pre-built jr binary from GitHub Releases,
verifies SHA-256 checksum, and installs to /usr/local/bin or
~/.local/bin. Supports macOS + Linux (x86_64, ARM64) and optional
version pinning.

Closes the 'Install script (planned)' item in the README."
```

---

### Task 2: Update README install section

**Files:**
- Modify: `README.md:5-27`

- [ ] **Step 1: Update the README Install section**

Replace the current Install section (lines 5-27) with:

```markdown
## Install

### One-liner (macOS, Linux)

```bash
curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh
```

To install a specific version:

```bash
curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh -s -- v0.3.0
```

### From source

```bash
brew install rust   # if you don't have Rust installed
git clone https://github.com/Zious11/jira-cli.git
cd jira-cli
cargo install --path .
```

### Coming soon

```bash
# Homebrew tap (planned)
brew install zious11/tap/jr

# Crates.io (planned)
cargo install jr-cli
```
```

- [ ] **Step 2: Verify the README renders correctly**

```bash
head -35 README.md
```

Expected: Install script is listed first, "From source" second, Homebrew/crates.io under "Coming soon".

- [ ] **Step 3: Commit**

```bash
git add README.md
git commit -m "docs: promote install script to primary install method in README

Move curl|sh one-liner from 'Coming soon' to the top of the Install
section. Add version pinning example."
```
