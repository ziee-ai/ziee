#!/usr/bin/env bash
# Publish a generated `latest.json` to the GitHub Pages branch.
#
# Shared by the production release workflow and the local CI test. The target
# remote is parameterized (env GH_PAGES_REMOTE) so the test can point it at a
# local bare repo instead of GitHub.
#
# Usage:
#   scripts/updater/publish-pages.sh <path-to-latest.json>
#
# Env:
#   GH_PAGES_REMOTE  git remote/url to push to        (default: origin)
#   GH_PAGES_BRANCH  branch to publish on             (default: gh-pages)
#   GIT_AUTHOR_NAME  / GIT_AUTHOR_EMAIL committer id   (defaults provided)
set -euo pipefail

LATEST_JSON="${1:?usage: publish-pages.sh <latest.json>}"
REMOTE="${GH_PAGES_REMOTE:-origin}"
BRANCH="${GH_PAGES_BRANCH:-gh-pages}"
AUTHOR_NAME="${GIT_AUTHOR_NAME:-ziee-release-bot}"
AUTHOR_EMAIL="${GIT_AUTHOR_EMAIL:-release-bot@ziee.local}"

if [ ! -f "$LATEST_JSON" ]; then
  echo "publish-pages: '$LATEST_JSON' not found" >&2
  exit 1
fi

VERSION="$(node -e 'process.stdout.write(JSON.parse(require("fs").readFileSync(process.argv[1],"utf8")).version)' "$LATEST_JSON")"

workdir="$(mktemp -d)"
trap 'rm -rf "$workdir"' EXIT

# Clone just the pages branch if it exists; otherwise start it as an orphan.
if git clone --quiet --branch "$BRANCH" --single-branch "$REMOTE" "$workdir" 2>/dev/null; then
  echo "publish-pages: cloned existing '$BRANCH'"
else
  echo "publish-pages: '$BRANCH' not found — creating orphan branch"
  git clone --quiet "$REMOTE" "$workdir" 2>/dev/null || git init --quiet "$workdir"
  git -C "$workdir" checkout --quiet --orphan "$BRANCH"
  # Drop anything inherited from the default branch (if we cloned one).
  git -C "$workdir" rm -rqf . 2>/dev/null || true
fi

cp "$LATEST_JSON" "$workdir/latest.json"

git -C "$workdir" add latest.json
if git -C "$workdir" diff --cached --quiet; then
  echo "publish-pages: latest.json unchanged — nothing to publish"
  exit 0
fi

git -C "$workdir" \
  -c "user.name=$AUTHOR_NAME" \
  -c "user.email=$AUTHOR_EMAIL" \
  commit --quiet -m "chore(updater): publish latest.json for ${VERSION}"

# `origin` here is whatever we cloned from (REMOTE). For a freshly init'd
# orphan (no clone), wire the remote explicitly.
if ! git -C "$workdir" remote get-url origin >/dev/null 2>&1; then
  git -C "$workdir" remote add origin "$REMOTE"
fi
git -C "$workdir" push --quiet origin "$BRANCH"
echo "publish-pages: pushed latest.json (${VERSION}) to ${REMOTE} ${BRANCH}"
