#!/usr/bin/env bash
# check-signing-workflow-injection.sh — YAML-structure-aware CI regression guard
#
# PURPOSE: Detects inline ${{ context }} expansions in run: script bodies inside
# jobs that have secrets or `contents: write` permissions in scope. These inline
# expansions are a CWE-77 shell injection risk when the context value is
# attacker-controlled (e.g. github.event.workflow_run.head_branch, inputs.*).
#
# TOOLING CHOICE: Uses Python 3 (standard library + PyYAML).
# Rationale: python3 is pre-installed on all GitHub Actions ubuntu/macos
# runners; PyYAML ships with the runner image — requires no CI install step.
# `yq` and `zizmor` are alternatives but require installation steps.
# `actionlint` is also an alternative but heavy and not pre-installed.
#
# YAML-STRUCTURE-AWARE: parses the YAML document and iterates jobs.*.steps[].run
# to extract run: block bodies. A naive line-oriented grep is INSUFFICIENT
# (cannot delimit run: scope, misses ${{ split across lines in block scalars).
#
# SCOPE: both sign-and-publish.yml and backfill-release.yml.
# Scope is COMPUTED STRUCTURALLY per-job — NOT from a hardcoded job-name list.
# A job is in scope when it meets ANY of:
#   (a) the job body contains any `secrets.*` reference (in any key under the job),
#   (b) the job-level `permissions.contents` is `write`, OR
#       the workflow-level `permissions.contents` is `write`, OR
#   (c) the job references a named `environment:` key.
#
# NOTE on criterion (b): only EXPLICIT `contents: write` is flagged. Jobs that
# simply inherit the workflow-default `contents: read` are NOT considered in
# scope on that criterion alone (they can still be in scope via (a) or (c)).
#
# ALLOWLIST (safe to inline — format-constrained values with no shell metacharacters):
#   github.sha, github.run_id, github.run_number,
#   github.repository, github.repository_owner
# Additionally, matrix.* and runner.* are safe (author/platform-controlled).
#
# DEFAULT-DENY rule: EVERY context expression not on the allowlist or in
# matrix.*/runner.* MUST be env-bound. This includes steps.*.outputs.* and
# needs.*.outputs.* — these can launder attacker-controlled values through
# multi-hop derivation chains that a guard cannot reliably trace.
#
# GUARD SCOPE NOTE: MUST NOT flag context expansions in env:, with:, or if:
# YAML keys — ONLY those textually inside run: script bodies.
#
# FAIL-CLOSED behaviour:
#   exit 1 — flagged violations found
#   exit 2 — YAML parse error, missing PyYAML, unreadable file,
#             or zero in-scope jobs detected in a workflow file
#             (sentinel for broken structural detection / renamed jobs)
#
# NEGATIVE FIXTURE: pass --self-test to run the built-in negative fixture
# (proves the detector is not a no-op per TD-VSDD-057 false-green prevention).
#
# USAGE:
#   scripts/check-signing-workflow-injection.sh            # scan hardened workflows
#   scripts/check-signing-workflow-injection.sh --self-test # run negative fixture

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

SIGN_WORKFLOW="${REPO_ROOT}/.github/workflows/sign-and-publish.yml"
BACKFILL_WORKFLOW="${REPO_ROOT}/.github/workflows/backfill-release.yml"

# Validate script syntax (catches accidental bash syntax errors in this wrapper)
bash -n "${BASH_SOURCE[0]}"

SELF_TEST_MODE=false
if [ "${1:-}" = "--self-test" ]; then
    SELF_TEST_MODE=true
fi

# ============================================================
# Python3 YAML-structure-aware scanner (inline, no temp file)
# ============================================================
run_python_guard() {
    python3 - "$@" <<'PYEOF'
import sys
import re
import os

# ---------------------------------------------------------------------------
# Preflight: explicit PyYAML import check (fail-closed with clear message).
# PyYAML is pre-installed on GitHub Actions ubuntu/macos runners; this check
# provides a clear error rather than an AttributeError if it's ever missing.
# ---------------------------------------------------------------------------
try:
    import yaml
except ImportError:
    print("ERROR: PyYAML not available. Install with: pip install pyyaml", file=sys.stderr)
    sys.exit(2)

# ---------------------------------------------------------------------------
# Allowlist: context values safe to inline (format-constrained — no shell
# metacharacters possible due to GitHub naming rules or fixed numeric format).
# ---------------------------------------------------------------------------
ALLOWLIST = frozenset({
    'github.sha',
    'github.run_id',
    'github.run_number',
    'github.repository',
    'github.repository_owner',
})


def is_high_risk(expression):
    """
    Returns True if the expression inside ${{ ... }} is high-risk (not on allowlist).

    DEFAULT-DENY policy per spec § "No inline context data in shell run-blocks":
    - Allowlist: github.sha, github.run_id, github.run_number,
                 github.repository, github.repository_owner
    - matrix.*  and runner.* are also safe (author/platform-controlled).
    - EVERYTHING ELSE must be env-bound — including steps.*.outputs.*
      and needs.*.outputs.* which can launder attacker-controlled values
      through multi-hop derivation chains.
    """
    expr = expression.strip()
    # Normalize internal whitespace (handles ${{ split across lines in block scalars)
    expr_normalized = re.sub(r'\s+', ' ', expr)
    # Explicit allowlist match
    if expr_normalized in ALLOWLIST:
        return False
    # matrix.* values are workflow-author-defined static literals (author-controlled)
    if re.match(r'^matrix\.[a-zA-Z0-9_.-]+$', expr_normalized):
        return False
    # runner.* values are runner-provided metadata (platform-controlled)
    if re.match(r'^runner\.[a-zA-Z0-9_.-]+$', expr_normalized):
        return False
    # Everything else is HIGH-RISK — default-deny.
    # This includes steps.*.outputs.* and needs.*.outputs.* because they can
    # launder attacker-controlled values (e.g. stable-sign.outputs.tag derived
    # from github.event.workflow_run.head_branch). A guard cannot reliably trace
    # cross-job derivation chains; the safe rule is to bind ALL non-allowlisted
    # expressions via step env:.
    return True


def find_inline_expressions(run_body):
    """
    Finds all ${{ ... }} expressions inside a run: script body.
    Returns list of (expression_normalized, is_flagged) tuples.
    """
    results = []
    for m in re.finditer(r'\$\{\{(.*?)\}\}', str(run_body), re.DOTALL):
        raw_expr = m.group(1)
        expr_normalized = re.sub(r'\s+', ' ', raw_expr).strip()
        flagged = is_high_risk(expr_normalized)
        results.append((expr_normalized, flagged))
    return results


def yaml_contains_secrets(obj, depth=0):
    """
    Recursively searches a YAML subtree for any secrets.* reference.
    Returns True if any string value matches the pattern ${{ secrets.* }}
    (dot notation) or ${{ secrets['NAME'] }} / ${{ secrets["NAME"] }}
    (index notation — L-PASS2-02).
    """
    if depth > 20:  # guard against pathological nesting
        return False
    if isinstance(obj, str):
        # Match both secrets.NAME (dot) and secrets['NAME'] / secrets["NAME"] (index)
        return bool(re.search(r'\$\{\{[^}]*secrets[\.\[]', obj))
    if isinstance(obj, dict):
        for v in obj.values():
            if yaml_contains_secrets(v, depth + 1):
                return True
    if isinstance(obj, list):
        for item in obj:
            if yaml_contains_secrets(item, depth + 1):
                return True
    return False


def job_is_in_scope(job_id, job_def, workflow_perms):
    """
    Determines whether a job is in scope for injection scanning.

    Criteria (any one is sufficient):
    (a) the job subtree contains any `secrets.*` reference in any key
    (b) job-level permissions.contents == 'write' OR
        workflow-level permissions.contents == 'write' (explicit only)
    (c) the job has a named `environment:` key

    Returns (bool, str) — (in_scope, reason_description)
    """
    if job_def is None:
        return False, ''

    # Criterion (a): secrets.* used anywhere in the job
    if yaml_contains_secrets(job_def):
        return True, 'uses secrets.*'

    # Criterion (b): explicit contents: write at job or workflow level.
    # Also catches permissions: write-all (string form) which grants write
    # to all scopes including contents (L-PASS2-01).
    job_perms = job_def.get('permissions', {}) or {}
    if isinstance(job_perms, str):
        # String form: 'write-all' grants all permissions including contents:write
        if job_perms in ('write-all', 'write'):
            return True, f'job-level permissions: {job_perms}'
    elif isinstance(job_perms, dict):
        if job_perms.get('contents') == 'write':
            return True, 'job-level permissions.contents: write'
    if isinstance(workflow_perms, str):
        # String form at workflow level: 'write-all' propagates to all jobs
        if workflow_perms in ('write-all', 'write'):
            return True, f'workflow-level permissions: {workflow_perms}'
    elif isinstance(workflow_perms, dict):
        if workflow_perms.get('contents') == 'write':
            return True, 'workflow-level permissions.contents: write'

    # Criterion (c): named environment
    if job_def.get('environment') is not None:
        return True, f"environment: {job_def.get('environment')!r}"

    return False, ''


def scan_workflow_doc(doc, filename):
    """
    Scans a parsed workflow YAML document.
    Computes in-scope jobs structurally (criteria a/b/c above).
    Returns (in_scope_job_count, run_block_count, total_expressions, flagged_list).
    flagged_list: list of (job_id, step_name, expr) tuples.
    """
    if not doc or 'jobs' not in doc:
        # Return a 5-tuple to match the normal return — callers always unpack 5
        # (M-PASS2-01: 4-tuple here caused ValueError at unpack sites).
        return 0, 0, 0, [], []

    workflow_perms = doc.get('permissions', {}) or {}
    run_block_count = 0
    total_expressions = 0
    flagged = []
    in_scope_jobs = []

    jobs = doc.get('jobs', {}) or {}
    for job_id, job_def in jobs.items():
        in_scope, reason = job_is_in_scope(job_id, job_def, workflow_perms)
        if not in_scope:
            continue

        in_scope_jobs.append((job_id, reason))

        steps = (job_def or {}).get('steps', []) or []
        for step in steps:
            if step is None:
                continue
            run_body = step.get('run')
            if run_body is None:
                continue
            run_block_count += 1
            step_name = step.get('name', '<unnamed step>')
            expressions = find_inline_expressions(str(run_body))
            for expr, is_flagged_expr in expressions:
                total_expressions += 1
                if is_flagged_expr:
                    flagged.append((job_id, step_name, expr))

    return len(in_scope_jobs), run_block_count, total_expressions, flagged, in_scope_jobs


def run_self_test():
    """
    Extended negative fixture: proves the detector fires on all required cases
    and does NOT fire on safe cases (TD-VSDD-057 false-green prevention).

    Assertions:
    1. Flags an in-scope github.event.* inline in a run: body.
    2. Does NOT flag env:/with:/if: sites (only run: bodies are checked).
    3. Does NOT flag allowlisted values (github.sha).
    4. Does NOT flag matrix.* / runner.* values.
    5. (NEW C-2) Flags a secrets-using job that would have been OMITTED by the
       old hardcoded job-name list — proving scope is structural.
    6. (NEW H-1) Flags a needs.*.outputs.* laundered value inline in a run: body.
    7. (M-PASS2-01) Empty/no-jobs document returns 5-tuple (no ValueError) and
       zero in-scope jobs — verifying the fail-closed path is clean.
    """
    fixture_yaml = """
permissions:
  contents: read
jobs:
  stable-sign:
    environment: release
    steps:
      - name: Violating step with github.event injection risk
        run: |
          TAG="${{ github.event.pull_request.title }}"
          echo "tag=$TAG"
      - name: Safe step with allowlisted value
        run: |
          echo "sha=${{ github.sha }}"
      - name: "Safe step: env-bound value does not count as run: injection"
        env:
          HEAD_BRANCH: ${{ github.event.workflow_run.head_branch }}
        run: |
          TAG="$HEAD_BRANCH"
      - name: "Safe step: matrix.* is exempt"
        run: |
          echo "target=${{ matrix.target }}"
      - name: "Safe step: runner.* is exempt"
        run: |
          echo "os=${{ runner.os }}"

  # NEW: a job that uses secrets but is NOT in the hardcoded list.
  # Old guard (SCOPED_JOBS_BY_FILE) would miss this entirely.
  # Structural detection MUST catch it via criterion (a).
  new-secrets-job:
    env:
      MY_SECRET: ${{ secrets.SOME_SECRET }}
    steps:
      - name: Violating step in previously-missed job
        run: |
          TAG="${{ needs.some-job.outputs.value }}"
          echo "tag=$TAG"

  # NEW: a job with needs.*.outputs.* laundered value inline — H-1.
  stable-homebrew:
    permissions:
      contents: write
    steps:
      - name: Violating step with laundered needs output
        run: |
          TAG="${{ needs.stable-sign.outputs.tag }}"
          echo "tag=$TAG"
      - name: Safe step with allowlisted github.repository
        run: |
          echo "repo=${{ github.repository }}"
"""
    print("=== NEGATIVE FIXTURE SELF-TEST ===")
    print("Fixture contains:")
    print("  [1] run: body with ${{ github.event.pull_request.title }}  [SHOULD FAIL]")
    print("  [2] run: body with ${{ github.sha }}                        [allowlisted, SHOULD PASS]")
    print("  [3] env: key with ${{ github.event.workflow_run.head_branch }} [env: not run:, SHOULD PASS]")
    print("  [4] run: body with ${{ matrix.target }}                     [matrix.*, SHOULD PASS]")
    print("  [5] run: body with ${{ runner.os }}                         [runner.*, SHOULD PASS]")
    print("  [6] new-secrets-job (not in old hardcoded list) run: with ${{ needs.*.outputs.* }} [SHOULD FAIL — C-2 structural scope]")
    print("  [7] stable-homebrew contents:write run: with ${{ needs.stable-sign.outputs.tag }} [SHOULD FAIL — H-1 laundered output]")
    print("  [8] stable-homebrew run: with ${{ github.repository }}     [allowlisted, SHOULD PASS]")
    print()

    doc = yaml.safe_load(fixture_yaml)
    in_scope_count, rb, te, flagged, in_scope_jobs = scan_workflow_doc(doc, '<self-test-fixture>')

    print(f"Structural scope detection found {in_scope_count} in-scope job(s):")
    for jid, reason in in_scope_jobs:
        print(f"  - {jid}: {reason}")
    print(f"Scanned {rb} run-block(s), {te} total ${{{{}}}} expression(s) in run: bodies")
    print()

    # Assertion A: exactly 3 flagged expressions
    # [1] github.event.pull_request.title (in stable-sign)
    # [6] needs.some-job.outputs.value (in new-secrets-job — C-2 structural scope)
    # [7] needs.stable-sign.outputs.tag (in stable-homebrew — H-1 laundered)
    expected_flagged = 3
    if len(flagged) != expected_flagged:
        print(f"FAIL: expected {expected_flagged} flagged expression(s), got {len(flagged)}")
        if not flagged:
            print("  CRITICAL: detector did NOT flag any violations — guard is a no-op!")
        else:
            for job_id, step_name, expr in flagged:
                print(f"  [FLAGGED] job={job_id} step='{step_name}': ${{{{ {expr} }}}}")
        sys.exit(1)

    # Assertion B: all three expected expressions were caught
    flagged_exprs = [expr for _, _, expr in flagged]
    checks = [
        ('event.pull_request.title', "github.event.* inline in stable-sign"),
        ('needs.some-job.outputs.value', "needs.*.outputs.* in new-secrets-job (C-2: structural scope catches previously-missed job)"),
        ('needs.stable-sign.outputs.tag', "needs.stable-sign.outputs.tag in stable-homebrew (H-1: laundered output)"),
    ]
    all_ok = True
    for needle, description in checks:
        found = any(needle in expr for expr in flagged_exprs)
        status = "PASS" if found else "FAIL"
        print(f"  [{status}] {description}")
        if not found:
            all_ok = False

    # Assertion C: in-scope count covers new-secrets-job (structural, not hardcoded)
    in_scope_ids = {jid for jid, _ in in_scope_jobs}
    if 'new-secrets-job' not in in_scope_ids:
        print("  [FAIL] new-secrets-job was NOT classified as in-scope (C-2 structural scope broken)")
        all_ok = False
    else:
        print("  [PASS] new-secrets-job classified in-scope via structural secrets detection (C-2)")

    if not all_ok:
        sys.exit(1)

    # Assertion D: empty / no-jobs document — fail-closed path (M-PASS2-01).
    # scan_workflow_doc must return a valid 5-tuple (no ValueError) with zero
    # in-scope jobs when given an empty doc or a doc lacking top-level `jobs:`.
    print()
    print("=== ASSERTION D: empty/no-jobs fail-closed path (M-PASS2-01) ===")
    empty_doc_cases = [
        ('null YAML (empty file)', None),
        ('doc with no jobs key', {'on': 'push', 'name': 'test'}),
        ('doc with empty jobs mapping', {'jobs': {}}),
    ]
    d_ok = True
    for case_label, test_doc in empty_doc_cases:
        try:
            result = scan_workflow_doc(test_doc, '<empty-test>')
            if len(result) != 5:
                print(f"  [FAIL] {case_label}: returned {len(result)}-tuple, expected 5 (M-PASS2-01)")
                d_ok = False
                continue
            in_scope_c, _, _, _, in_scope_j = result
            if in_scope_c != 0 or in_scope_j != []:
                print(f"  [FAIL] {case_label}: expected 0 in-scope jobs, got {in_scope_c}")
                d_ok = False
            else:
                print(f"  [PASS] {case_label}: 5-tuple returned, 0 in-scope jobs, no crash")
        except Exception as exc:
            print(f"  [FAIL] {case_label}: raised {type(exc).__name__}: {exc}")
            d_ok = False

    if not d_ok:
        print("FAIL: empty/no-jobs fail-closed path is broken (M-PASS2-01)")
        sys.exit(1)

    print()
    print(f"PASS: detector correctly flagged {len(flagged)} violation(s), "
          f"did NOT flag allowlisted/env-bound/matrix/runner values.")
    print("PASS: empty/no-jobs fail-closed path returns clean 5-tuple (M-PASS2-01).")
    sys.exit(0)


def main():
    args = sys.argv[1:]

    if '--self-test' in args:
        run_self_test()
        return  # run_self_test exits directly

    # Expect exactly 2 positional file arguments
    files = [a for a in args if not a.startswith('--')]
    if len(files) < 2:
        print("Usage: check-signing-workflow-injection.sh [sign-and-publish.yml] [backfill-release.yml]",
              file=sys.stderr)
        sys.exit(2)

    sign_workflow, backfill_workflow = files[0], files[1]

    total_run_blocks = 0
    total_expressions = 0
    all_flagged = []
    workflow_files = [sign_workflow, backfill_workflow]

    for filepath in workflow_files:
        fname = os.path.basename(filepath)
        try:
            with open(filepath, 'r') as f:
                raw = f.read()
        except OSError as e:
            print(f"ERROR: Cannot read {filepath}: {e}", file=sys.stderr)
            sys.exit(2)

        try:
            doc = yaml.safe_load(raw)
        except yaml.YAMLError as e:
            print(f"ERROR: YAML parse error in {filepath}: {e}", file=sys.stderr)
            sys.exit(2)

        in_scope_count, rb, te, flagged, in_scope_jobs = scan_workflow_doc(doc, fname)

        # Fail-closed: zero in-scope jobs is a sentinel for broken detection
        if in_scope_count == 0:
            print(f"ERROR: {fname}: structural scope detection found ZERO in-scope jobs.",
                  file=sys.stderr)
            print(f"  This is a sentinel for broken detection (e.g. renamed jobs, empty workflow).",
                  file=sys.stderr)
            print(f"  Each workflow that handles secrets/signing MUST have at least one in-scope job.",
                  file=sys.stderr)
            sys.exit(2)

        total_run_blocks += rb
        total_expressions += te
        for job_id, step_name, expr in flagged:
            all_flagged.append((fname, job_id, step_name, expr))

        scope_summary = ', '.join(f"{jid}({reason})" for jid, reason in in_scope_jobs)
        print(f"  {fname}: {in_scope_count} in-scope job(s), {rb} run-blocks, "
              f"{te} ${{{{}}}} expressions")
        print(f"    In-scope: {scope_summary}")

    print()
    print(f"Summary: scanned {total_run_blocks} run-blocks across {len(workflow_files)} files, "
          f"{total_expressions} total ${{{{}}}} expressions scanned, "
          f"{len(all_flagged)} inline high-risk expansion(s) flagged")

    if all_flagged:
        print()
        print("FAILURE: inline high-risk context expansions found in run: script bodies:")
        for fname, job_id, step_name, expr in all_flagged:
            print(f"  [{fname}] job={job_id}, step='{step_name}': ${{{{ {expr} }}}}")
        print()
        print("FIX: bind the value via step env: and reference as a quoted shell variable.")
        print("     Example:")
        print("       env:")
        print("         HEAD_BRANCH: ${{ github.event.workflow_run.head_branch }}")
        print("       run: |")
        print('         TAG="$HEAD_BRANCH"')
        print("     See docs/specs/fork-friendly-release-ops.md § 'No inline context data'")
        sys.exit(1)

    print("PASS: no inline high-risk expansions found in run: bodies of in-scope jobs.")
    sys.exit(0)


if __name__ == '__main__':
    main()
PYEOF
}

if [ "$SELF_TEST_MODE" = "true" ]; then
    run_python_guard --self-test
else
    echo "check-signing-workflow-injection: scanning signing workflow files..."
    run_python_guard "$SIGN_WORKFLOW" "$BACKFILL_WORKFLOW"
fi
