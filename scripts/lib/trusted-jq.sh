#!/usr/bin/env bash
# scripts/lib/trusted-jq.sh — shared, PATH-shim-resistant jq resolver
# (cycle-006 mutants-ci-sharding, architecture-delta.md §6.11, round-7
# extraction). Sourced by BOTH scripts/check-ci-gate.sh and
# scripts/mutants-aggregate.sh so a future fix to this logic cannot land in
# one decision-path script and be forgotten in the other.
#
# NOT executable on its own — declares functions only, no `main`/dispatch.
# Meant to be `source`d, never invoked directly. Does NOT call
# `set -euo pipefail` itself: both current callers already set their own
# strict mode before sourcing this file, and a library file silently
# changing a caller's shell options on `source` would be a surprising,
# hard-to-audit side effect — the caller owns its own strict-mode posture.
#
# Extracted verbatim (behavior-preserving move, no logic change) from
# scripts/check-ci-gate.sh's own trusted_jq_dirs_for / is_trusted_jq_dir /
# resolve_trusted_jq (S-626-1 passes 59-61). See check-ci-gate.sh's git
# history for the full multi-pass research trail these functions'
# in-line doc comments (kept below, unabridged) narrate: WHY RUNNER_OS not
# GITHUB_ACTIONS, the pure-bash-dirname fix that closes a second PATH-shim
# vector on `dirname` itself, and the HONEST SCOPE paragraph on what this
# resolver can and cannot close — most importantly, GitHub-hosted runners'
# passwordless sudo means an attacker with EARLIER-STEP arbitrary execution
# in the SAME job does not need a PATH shim at all (`sudo cp /tmp/shim
# /usr/bin/jq` replaces the trusted binary in place) — this resolver closes
# the cheaper PATH-shim vector and is worth keeping, but "an attacker
# cannot forge the decision" is never an accurate description of what it
# achieves on its own.

# trusted_jq_dirs_for <runner_os> — single source of truth for the trusted
# system jq directory allowlist, keyed by GitHub Actions' own $RUNNER_OS
# value ("Linux" | "macOS" | "Windows"). One directory per line on
# stdout; empty output for an OS this guard does not (yet) model. See
# resolve_trusted_jq's "S-626-1 CI-BREAK-1" comment below for why this is
# a directory ALLOWLIST rather than a single-path pin or a writable-
# location denylist.
#
# Only Linux and macOS are populated — the only two RUNNER_OS values
# under which this script actually executes today (`ubuntu-latest` for
# `spec-guard`/`ci-gate`; `ubuntu-latest` AND `macos-latest` for the
# `#[cfg(unix)]` subprocess tests in `tests/ci_gate_completeness.rs`; no
# `windows-latest` job invokes this script — those tests are
# `#[cfg(unix)]`-gated and do not exist on that leg). A future job that
# runs this script on `windows-latest` must add a `Windows` entry here
# FIRST, deliberately, not discover the gap via another production CI
# break of the kind this whole function exists to prevent recurring.
#
# Linux entries are ONE physical directory under two names, not two
# independent trust grants (S-626-1 research pass, 2026-08-10; corrected
# ADV-P675-MEDIUM-001, 2026-08-10 — the original "first LTS with usrmerge"
# framing was wrong): `/bin` is a symlink to `/usr/bin` on `ubuntu-latest`
# (Ubuntu 24.04) — `/usr/bin/jq` and `/bin/jq` are the same inode. Ubuntu
# has shipped merged-`/usr` for new installs since 18.10 Cosmic, so 20.04
# and 22.04 LTS were ALSO already usrmerged; "24.04 is the first LTS with
# usrmerge" is false as a general Ubuntu-installations claim — the
# Rockcraft source for that phrasing scopes it to Ubuntu as a *base system
# inside rocks/container images*, not installations generally. The
# operative conclusion (one physical directory, no canonicalization
# needed) is unaffected: it holds on every LTS the GitHub-hosted runner
# fleet has offered. CONFIRM against Canonical/Debian docs still applies —
# to the merged-`/usr`-since-18.10 property, not the retracted "first LTS"
# claim. No equivalence gap exists for the `/bin` entry to close, and
# `/bin` is kept only as free, defensive redundancy against a future
# `PATH` reordering. Do NOT add `realpath`/`readlink` canonicalization to
# `is_trusted_jq_dir` to "resolve" this — there is nothing to resolve, and
# doing so would reintroduce an external-binary dependency on the decision
# path, undoing the point of `736fea28`.
#
# macOS entries — read as a COMPATIBILITY assertion, not a security one
# (S-626-1 research pass, 2026-08-10): `/opt/homebrew/bin` is (INFERRED,
# high confidence — Homebrew chowns its prefix to the installing user; no
# primary source states the hosted image's mode bits) owned by `runner`
# and writable without `sudo`, unlike `/usr/bin`/`/bin` above, whose
# root-only status rests on the same class of inference (ADV-P675-MEDIUM-002:
# neither half of this comparison is independently CONFIRMED against a
# primary source — see `.factory/research/ci-gate-shell-trust-assumptions-2026-08-10.md`
# Q1b/Q3b). This entry exists solely so the runner's own real Homebrew `jq`
# (Apple Silicon `macos-latest`) is accepted rather than falsely rejected
# — see the CI-BREAK-1 comment on `resolve_trusted_jq` below, the
# production break this entry was added to fix. It provides no meaningful
# security value on this leg (an earlier step can `cp` a shim there with
# no privilege escalation) but this is NOT the decision path — `ci-gate`
# and `spec-guard` both run on `ubuntu-latest`; `/opt/homebrew/bin` is
# reached only by the `test` job's `macos-latest` leg via the
# `#[cfg(unix)]` subprocess tests. Do NOT remove this entry — that
# reproduces CI-BREAK-1 verbatim.
trusted_jq_dirs_for() {
    case "$1" in
        Linux)
            printf '%s\n' "/usr/bin" "/bin"
            ;;
        macOS)
            printf '%s\n' "/usr/bin" "/bin" "/usr/local/bin" "/opt/homebrew/bin"
            ;;
        *)
            ;;
    esac
}

# is_trusted_jq_dir <runner_os> <dir> — true (rc=0) iff <dir> is exactly
# one of trusted_jq_dirs_for(<runner_os>)'s lines. Pure string
# comparison — no filesystem access — so every supported
# (runner_os, dir) pair is directly unit-testable with synthetic inputs
# in `run_jq_trust_self_test` without needing a real jq binary (or
# anything else) to actually exist at the candidate path.
is_trusted_jq_dir() {
    local os="$1" dir="$2" candidate
    while IFS= read -r candidate; do
        [ -z "${candidate}" ] && continue
        [ "${dir}" = "${candidate}" ] && return 0
    done <<EOF
$(trusted_jq_dirs_for "${os}")
EOF
    return 1
}

# resolve_trusted_jq — S-626-1 pass-59 (ADV-P59-LOW-001): every decision
# value this script produces is jq-derived, resolved via a bare `command -v
# jq` lookup. `$GITHUB_PATH` is a documented GitHub Actions mechanism that
# lets ANY earlier step in the same job prepend a directory to `PATH` for
# every subsequent step — a `jq` shim placed there and printing
# `"success"` for every `.result` query (or, worse, `command -v jq`
# resolving to a step-written shim after a `sudo`-writable-`/usr/bin`
# compromise) drives `evaluate_needs` to exit 0 while ALSO printing a
# manufactured `OK  <job> = success` line per job — a fabricated clean
# record, worse than a bare `|| true` appended to the run line (that
# vector is pinned by `tests/ci_gate_completeness.rs`'s M2-i; this one is
# a PATH/binary-identity vector, not a `run:` line vector, and sits
# entirely outside every existing byte-pin in that file). CLAUDE.md's
# round-13 IMPORTANT-2 note previously described this exposure as
# `$GITHUB_ENV` -> `BASH_ENV` (an environment-variable model); a second,
# independent channel is `PATH` -> WHICH BINARY RUNS, a different
# mechanism no env-surface pin or future YAML-parser rewrite would touch.
# INFERRED, not independently run against a live runner by this pass: the
# two channels are both live and do not supersede one another — CLAUDE.md's
# round-12 `env:`-key-set pins (M2-o, workflow-level) read `ci.yml` and so
# cannot see a `BASH_ENV` value an earlier step writes at runtime via
# `$GITHUB_ENV`, which still reaches this script's process environment
# (see the "WHY RUNNER_OS" ordering discussion below for the mechanism).
# This is not a new exposure — it requires the same earlier-step arbitrary-
# execution capability that already yields the `sudo cp .../usr/bin/jq`
# vector in the HONEST SCOPE paragraph below, so it adds nothing to the
# attacker's reach. It is also narrower than it first looks: a `BASH_ENV`
# shim defining a `jq` shell FUNCTION rather than a file is independently
# rejected here regardless — `command -v jq` then returns the bare string
# `jq`, which the absolute-path check just below refuses.
#
# Resolved once per `evaluate_needs` call and reused for every jq
# invocation inside it (rather than re-resolving per call), so a single
# TOCTOU-style PATH mutation mid-function cannot make different
# invocations within the same decision see different binaries.
#
# Enforced STRICTLY (the resolved path's directory must be a member of
# trusted_jq_dirs_for($RUNNER_OS) — see that function immediately below)
# whenever `RUNNER_OS` is non-empty. Outside that (local `--self-test`
# runs under any package manager's jq, where `RUNNER_OS` is normally
# unset), every path still goes through the absolute/executable check
# immediately below — the ONLY thing NOT enforced outside strict mode is
# directory membership in trusted_jq_dirs_for($RUNNER_OS).
#
# S-626-1 ADV-P61 follow-up research — WHY `RUNNER_OS`, NOT `GITHUB_ACTIONS`
# (this superseded an earlier revision of this fix that gated on
# `GITHUB_ACTIONS=true` OR `RUNNER_OS` non-empty, treating a mismatch as
# an error): a GitHub-supplied source-level fact search of `actions/runner`
# found `FileCommandManager.cs`'s `$GITHUB_ENV` write blocklist is
# verbatim `{ "NODE_OPTIONS" }` — no `GITHUB_*`/`RUNNER_*` prefix filter —
# so an earlier step in the SAME job can very likely overwrite
# `GITHUB_ACTIONS` to anything it wants (this specific claim is a
# high-confidence INFERENCE from source reading, not independently run
# against a live workflow). Do NOT lean on GitHub's own docs sentence,
# "You can't overwrite the value of the default environment variables
# named `GITHUB_*` and `RUNNER_*`" (Variables reference), as the reason
# either variable is trustworthy here — that claim is true IN EFFECT for
# `run:` steps, per the write-ordering mechanism below, but the
# `$GITHUB_ENV` write-time BLOCKLIST that would make it true by
# construction is verbatim `{ "NODE_OPTIONS" }`, with no `GITHUB_*`/
# `RUNNER_*` prefix filter — which is precisely why it does NOT by itself
# protect `GITHUB_ACTIONS`. Docs and source disagree at the blocklist
# layer; do not cite the docs sentence as if it settled the question.
#
# `RUNNER_OS`, by contrast, IS confirmed protected — and by a stronger,
# more general mechanism than "regeneration with no allowlist gap" alone
# (S-626-1 research pass, 2026-08-10, NEWLY-RESEARCHED against primary
# source): `actions/runner :: src/Runner.Worker/Handlers/ScriptHandler.cs`
# assembles a `run:` step's process environment by applying runtime
# contexts — every `RunnerContext` key, `RUNNER_OS` included — LAST, and
# the write ITSELF is unconditional once that per-context type check
# passes (ADV-P675-LOW-001: the line below binding `runtimeContext` was
# previously dropped from this quotation without an elision marker,
# reading as though the write were unconditional on context TYPE too —
# it is not; the filter is on which `ExpressionValues` entry qualifies,
# not on which keys get written once it does):
#   foreach (var context in ExecutionContext.ExpressionValues)
#       if (context.Value is IEnvironmentContextData runtimeContext && runtimeContext != null)
#           foreach (var env in runtimeContext.GetRuntimeEnvironmentVariables())
#               Environment[env.Key] = env.Value;
# — a plain assignment, not `TryAdd`, no null guard, no allowlist or
# denylist on which keys get written. This runs AFTER the inherited
# global environment (where `$GITHUB_ENV` writes accumulate) and the
# step's own `env:` are already in place, so it OVERWRITES — not merely
# "regenerates around" — whatever an earlier step's `$GITHUB_ENV` write,
# or a workflow/job/step `env:` block, set `RUNNER_OS` to. That is WRITE
# ORDERING, a stronger property than "no allowlist gap": it defeats
# `$GITHUB_ENV`, workflow `env:`, job `env:`, AND step `env:`
# simultaneously, all by the same single mechanism, not one channel at a
# time. Keying strict mode on `RUNNER_OS` alone (not `GITHUB_ACTIONS`, and
# not a mismatch between the two) uses the one signal in this pair that is
# actually known-trustworthy, for this reason.
#
# ADV-P61-LOW-003 (fixed, not just documented): this comment previously
# claimed "only an absolute, existing path is required" outside strict
# mode, but no such check existed — a relative path like `./jq` from cwd
# was accepted and executed unconditionally. The absolute+executable check
# a few lines below now makes this comment's claim true instead of
# weakening the comment to match the gap.
#
# HONEST SCOPE — WHAT THIS CANNOT CLOSE (S-626-1 ADV-P61 follow-up
# research, do not remove or soften this paragraph; see "THE TRAP" review
# guidance this story's own commit history already cites): both
# `ubuntu-latest` and `macos-latest` GitHub-hosted runners grant the job
# PASSWORDLESS sudo (CONFIRMED — GitHub Docs "GitHub-hosted runners" §
# "Administrative privileges"; corroborated by `actions/runner-images`
# issue #10484 showing `/etc/sudoers.d/runner` grants
# `runner ALL=(root) NOPASSWD:ALL`). An attacker with this story's modeled
# capability — arbitrary execution in an EARLIER STEP of the same job —
# does not need a PATH shim at all: `sudo cp /tmp/shim /usr/bin/jq`
# replaces the TRUSTED system binary in place. No directory allowlist,
# however refined, can detect that — the shim now IS the trusted path.
# This function's checks (dirname-immune directory allowlisting, absolute
# +executable, RUNNER_OS-keyed strict mode) close the cheaper PATH-shim
# vector and are worth keeping, but they do not achieve, and must never be
# described as achieving, "an attacker cannot forge the gate's decision"
# — that property is unreachable from inside this script. The actual
# control for the sudo-replacement vector is not running untrusted code in
# an earlier step of the `ci-gate` job at all, which is the `uses:`-value
# pinning question CLAUDE.md's CI Gate history already records as a
# knowing, deliberate scope decision (out of this story, which is about
# the pass/fail decision path once inputs are trusted, not supply-chain
# pinning of what runs before it).
#
# S-626-1 CI-BREAK-1 (real CI run 31406705091 on commit a17939e2): the
# ORIGINAL version of this function pinned exactly one path,
# `/usr/bin/jq` — correct for `ubuntu-latest` (where `spec-guard`/
# `ci-gate` actually run this script) but WRONG for `macos-latest`, where
# `tests/ci_gate_completeness.rs`'s `#[cfg(unix)]` subprocess tests
# invoke `evaluate_needs()` and inherit the runner's own real
# `GITHUB_ACTIONS=true` from the job environment — Homebrew installs
# `jq` at `/opt/homebrew/bin/jq` (Apple Silicon `macos-latest`, the
# current default) or `/usr/local/bin/jq` (Intel), never `/usr/bin/jq`.
# The single-path pin rejected the runner's own LEGITIMATE jq on every
# macOS `Test` leg, breaking `CI Gate` downstream (13/14 jobs succeeded
# for real; `Test (macos-latest)` failed on this false rejection;
# `CI Gate` correctly failed as a consequence of that failure). Why this
# was invisible locally before merging: strict mode only engages when
# `RUNNER_OS` is non-empty (ADV-P675-LOW-002, 2026-08-10: corrected from
# `GITHUB_ACTIONS=true`, stale since the S-626-1 re-key documented above),
# which is unset on a developer machine by default, so `--self-test` alone
# never reached this branch — see `run_jq_trust_self_test` below, added
# specifically to close that gap by exercising the strict branch
# deterministically regardless of where `--self-test` runs.
#
# Why a DIRECTORY ALLOWLIST keyed by $RUNNER_OS, not a denylist of
# writable locations ($GITHUB_WORKSPACE/$RUNNER_TEMP/$HOME/...): this
# repo's established CI-gate posture (see CLAUDE.md's CI-Gate history
# above `evaluate_needs`) is default-deny — an allowlist of known-trusted
# values that fails closed on anything new, not a denylist of known-bad
# locations that silently passes anything not yet enumerated (the exact
# allowlist-of-known-bad-values shape this whole guard exists to avoid
# repeating, per the module-level PURPOSE comment). A writable-location
# denylist is itself an open enumeration of the same kind that defeated
# the round-3/round-5 `if:`-legitimacy predicates documented above. A
# directory allowlist keyed by the runner's own reported OS is closed by
# construction: a jq shim in ANY directory not in that OS's list is
# rejected, including a location this comment's author never
# anticipated.
resolve_trusted_jq() {
    local resolved
    if ! resolved=$(command -v jq 2>/dev/null); then
        echo "ERROR: jq is required but was not found on PATH." >&2
        return 2
    fi

    # ADV-P61-LOW-003: applies in EVERY mode, not just strict — a resolved
    # jq must be an absolute, existing, executable path. `command -v`
    # ordinarily returns an absolute path for a PATH-resolved binary, but
    # if PATH contains a relative entry (e.g. "." — plausible if an
    # earlier step cd's somewhere and prepends it, or simply a developer's
    # own shell config) it returns exactly what would be executed,
    # relative-ness included. Reject that outright rather than silently
    # trusting whatever the current working directory happens to be.
    case "${resolved}" in
        /*) ;;
        *)
            echo "ERROR: jq resolved to a non-absolute path '${resolved}'." >&2
            echo "       Refusing to trust a jq found via a relative PATH" >&2
            echo "       entry (e.g. '.')." >&2
            return 2
            ;;
    esac
    if [ ! -x "${resolved}" ]; then
        echo "ERROR: jq resolved to '${resolved}', which is not an" >&2
        echo "       executable file." >&2
        return 2
    fi

    # S-626-1 ADV-P61-MEDIUM-002, RE-KEYED per follow-up research (see the
    # HONEST SCOPE / "WHY RUNNER_OS, NOT GITHUB_ACTIONS" comment block
    # above this function): strict mode is gated on `RUNNER_OS` alone.
    # `RUNNER_OS` is CONFIRMED regenerated by the runner's own context
    # machinery every step, with no override path found — unlike
    # `GITHUB_ACTIONS`, which prior research found very likely
    # attacker-writable via an earlier step's `$GITHUB_ENV` write (no
    # `GITHUB_*`/`RUNNER_*` prefix filter on that blocklist). An earlier
    # revision of this fix additionally gated on `GITHUB_ACTIONS=true` and
    # treated a mismatch against `RUNNER_OS` as an error; that is removed
    # here — `GITHUB_ACTIONS`'s value carries no security-relevant
    # information once `RUNNER_OS` alone is the trigger, so checking it
    # would add complexity without adding assurance.
    local os="${RUNNER_OS:-}"
    if [ -n "${os}" ]; then
        local dir
        # Pure-bash dirname (S-626-1 ADV-P61-HIGH-001): `dirname` is
        # itself resolved via PATH, so calling the external binary here
        # let the SAME $GITHUB_PATH shim that supplies a malicious `jq`
        # also supply a `dirname` that unconditionally prints a trusted
        # directory (e.g. "/usr/bin"), defeating the directory-allowlist
        # check below with a jq shim that was never actually in a
        # trusted location. Reproduced end-to-end pre-fix: a two-file
        # shim directory (`jq` + `dirname`, the latter always printing
        # `/usr/bin`) prepended to PATH made resolve_trusted_jq() accept
        # the shim under GITHUB_ACTIONS=true. Computing the directory
        # with bash parameter expansion instead closes this by
        # construction — there is no external `dirname` left on the
        # decision path to shim. See run_jq_trust_self_test's
        # "reject-dirname-shim..." check for the regression pin.
        if [ "${resolved}" = "${resolved#*/}" ]; then
            dir=""                       # no slash at all -> not absolute
        else
            dir="${resolved%/*}"
            [ -z "${dir}" ] && dir="/"   # "/jq" -> "/"
        fi
        if ! is_trusted_jq_dir "${os}" "${dir}"; then
            echo "ERROR: jq resolved to '${resolved}' (directory" >&2
            echo "       '${dir}') under RUNNER_OS='${os}', which is not" >&2
            echo "       one of the trusted system jq directories for" >&2
            echo "       that runner. Refusing to trust a jq binary found" >&2
            echo "       elsewhere on PATH inside a GitHub Actions job" >&2
            echo "       (possible PATH-prepend shim attack via" >&2
            echo "       \$GITHUB_PATH — see resolve_trusted_jq's comment" >&2
            echo "       in this file). Trusted directories for" >&2
            echo "       RUNNER_OS='${os}':" >&2
            local trusted_line
            while IFS= read -r trusted_line; do
                [ -z "${trusted_line}" ] && continue
                echo "         ${trusted_line}" >&2
            done <<EOF
$(trusted_jq_dirs_for "${os}")
EOF
            return 2
        fi
    fi
    printf '%s\n' "${resolved}"
}
