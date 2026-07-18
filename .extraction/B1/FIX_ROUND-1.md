# Chunk B1 — FIX round 1

Findings from the C-3 blind audit (`LEDGER.jsonl`), triaged:

- Two findings were fixed **during** the move loop (before this round) and are
  recorded as the reasons for T-3/T-4:
  - Standalone doctest compile failure on the two illustrative ```rust``` doc
    examples → fenced ```ignore``` (T-3).
  - `unused_imports = "deny"` on the `ApiError` re-export → dropped from the
    ziee shim's `pub use` (T-4). `cargo check -p ziee` is green.

- One acknowledged, **pre-existing, NON-B1** finding (ordering-determinism,
  `openapi.json`): the JSON key order is not reproducible from the committed
  baseline even by pristine HEAD (357-line reorder with B1 fully reverted). It is
  semantically null (path-set + schema-set equal; `types.ts` byte-identical). It
  is NOT a B1 equivalence break to fix here — it is a defective/stale committed
  `openapi.json` baseline that needs re-capture, escalated to the orchestrator.
  No code fix in B1 can make a non-reproducible baseline reproducible.

- All other ledger entries are `status: ok` (verified-equivalent), requiring no fix.

No new B1-introduced defects were confirmed by the audit.

**New confirmed findings:** 0
