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
# SCOPE: both sign-and-publish.yml and backfill-release.yml, restricted to jobs
# with secrets in scope or `contents: write` permissions.
# Named jobs in scope: stable-sign, alpha-sign (sign-and-publish.yml);
#                      sign, release (backfill-release.yml).
#
# ALLOWLIST (safe to inline — format-constrained values with no shell metacharacters):
#   github.sha, github.run_id, github.run_number,
#   github.repository, github.repository_owner
#
# GUARD SCOPE NOTE: MUST NOT flag context expansions in env:, with:, or if:
# YAML keys — ONLY those textually inside run: script bodies.
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

# ---------------------------------------------------------------------------
# Jobs in scope (have secrets or contents: write in their job scope).
# ---------------------------------------------------------------------------
SCOPED_JOBS_BY_FILE = {
    'sign-and-publish.yml': frozenset({'stable-sign', 'alpha-sign'}),
    'backfill-release.yml': frozenset({'sign', 'release'}),
}


def is_high_risk(expression):
    """
    Returns True if the expression inside ${{ ... }} is high-risk (not on allowlist).
    steps.*.outputs.* are NOT high-risk: they are workflow-internal values
    produced by previous steps in the same workflow, not attacker-controlled event
    context. Only event-sourced context (github.event.*, inputs.*, github.head_ref,
    github.ref_name, etc.) is considered high-risk.
    """
    expr = expression.strip()
    # Normalize internal whitespace (handles ${{ split across lines in block scalars)
    expr_normalized = re.sub(r'\s+', ' ', expr)
    # Explicit allowlist match
    if expr_normalized in ALLOWLIST:
        return False
    # steps.*.outputs.* are workflow-internal (not attacker-controlled event context)
    if re.match(r'^steps\.[a-zA-Z0-9_-]+\.outputs\.[a-zA-Z0-9_-]+$', expr_normalized):
        return False
    # needs.*.outputs.* are workflow-internal (job outputs)
    if re.match(r'^needs\.[a-zA-Z0-9_-]+\.outputs\.[a-zA-Z0-9_-]+$', expr_normalized):
        return False
    # matrix.* values are workflow-controlled (defined in the workflow's strategy.matrix)
    if re.match(r'^matrix\.[a-zA-Z0-9_.-]+$', expr_normalized):
        return False
    # runner.* values are runner-provided (not attacker-controlled)
    if re.match(r'^runner\.[a-zA-Z0-9_.-]+$', expr_normalized):
        return False
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


def scan_workflow_doc(doc, scoped_job_names, filename):
    """
    Scans a parsed workflow YAML document.
    Returns (run_block_count, total_expressions, flagged_list).
    flagged_list: list of (job_id, step_name, expr) tuples.
    """
    if not doc or 'jobs' not in doc:
        return 0, 0, []

    run_block_count = 0
    total_expressions = 0
    flagged = []

    jobs = doc.get('jobs', {}) or {}
    for job_id, job_def in jobs.items():
        if job_def is None:
            continue
        if job_id not in scoped_job_names:
            continue

        steps = job_def.get('steps', []) or []
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

    return run_block_count, total_expressions, flagged


def run_self_test():
    """
    Negative fixture: proves the detector fires on a deliberately injected
    violation (TD-VSDD-057 false-green prevention).
    """
    # Fixture: run: body with an inline high-risk expansion
    fixture_yaml = """
jobs:
  stable-sign:
    steps:
      - name: Violating step with injection risk
        run: |
          TAG="${{ github.event.pull_request.title }}"
          echo "tag=$TAG"
      - name: Safe step with allowlisted value
        run: |
          echo "sha=${{ github.sha }}"
      - name: "Safe step with env-bound value (env key is not run body)"
        env:
          HEAD_BRANCH: ${{ github.event.workflow_run.head_branch }}
        run: |
          TAG="$HEAD_BRANCH"
"""
    print("=== NEGATIVE FIXTURE SELF-TEST ===")
    print("Fixture contains:")
    print("  - run: body with ${{ github.event.pull_request.title }} [SHOULD FAIL]")
    print("  - run: body with ${{ github.sha }} [allowlisted, SHOULD PASS]")
    print("  - env: key with ${{ github.event.workflow_run.head_branch }} [env: not run:, SHOULD PASS]")
    print()

    doc = yaml.safe_load(fixture_yaml)
    rb, te, flagged = scan_workflow_doc(doc, frozenset({'stable-sign'}), '<self-test-fixture>')

    print(f"Scanned {rb} run-block(s), {te} total ${{{{}}}} expression(s) in run: bodies")

    # Verify: exactly one flagged (the event.pull_request.title) and two NOT flagged
    # (github.sha is allowlisted; HEAD_BRANCH env: key is not in run:)
    expected_flagged = 1
    if len(flagged) != expected_flagged:
        print(f"FAIL: expected {expected_flagged} flagged expression(s), got {len(flagged)}")
        if not flagged:
            print("  CRITICAL: detector did NOT flag the injected violation — guard is a no-op!")
        else:
            for job_id, step_name, expr in flagged:
                print(f"  [FLAGGED] job={job_id} step='{step_name}': ${{{{ {expr} }}}}")
        sys.exit(1)

    job_id, step_name, expr = flagged[0]
    print(f"  [FLAGGED] job={job_id} step='{step_name}': ${{{{ {expr} }}}}")

    # Verify the correct expression was flagged
    if 'event.pull_request.title' not in expr:
        print(f"FAIL: wrong expression flagged: ${{{{ {expr} }}}}")
        sys.exit(1)

    print()
    print(f"PASS: detector correctly flagged {len(flagged)} violation(s), "
          f"did NOT flag allowlisted/env-bound values.")
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

    workflow_scan_list = [
        (sign_workflow, SCOPED_JOBS_BY_FILE['sign-and-publish.yml']),
        (backfill_workflow, SCOPED_JOBS_BY_FILE['backfill-release.yml']),
    ]

    total_run_blocks = 0
    total_expressions = 0
    all_flagged = []

    for filepath, scoped_jobs in workflow_scan_list:
        fname = os.path.basename(filepath)
        with open(filepath, 'r') as f:
            doc = yaml.safe_load(f)

        rb, te, flagged = scan_workflow_doc(doc, scoped_jobs, fname)
        total_run_blocks += rb
        total_expressions += te
        for job_id, step_name, expr in flagged:
            all_flagged.append((fname, job_id, step_name, expr))

        print(f"  {fname}: {rb} run-blocks, {te} ${{{{}}}} expressions "
              f"(jobs scanned: {', '.join(sorted(scoped_jobs))})")

    print()
    print(f"Summary: scanned {total_run_blocks} run-blocks across {len(workflow_scan_list)} files, "
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
