# Install Script Design

## Goal

Provide a one-liner install command that downloads a pre-built `jr` binary from GitHub Releases, verifies its integrity, and places it on the user's PATH.

## Interface

```bash
# Install latest release
curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh

# Install specific version
curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh -s -- v0.3.0
```

## Supported Platforms

The script supports the same 4 targets built by the existing release workflow (`.github/workflows/release.yml`):

| `uname -s` | `uname -m` | Rust target |
|---|---|---|
| Darwin | arm64 | aarch64-apple-darwin |
| Darwin | x86_64 | x86_64-apple-darwin |
| Linux | aarch64 | aarch64-unknown-linux-gnu |
| Linux | x86_64 | x86_64-unknown-linux-gnu |

**Note:** macOS reports `arm64` from `uname -m`, while Linux reports `aarch64`. The script must match `arm64` for Darwin and `aarch64` for Linux — not the other way around.

Any other OS/arch combination exits with an error.

## Behavior

1. **Set shell options:** `set -eu` for strict error handling (no `pipefail` — not POSIX, fails on dash/ash)
2. **Check dependencies:** Verify `curl` and `tar` are available via `command -v`. Checksum tools (`sha256sum` or `shasum`) are checked later at verification time — both are present on all supported platforms, so missing tools are a soft warning rather than a hard fail.
3. **Detect platform:** `uname -s` for OS, `uname -m` for architecture → map to Rust target triple
4. **Resolve version:**
   - If argument provided (e.g., `v0.3.0`), use it directly. The `v` prefix is required — the script does not normalize (passing `0.3.0` will produce a "not found" error with a link to the releases page).
   - Otherwise, query `https://api.github.com/repos/Zious11/jira-cli/releases/latest` and extract the tag name
5. **Create temp directory:** `mktemp -d` with `trap 'rm -rf "$TMP_DIR"' EXIT` for cleanup
6. **Download artifacts:** Fetch `jr-{version}-{target}.tar.gz` and `jr-{version}-{target}.tar.gz.sha256` from GitHub Releases into temp dir
7. **Verify checksum:** `cd "$TMP_DIR"` then use `sha256sum -c` (Linux) or `shasum -a 256 -c` (macOS) to verify the tarball against the `.sha256` file. The `cd` is required because the `.sha256` file contains a bare filename (no path), and the check tools resolve filenames relative to the working directory.
8. **Extract binary:** `tar xzf` the tarball into `$TMP_DIR` to get the `jr` binary. The release tarball contains exactly one file named `jr` at the root — no directories or other files.
9. **Install binary:**
   - Try `/usr/local/bin` first — if writable (test with `[ -w /usr/local/bin ]`), install there
   - Otherwise create `~/.local/bin` if needed and install there
10. **Print result:**
    - Success: `Installed jr {version} to {path}`
    - If installed to `~/.local/bin` and it's not in `$PATH`, print a hint:
      ```
      Add ~/.local/bin to your PATH:
        export PATH="$HOME/.local/bin:$PATH"
      ```
    - Always end with: `Run "jr init" to get started.`

## Checksum Verification

The release workflow already produces `.sha256` files alongside each tarball. The install script detects which checksum tool is available:

```sh
cd "$TMP_DIR"  # Required — .sha256 file contains bare filename
if command -v sha256sum >/dev/null 2>&1; then
    sha256sum -c "$CHECKSUM_FILE"
elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 -c "$CHECKSUM_FILE"
else
    echo "Warning: No checksum tool found. Skipping verification."
fi
```

If verification fails, the script exits with an error. If no checksum tool is found, it warns but continues (both `sha256sum` and `shasum` are present on all supported platforms, so this fallback is defensive only).

## Error Messages

All errors follow the project convention of suggesting what to do next:

| Condition | Message |
|---|---|
| Unsupported OS/arch | `Unsupported platform: {os} {arch}. jr supports macOS and Linux (x86_64, ARM64).` |
| Missing curl | `curl is required but not found. Install curl and try again.` |
| Missing tar | `tar is required but not found. Install tar and try again.` |
| GitHub API unreachable | `Failed to fetch release info. Check your internet connection and try again.` |
| Version not found | `Release {version} not found. See https://github.com/Zious11/jira-cli/releases for available versions.` |
| Download failed | `Failed to download jr {version}. Check your internet connection and try again.` |
| Checksum mismatch | `Checksum verification failed. The download may be corrupted. Try again.` |

## File Layout

Single file at repo root:

```
install.sh    # Shell install script (#!/bin/sh, POSIX compatible)
```

The README already references this path: `curl -fsSL https://raw.githubusercontent.com/Zious11/jira-cli/main/install.sh | sh`

## What Doesn't Change

- Release workflow — artifacts are already in the right format
- README — install URL already documented (just move from "Coming soon" to active)
- No uninstall command — `rm $(which jr)` is sufficient

## Design Rationale

### Why not a package manager (Homebrew tap, crates.io)?

Those are separate distribution channels that should be added independently. The install script is the fastest path to "zero-dependency install" for users who don't have Rust or Homebrew.

### Why `/usr/local/bin` first?

It's the standard location for user-installed binaries on both macOS and Linux, and is already in `$PATH` on virtually all systems. On macOS with Homebrew, it's user-writable. Falling back to `~/.local/bin` avoids requiring sudo.

### Why POSIX sh and not bash?

Maximum portability. Some minimal Linux environments (Alpine, Docker images) have only `dash` or `ash` as `/bin/sh`. The script uses `set -eu` (POSIX) and avoids `set -o pipefail` (not POSIX — fails on dash/ash). All pipelines are structured so that failure of any stage is caught by `set -e`.

## Testing

The install script is a standalone shell file — no Rust tests. Verification:

- Manual test on macOS ARM64 (primary dev machine)
- Manual test on Linux x86_64 (CI runner or Docker)
- Test with explicit version: `sh install.sh v0.2.0`
- Test with nonexistent version: `sh install.sh v99.99.99` (should error)
- Test checksum failure: corrupt the tarball and verify the script catches it
