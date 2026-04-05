# Release Workflow

Multi-stage release workflow for jr. Supports two release types:

- **Dev release** — pre-release from `develop`, optimistic next minor (e.g., `v0.3.0-dev.1`)
- **Stable release** — full release from `main` (e.g., `v0.3.0` or `v0.2.1`)

## Entry Point

Ask the user: "What type of release? (A) Dev release from develop, (B) Stable release to main"

- If **Dev release**: go to Dev Release Flow
- If **Stable release**: go to Stage 1

---

## Dev Release Flow

Dev versions use an optimistic minor bump from the last stable release. For example,
if the last stable release is `0.2.0`, the first dev release is `0.3.0-dev.1`. If the
final stable release turns out to be a patch (`0.2.1`), that's fine — dev tags are
ephemeral pre-releases and the stable version is chosen at release time.

1. Checkout `develop` and pull latest
2. Determine the next dev version:
   - Find the last stable tag: `git tag -l "v*" --sort=-v:refname | grep -v -- '-' | head -1`
   - Compute the optimistic next minor: bump the minor component, reset patch to 0
     (e.g., `v0.2.0` → `0.3.0`)
   - Find the latest dev tag for that base: `git tag -l "v0.3.0-dev.*" --sort=-v:refname | head -1`
   - If none exist, start at `dev.1`. Otherwise increment (e.g., `dev.1` → `dev.2`)
   - Confirm with the user: "Next dev version: vX.Y.Z-dev.N — proceed?"
3. Bump the version in `Cargo.toml` to the dev version (e.g., `0.3.0-dev.1`)
4. Run `cargo check` to update `Cargo.lock`
5. Run `cargo fmt --all`
6. Run `cargo clippy -- -D warnings` to verify no lint issues
7. Run `cargo test` to verify everything passes
8. Commit the version bump:
   ```
   git commit -m "chore: bump version to X.Y.Z-dev.N"
   ```
9. Tag (annotated — required to trigger the Release workflow and show as verified):
   ```
   git tag -a vX.Y.Z-dev.N -m "chore: release vX.Y.Z-dev.N"
   ```
10. Push commit and tag:
   ```
   git push origin develop
   git push origin vX.Y.Z-dev.N
   ```
11. Print: "Dev release vX.Y.Z-dev.N tagged and pushed. GitHub Actions will build and publish pre-release binaries."
12. Provide the releases URL

---

## Stage 1: Feature → develop

If on a feature branch (not `develop` or `main`):

1. Ensure all changes are committed
2. Push the branch if needed
3. Create a PR targeting `develop`
4. Show the PR URL
5. Ask: "PR created. Proceed to develop → main release, or stop here?"
   - If stop: done
   - If proceed: wait for the user to confirm the PR is merged, then continue

If already on `develop` or `main`, skip to Stage 2.

## Stage 2: develop → main

1. Checkout `develop` and pull latest
2. Generate a changelog summary from conventional commits since the last stable tag:
   ```
   git tag -l "v*" --sort=-v:refname | grep -v -- '-' | head -1
   ```
   Then:
   ```
   git log <last-stable-tag>..HEAD --oneline --pretty=format:"- %s"
   ```
3. Group commits by type (feat, fix, docs, chore, etc.)
4. Determine the stable version:
   - Show the changelog and ask: "Release as (A) minor vX.Y+1.0, (B) patch vX.Y.Z+1, or (C) custom version?"
   - The dev version in Cargo.toml used an optimistic minor bump, but the actual release
     version may differ (e.g., patch-only release)
5. Set `Cargo.toml` version to the chosen stable version (strip any `-dev.N` suffix)
6. Create a branch `release/vX.Y.Z` off develop
7. Create a PR from `release/vX.Y.Z` → `main` with the changelog as the PR body
8. Show the PR URL
9. Ask: "Release PR created. Proceed to tag after merge, or stop here?"
   - If stop: done
   - If proceed: wait for the user to confirm the PR is merged, then continue

## Stage 3: Tag & Release

1. Checkout `main` and pull latest
2. Verify the version in `Cargo.toml` matches the intended release (no pre-release suffix):
   ```
   grep '^version' Cargo.toml
   ```
3. Run `cargo check` to update `Cargo.lock`
4. Run `cargo fmt --all` to ensure formatting is correct
5. Run `cargo clippy -- -D warnings` to verify no lint issues
6. Run `cargo test` to verify everything passes
7. If any changes from steps 3-6, commit them on a branch, PR into `main`
8. After merge confirmation, create an annotated tag on main:
   ```
   git tag -a vX.Y.Z -m "chore: release vX.Y.Z"
   ```
9. Verify the tag matches `Cargo.toml`:
   ```
   TAG_VERSION=$(git describe --tags --exact-match | sed 's/^v//')
   CARGO_VERSION=$(grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
   ```
   If they don't match, stop and report the mismatch.
10. Push the tag to trigger the release workflow
11. Print: "Release vX.Y.Z tagged and pushed. GitHub Actions will build and publish binaries."
12. Provide the releases URL
13. Clean up dev tags for this release cycle:
    ```
    git tag -l "vX.Y.Z-dev.*" | xargs -I {} git push origin :refs/tags/{}
    git tag -l "vX.Y.Z-dev.*" | xargs git tag -d
    ```
14. Merge `main` back into `develop` and bump to next dev version:
    - Checkout `develop` and pull latest
    - Merge `main` into `develop`: `git merge main`
    - If conflicts, resolve and commit
    - Bump `Cargo.toml` to next optimistic dev version (e.g., `0.4.0` → `0.5.0-dev.1`)
    - Run `cargo check` to update `Cargo.lock`
    - Commit: `git commit -m "chore: bump version to X.Y.Z-dev.1"`
    - Push: `git push origin develop`

## Rules

- Never force push
- Never skip CI checks
- Always use PRs for changes to protected branches (`main`, `develop`)
- Tags can be pushed directly (they trigger the release workflow)
- Use conventional commit format for all commits
- Ask the user before every destructive or visible action
- If any step fails, stop and report the error — don't continue
- Dev tags go on `develop`, stable tags go on `main`
- Always verify tag/version match before pushing tags
