#!/usr/bin/env bash
# dev-init.sh — one-command post-clone setup for the agent-kit consume model.
#
#   bash scripts/dev-init.sh        (or: just dev-init)
#
# Idempotent. Runs after a fresh clone (or any time the submodule / symlinks /
# hook go stale). Cross-platform: bash on Linux, macOS, and Windows git-bash.
#
# It (0) checks Windows symlink support, (1) populates the agent-kit submodule
# (the target of the .claude symlinks), (2) (re)creates the skill + lifecycle
# symlinks into it — enumerated FROM the submodule so the set never drifts, and
# (3) installs the feature-lifecycle pre-push hook via the submodule's shared
# installer (single source of truth). The symlinks are committed in git, so a
# fresh checkout already HAS them — they just dangle until the submodule is
# populated; step 2 is a self-heal for a moved/broken link.
set -euo pipefail
ROOT="$(git rev-parse --show-toplevel)"
cd "$ROOT"

# 0. Windows symlink support. Default Git on Windows checks committed mode-120000
#    symlinks out as PLAIN TEXT FILES (core.symlinks off without Developer Mode /
#    admin), which silently breaks the whole consume model. Detect it early and
#    tell the user exactly how to fix it.
if [ -e .claude/lifecycle ] && [ ! -L .claude/lifecycle ]; then
  echo "dev-init: WARNING — .claude/lifecycle is NOT a symlink (Git checked it out as a plain file)." >&2
  echo "  This happens on Windows without symlink support. Fix, then re-run dev-init:" >&2
  echo "    1) enable Developer Mode (or run your shell as Administrator)," >&2
  echo "    2) git config core.symlinks true" >&2
  echo "    3) git checkout -- .claude   (re-materialises the symlinks)" >&2
  git config core.symlinks true 2>/dev/null || true
  git checkout -- .claude 2>/dev/null || true
  if [ ! -L .claude/lifecycle ]; then
    echo "dev-init: still not a symlink after retry — enable Developer Mode/admin and re-run." >&2
    exit 1
  fi
  echo "dev-init: re-materialised the .claude symlinks." >&2
fi

# 1. populate the agent-kit submodule (skills/lifecycle symlink targets).
echo "dev-init: updating the agent-kit submodule…"
git submodule update --init agent-kit

# 2. (re)create the skill + lifecycle symlinks. The skill set is ENUMERATED from
#    agent-kit/skills/* so adding/removing a skill in the submodule can't drift
#    the link set. link() only touches a link that is missing or points
#    elsewhere; a correct committed symlink is left as-is. NOTE: link() rm -rf's
#    whatever is at the link path first — a real dir/file placed there by hand is
#    replaced (skills/lifecycle are symlink-only by design).
link() { # link <linkpath> <relative-target>
  local lp="$1" tgt="$2"
  if [ -L "$lp" ] && [ "$(readlink "$lp" 2>/dev/null)" = "$tgt" ]; then return 0; fi
  rm -rf "$lp"
  ln -s "$tgt" "$lp"
  echo "dev-init: linked $lp -> $tgt"
}
mkdir -p .claude/skills
if [ -d agent-kit/skills ]; then
  for d in agent-kit/skills/*/; do
    # guard the no-match case: without nullglob the glob stays literal
    # ('agent-kit/skills/*/') when the dir is empty — skip it so we never create
    # a bogus '.claude/skills/*' dangling symlink.
    [ -d "$d" ] || continue
    name="$(basename "$d")"
    link ".claude/skills/$name" "../../agent-kit/skills/$name"
  done
else
  echo "dev-init: WARNING — agent-kit/skills is empty (submodule not populated?)" >&2
fi
link ".claude/lifecycle" "../agent-kit/lifecycle"

# 3. install the feature-lifecycle pre-push hook via the submodule's shared
#    installer (single source; guarded so a missing installer fails clearly, not
#    with a bare exit 127).
INSTALLER="$ROOT/agent-kit/scripts/install-agent-hooks.sh"
if [ -f "$INSTALLER" ]; then
  bash "$INSTALLER"
else
  echo "dev-init: WARNING — $INSTALLER not found; pre-push hook NOT installed (run 'git submodule update --init agent-kit' first)." >&2
  exit 1
fi

echo "dev-init: done — agent-kit submodule + skill/lifecycle symlinks + pre-push hook are in place."
