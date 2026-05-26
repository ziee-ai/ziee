#!/usr/bin/env bash
# antd-diagnose.sh — run @ant-design/cli diagnostics against src-app/ui.
#
# Outputs:
#   docs/antd-diagnostics/<date>/doctor.txt — antd doctor checks
#   docs/antd-diagnostics/<date>/lint.txt   — deprecation / a11y / perf findings
#   docs/antd-diagnostics/<date>/usage.txt  — component import inventory
#
# Run from src-app/ui (or via `just antd-check` at repo root).
#
# CI gates on doctor passing all checks. Lint findings are advisory unless
# promoted to required (see .claude/FRONTEND_DEPS.md).

set -euo pipefail

DATE="${1:-$(date +%Y-%m-%d)}"
OUT_DIR="docs/antd-diagnostics/${DATE}"

mkdir -p "${OUT_DIR}"

echo "→ antd doctor"
npx --no-install antd doctor 2>&1 | tee "${OUT_DIR}/doctor.txt"

echo
echo "→ antd lint src"
npx --no-install antd lint src 2>&1 | tee "${OUT_DIR}/lint.txt"

echo
echo "→ antd usage src"
npx --no-install antd usage src 2>&1 | tee "${OUT_DIR}/usage.txt"

echo
echo "Reports in: ${OUT_DIR}/"
