# DECISIONS — office-bridge pane RPC

Every input resolved up front so implementation runs nonstop. All resolvable by
codebase convention + Office.js API reality + the user's stated scope (the 5 pane
tools + Windows documentation); none need a fresh product call.

### DEC-1: How is `doc_full_name` (native list) mapped to a connected pane (`doc_key`)?
**Resolution:** each pane registers a `doc_key` from `Office.context.document.url`.
`call_pane` resolves in order: (a) a pane whose `doc_key` equals `doc_full_name` or
shares its basename (path formats differ between the native enumerate and Office.js),
(b) if exactly one pane is connected, that sole pane, (c) otherwise a typed
`OFFICE_PANE_NOT_CONNECTED`. The common case (one open task pane) always resolves.
**Basis:** convention — best-effort correlation; a suffix/basename match tolerates the
`/Users/x/Report.docx` (native) vs `file:///…/Report.docx` (Office.js) format gap.

### DEC-2: `call_pane` timeout?
**Resolution:** a 15s wall-clock timeout via `tokio::time::timeout` around the oneshot
recv, exposed as a `const` with a test-only shorter override path. Timeout →
`OFFICE_PANE_TIMEOUT` and the pending entry is removed.
**Basis:** convention — generous for an interactive Office.js op on a large document,
in the same tens-of-seconds band as other interactive tool waits; deterministic tests
use a short override so they don't sleep 15s.

### DEC-3: Which hosts support each pane op, and how is an unsupported combo reported?
**Resolution:** `get_selection` + `read_document` → Word + Excel (PowerPoint
best-effort selection); `add_comment` + `set_track_changes` + `get_tracked_changes` →
Word only. `taskpane.js` executes the op only on a supporting host and otherwise replies
with a JSON-RPC `error` the backend maps to `OFFICE_UNSUPPORTED_ON_HOST`. The existing
**native** PowerPoint pre-gate for `add_comment`/`set_track_changes` stays (fast path,
no round-trip).
**Basis:** codebase + API reality — mirrors the existing `OFFICE_UNSUPPORTED_ON_HOST`
capability model; comments / change-tracking are Word Office.js APIs, not Excel/PPT.

### DEC-4: `read_document` content shape per host?
**Resolution:** Word → `body.text` (plain text). Excel → the used range serialized as
TSV (rows by `\n`, cells by `\t`), capped at a sane character budget. Both returned in
the `{ text }` result the tool descriptor promises ("document body as plain text").
**Basis:** convention — TSV is the natural plain-text projection of a sheet; matches the
descriptor's "plain text" contract.

### DEC-5: Does the pane's `register` frame need auth beyond the WSS upgrade?
**Resolution:** no. The `/bridge` upgrade already validated the per-session token +
Origin, so a post-upgrade `register` frame is trusted; the broker keys the pane by its
socket/connection, not by a re-presented token.
**Basis:** codebase — mirrors DEC-6 of the original feature (socket authenticated at
upgrade; the frame-level `session_token` is belt-and-suspenders, not load-bearing for
routing).

### DEC-6: Correlation-id space — shared with the pane's ids or separate?
**Resolution:** the daemon allocates its own monotonic `AtomicU64` corr ids for
daemon→pane requests; responses are matched ONLY against the daemon's `PENDING` map.
A pane→daemon frame is classified as a request/notification by the presence of `method`
(never looked up in `PENDING`); a daemon→pane response is classified by `result`/`error`.
No cross-direction id collision is possible.
**Basis:** convention — JSON-RPC 2.0 disambiguates direction by method-vs-result/error;
`protocol.rs` already models both shapes.

### DEC-7: Pane→daemon events (`selection_changed`) — broadcast or keep as-is?
**Resolution:** keep the current behavior (debug-log, not consumed). The model reads the
selection via the `get_selection` **pull** (request/response), which fully covers the
requirement. Broadcasting spontaneous pane events to (multiple) ziee instances is the
future shared-broker concern and is explicitly out of scope here.
**Basis:** user scope — the requested work is the 5 request/response tools; event
fan-out is deferred and documented (broker design note).

### DEC-8: Concurrent ops to the same pane — serialize?
**Resolution:** no explicit serialization. Each op carries a unique corr id; the pane
processes `Word.run`/`Excel.run` batches independently and the broker correlates
replies by id, so interleaved in-flight ops are safe.
**Basis:** convention — Office.js run batches queue internally; id correlation handles
interleaving without a broker-side lock.

### DEC-9: New file `broker.rs` vs folding the broker into `server.rs`?
**Resolution:** a new `bridge/broker.rs` owning the two registries + `call_pane`;
`server.rs::handle_socket` calls into it. Keeps the socket I/O and the correlation
state separate + independently unit-testable (the broker unit tests need no socket).
**Basis:** codebase — mirrors the `auth.rs` (state module) vs `server.rs` (I/O) split
already in `bridge/`.
