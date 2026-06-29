#!/usr/bin/env bash
# One-time host bootstrap for running ziee's test suites (integration + e2e +
# sandbox tiers) on a fresh Linux dev box / CI runner. Idempotent — safe to
# re-run. Baked in so test runs never need ad-hoc manual setup.
#
#   Usage:  sudo-capable user runs:  ./scripts/dev-test-bootstrap.sh
set -uo pipefail

echo "== ziee dev-test bootstrap =="

# 1. Sandbox host deps (bwrap + squashfuse, for code_sandbox tier4/tier6).
if command -v apt-get >/dev/null 2>&1; then
  sudo apt-get install -y bubblewrap squashfuse fuse3 || echo "  (apt install failed — install bubblewrap/squashfuse/fuse3 manually)"
fi

# 2. Allow unprivileged user namespaces so bwrap can set up its uid map.
#    On Ubuntu >=23.10 the AppArmor restriction (=1) makes bwrap fail with
#    "setting up uid map: Permission denied". Resets on reboot — re-run this.
if [ -e /proc/sys/kernel/apparmor_restrict_unprivileged_userns ]; then
  sudo sysctl -w kernel.apparmor_restrict_unprivileged_userns=0 || true
fi
sudo sysctl -w user.max_user_namespaces=15000 2>/dev/null || true

# 3. Docker group (testcontainers in integration tests + the e2e postgres
#    container reach /var/run/docker.sock). New logins inherit it; for the
#    current shell use `sg docker -c '<cmd>'`.
if ! id -nG | tr ' ' '\n' | grep -qx docker; then
  sudo usermod -aG docker "$(whoami)" && echo "  added $(whoami) to docker group (re-login or use 'sg docker -c')"
fi

# 4. Playwright browsers for BOTH UI workspaces (e2e). Also auto-ensured at
#    globalSetup time, but pre-installing here avoids the first-run delay.
for ui in src-app/ui src-app/desktop/ui; do
  if [ -f "$ui/package.json" ]; then
    ( cd "$ui" && npx playwright install --with-deps chromium ) || echo "  (playwright install failed for $ui)"
  fi
done

cat <<'NOTE'

== bootstrap done ==
Test-run env reference:
  - Integration:  source src-app/server/tests/.env.test  (or set DATABASE_URL +
    the LLM bridge vars below); run the sandbox tiers as a SEPARATE serial pass:
      cargo test --test integration_tests -- --test-threads=1 code_sandbox::tier4 code_sandbox::tier6
    and the rest fully parallel.
  - Real-LLM via a local bridge (no paid keys): set per-provider base URLs to a
    local OpenAI/Anthropic bridge (must include the path suffix, Anthropic = /v1):
      ANTHROPIC_API_KEY=sk-local  ANTHROPIC_BASE_URL=http://localhost:4000/v1
      (or the global ZIEE_TEST_LLM_BASE_URL fallback). Honored by both the
      integration test helpers and the e2e provider helper.
  - E2E: `npm run test:e2e` (browsers auto-ensured in globalSetup); PLAYWRIGHT_WORKERS=N
    for parallelism; each test self-isolates its own DB + backend.
NOTE
