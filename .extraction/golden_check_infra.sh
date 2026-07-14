#!/bin/bash
# Golden check (sdk-infra worktree): regen openapi/types for both surfaces,
# compare to baseline. types.ts -> byte-identical ; openapi.json -> canonical
# set-equality (jq -S).
set -e
export CARGO_TARGET_DIR=/data/pbya/ziee/tmp/sdk-infra-target
WT=/data/pbya/ziee/tmp/sdk-infra-wt
cd $WT/src-app
DB="postgresql://postgres:password@127.0.0.1:54321/ziee_build_4968222e"
echo ">>> regen UI spec"
DATABASE_URL="$DB" CONFIG_FILE=server/config/openapi-gen.yaml cargo run --bin ziee -- --generate-openapi ui/openapi >/tmp/regen-ui-infra.log 2>&1 || { echo "UI REGEN BUILD FAILED"; tail -30 /tmp/regen-ui-infra.log; exit 3; }
echo ">>> regen desktop spec"
DATABASE_URL="$DB" CONFIG_FILE=server/config/openapi-gen.yaml cargo run -p ziee-desktop -- --generate-openapi desktop/ui/openapi >/tmp/regen-desktop-infra.log 2>&1 || { echo "DESKTOP REGEN BUILD FAILED"; tail -30 /tmp/regen-desktop-infra.log; exit 3; }

B=$WT/.extraction/baseline
echo "=== GOLDEN RESULTS ==="
if diff -q ui/src/api-client/types.ts $B/types.ui.ts >/dev/null; then echo "types.ui.ts: BYTE-IDENTICAL"; else echo "types.ui.ts: *** DRIFT ***"; fi
if diff -q desktop/ui/src/api-client/types.ts $B/types.desktop.ts >/dev/null; then echo "types.desktop.ts: BYTE-IDENTICAL"; else echo "types.desktop.ts: *** DRIFT ***"; fi
if diff <(jq -S . ui/openapi/openapi.json) <(jq -S . $B/openapi.ui.json) >/dev/null; then echo "openapi.ui.json: CANONICALLY-EQUAL"; else echo "openapi.ui.json: *** CANONICAL DRIFT ***"; fi
if diff <(jq -S . desktop/ui/openapi/openapi.json) <(jq -S . $B/openapi.desktop.json) >/dev/null; then echo "openapi.desktop.json: CANONICALLY-EQUAL"; else echo "openapi.desktop.json: *** CANONICAL DRIFT ***"; fi
