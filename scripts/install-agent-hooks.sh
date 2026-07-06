#!/usr/bin/env bash
# Installs the feature-lifecycle pre-push hook into this clone's shared git hooks.
# The hook only enforces on branches whose worktree contains a .lifecycle/ dir;
# all other pushes pass through untouched. Idempotent.
set -euo pipefail
ROOT="$(git rev-parse --git-common-dir)"
HOOK="$ROOT/hooks/pre-push"
cat > "$HOOK" <<'HOOKEOF'
#!/usr/bin/env bash
# feature-lifecycle enforcement: only for lifecycle branches (worktree has .lifecycle/)
TOP="$(git rev-parse --show-toplevel)"
# Merges to main are exempt: the lifecycle already validated on the feature
# branch (the branch push was gated); re-validating a merge context breaks
# the diff-vs-main reconciliation.
ONLY_MAIN=1
while read -r _local _lsha remote _rsha; do
  [ "$remote" = "refs/heads/main" ] || ONLY_MAIN=0
done
if [ "$ONLY_MAIN" = "1" ]; then exit 0; fi
if [ -d "$TOP/.lifecycle" ]; then
  CHECK="$TOP/.claude/lifecycle/lifecycle-check.mjs"
  if [ -f "$CHECK" ]; then
    node "$CHECK" --all --repo "$TOP" || {
      echo "pre-push: lifecycle-check --all FAILED — fix the gaps above before pushing." >&2
      exit 1
    }
  else
    echo "pre-push: .lifecycle/ present but lifecycle-check.mjs missing — run scripts/install-agent-hooks.sh from a clone with .claude/lifecycle committed." >&2
    exit 1
  fi
fi
exit 0
HOOKEOF
chmod +x "$HOOK"
echo "installed: $HOOK"
