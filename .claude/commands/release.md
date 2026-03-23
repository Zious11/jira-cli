# Release Workflow

Multi-stage release workflow for jr. Each stage asks whether to proceed to the next or stop.

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
2. Generate a changelog summary from conventional commits since the last tag:
   ```
   git log $(git describe --tags --abbrev=0 2>/dev/null || git rev-list --max-parents=0 HEAD)..HEAD --oneline --pretty=format:"- %s"
   ```
3. Group commits by type (feat, fix, docs, chore, etc.)
4. Create a branch `release/vX.Y.Z` off develop (ask the user for the version number)
5. Create a PR from `release/vX.Y.Z` → `main` with the changelog as the PR body
6. Show the PR URL
7. Ask: "Release PR created. Proceed to tag after merge, or stop here?"
   - If stop: done
   - If proceed: wait for the user to confirm the PR is merged, then continue

## Stage 3: Tag & Release

1. Checkout `main` and pull latest
2. Bump the version in `Cargo.toml` to the version from Stage 2
3. Run `cargo check` to update `Cargo.lock`
4. Run `cargo fmt --all` to ensure formatting is correct
5. Run `cargo test` to verify everything passes
6. Commit the version bump on a branch, PR into `main`
7. After merge confirmation, tag `vX.Y.Z` on main
8. Push the tag to trigger the release workflow
9. Print: "Release vX.Y.Z tagged and pushed. GitHub Actions will build and publish binaries."
10. Provide the releases URL

## Rules

- Never force push
- Never skip CI checks
- Always use PRs (branches are protected)
- Use conventional commit format for all commits
- Ask the user before every destructive or visible action
- If any step fails, stop and report the error — don't continue
