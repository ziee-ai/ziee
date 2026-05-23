# Standalone GET-SSE — Gap Audit vs MCP TypeScript SDK

**Our impl:** `/home/pbya/projects/ziee-chat/src-app/server/src/modules/mcp/client/http.rs`
- `spawn_standalone_get_sse` at L396–495
- Call site: `connect()` L1536
- Abort: `disconnect()` L1544 / `abort_standalone_get_sse` L499

**Reference:** `/tmp/mcp-ts-sdk/packages/client/src/client/streamableHttp.ts` (770 lines, latest `main`)
Key methods: `_commonHeaders` L212–232, `_startOrAuthSse` L234–304, `_getNextReconnectionDelay` L312–325, `_scheduleReconnection` L333–366, `_handleSseStream` L368–478, `_send` 202-trigger L645–655.

Methodology: read the SDK in full, paired each behavior with our L396–495, and graded by what a real server (other MCP servers we'll interop with, and our own built-in handler) will actually observe on the wire.

---

## HIGH severity (will cause wire-level failure or wrong behavior against real servers)

### H1. Header casing: `MCP-Protocol-Version` vs `mcp-protocol-version`
- SDK L223: `headers['mcp-protocol-version'] = this._protocolVersion;` (lowercase). All-lowercase, consistent with HTTP/2 normalization.
- Ours (http.rs L410): `req = req.header("MCP-Protocol-Version", pv);` — mixed-case.
- **Severity:** LOW *on-wire* (HTTP/1.1 headers are case-insensitive per RFC 7230 § 3.2), but it's a **HIGH inconsistency** with the rest of our codebase — we use `MCP-Protocol-Version` everywhere (see http.rs L559, L1553), and SDKs that lowercase before string-compare won't notice. Keep as-is unless you discover a server doing case-sensitive matching (some hand-rolled Python servers do). Defer.
- **Fix:** none required; flag only.

### H2. SDK opens GET-SSE only on **202 Accepted** for `notifications/initialized`, gated by isInitializedNotification
- SDK L645–655: GET-SSE is started inside `_send`, conditional on (a) HTTP 202 *and* (b) the just-sent message being `notifications/initialized`.
- Ours: `connect()` always starts the GET-SSE after `do_initialize()` succeeds, regardless of the HTTP status the server returned for `notifications/initialized`. Our `send_notification` (L571) accepts any `is_success()` (200, 201, 202, 204) as OK, and even on 200 we still fire the GET.
- **Why this matters:** the SDK's gate is a deliberate signal — if a server responds **200 with a body** (rare but spec-legal for the JSON-only flavor), it's communicating "I do not stream; this was the response, no GET stream available." Eagerly opening a GET against such servers wastes a connection and reliably 405s. Against real servers that *do* stream, the gate doesn't matter — they'll 202. So the practical impact is: against JSON-only servers we'll always pay one extra 405 round-trip on every `connect()`.
- **Severity:** MEDIUM. Not a wire failure (we already handle 405 silently), but it's wasteful and noisy. The SDK pattern is cleaner.
- **Fix:** Either (a) accept the 405 cost as cheap (we silently exit on 405; cost is ~1 round-trip per connect), or (b) gate `spawn_standalone_get_sse` on the actual response code of the `notifications/initialized` POST. Option (a) is the lower-risk choice for now — document it.

### H3. We do not send `Last-Event-Id` on the GET; SDK does on resume
- SDK L246–248: `if (resumptionToken) headers.set('last-event-id', resumptionToken);`
- Ours: never sends `Last-Event-Id` on the GET. The header *is* threaded through our **POST-stream** resume path (`try_resume_sse`), but the standalone GET task starts fresh with no resume.
- **Spec wording (MCP § Transports → "Resumability and Redelivery"):** "Clients reconnecting after disconnection MAY send the `Last-Event-ID` header." Allowed but not required; since we don't reconnect-loop at all (see H4), there's nothing to resume from yet.
- **Severity:** MEDIUM. Tied to H4 — we won't need this until we add a reconnect loop. Track as follow-up.
- **Fix:** when adding the reconnect loop (H4), persist `last_event_id` from `id:` lines (we already extract them implicitly — see M3 — but don't store) and attach on next GET.

### H4. No reconnect loop on stream end / network error
- SDK L312–366 implements: initial 1000 ms, grow ×1.5, cap 30 s, max 2 retries, honoring server-pushed `retry:` field via `_serverRetryMs`. Reconnect fires on (a) `done` from the reader, (b) try/catch on stream error (L451–474). Reconnect schedule resets to attempt 0 each fresh stream open.
- Ours L483–485: stream end → debug log, exit. No backoff, no reconnect, no `retry:` honored.
- **Severity:** MEDIUM. Our doc comment (L391–395) acknowledges this and justifies it by "ephemeral session per tool call." That justification holds **today** (every `tool_call` POST opens its own stream and the standalone GET is short-lived), but it's brittle:
  - The very first server-initiated `sampling/createMessage` after a tool returns travels over the standalone GET. If that GET is dead from a transient blip, sampling silently never arrives.
  - The 405 case is the only documented exit; a 500 from a load-balancer hiccup terminates the stream forever for the life of the client.
- **Fix (concrete):** add a backoff loop around the existing body, parameterize attempts, honor `retry:`:
  ```rust
  // sketch
  let mut attempt = 0u32;
  let mut server_retry: Option<Duration> = None;
  loop {
      let result = run_one_get_sse(&stream_client, &url, ...).await;
      match result {
          GetSseOutcome::FourOhFive | GetSseOutcome::Aborted => break,
          GetSseOutcome::ServerClosedAfterPriming { last_id, retry } => {
              if attempt >= MAX_RETRIES { break; }
              server_retry = retry.or(server_retry);
              let delay = server_retry.unwrap_or(backoff(attempt));
              tokio::time::sleep(delay).await;
              attempt += 1;
              // reuse last_id on next GET
          }
          GetSseOutcome::Error => { /* same */ }
      }
  }
  ```
  Mirror SDK defaults: `initial=1s, grow=1.5, max=30s, max_retries=2`.

### H5. 401 handling — SDK refreshes auth and retries; ours just warns
- SDK L257–283: on 401 with `www-authenticate`, extracts resource metadata + scope, calls `authProvider.onUnauthorized()`, retries the GET once, then either succeeds or throws `UnauthorizedError`.
- Ours L434–438: any non-2xx (incl 401) → `tracing::warn!` + return.
- **Why this matters:** for external MCP servers using OAuth (the path you built `acquire_oauth_token` for), a token that expires while the client is connected will 401 the GET-SSE. We have `acquire_oauth_token` (http.rs L350) — but it's not wired into the GET path. Concretely: a server using OAuth with a 1-hour token + a long-lived client will lose the standalone GET stream after the first refresh boundary.
- **Severity:** MEDIUM-HIGH for OAuth deployments; LOW for our built-in code-sandbox (static JWT, no refresh).
- **Fix:** on 401 in `spawn_standalone_get_sse`, capture the `WWW-Authenticate` header, call `self.acquire_oauth_token(&www_auth).await`, and retry the GET once. The same `(isAuthRetry: bool)` pattern the SDK uses. Keep the retry budget at 1 to avoid runaway loops if the IdP itself is sick.

---

## MEDIUM severity (weird behavior in some flows)

### M1. We log received events at debug and drop them — no routing to elicitation / sampling / progress / cancellation
- SDK L417–432: every event with `data` and `event === 'message'` (or unset) is `JSON.parse`d, validated against `JSONRPCMessageSchema`, and dispatched via `this.onmessage(message)`. The transport-layer onmessage is set by the higher-level `Client` class which routes by method/id to the request handlers (sampling, elicitation, progress, etc.).
- Ours L477–480: collected `data:` lines logged and discarded.
- **Spec wording (MCP § Transports → "Sending Messages to the Server"):** "The server **MAY** send JSON-RPC requests and notifications on the GET-opened SSE stream." Known things that flow this way:
  - `notifications/progress` — for in-flight tool calls when the tool was POSTed via a separate POST (the progress notification's correlation token is `progressToken`, which lives in the original request's `params._meta.progressToken`).
  - `notifications/message` (logging) — once a server's `logging/setLevel` has been called.
  - `notifications/cancelled` — server-side cancellation of an in-flight request.
  - `sampling/createMessage` — server-initiated, expects a response.
  - `elicitation/create` — server-initiated, expects a response.
  - `roots/list_changed`, `tools/list_changed`, `resources/list_changed`, `prompts/list_changed`.
- **Today's usage in our app:** every tool call uses the in-POST stream (call_tool_with_sampling at L797, the `branch 2` at L1778), so sampling/elicitation/progress arrive over the POST-response stream, not the GET stream. **No consumer of the GET stream exists today.**
- **Severity:** MEDIUM — this is the "TODO" the inline comment already calls out (L443–445). Safe to leave dropped for now; **becomes a HIGH the day a server uses GET for any of the above** (most production MCP servers do for `roots/list_changed` after `roots/changed`).
- **Fix:** parse each event block to JSON, then call a new method `route_unsolicited_event(json)` that:
  - For `notifications/progress` with a `progressToken` → look up the in-flight tool call by token and forward via the same `sse_tx` the POST stream uses. Need a registry (small new map keyed by progressToken → mpsc::Sender). Out of scope for now but well-defined.
  - For `elicitation/create` → use the same elicitation registry path we already have at L976–1103.
  - For `sampling/createMessage` → same path as L1106. Requires `self.sampling_handler` to be cloneable into the task (it's `Arc<dyn SamplingHandler>` so this is trivial).
  - Anything else → debug-log + drop (current behavior is fine).

### M2. Hand-rolled SSE parser misses CRLF and multi-line `data:` semantics
- SDK uses `eventsource-parser/stream` (L16, L389) — RFC-compliant.
- Ours L459: `buf.find("\n\n")` only matches LF-LF. SSE spec (HTML Living Standard § "Server-sent events") allows event boundaries to be `\r\n\r\n`, `\n\n`, or `\r\r`. Real servers (especially behind nginx with `proxy_buffering off`) sometimes emit `\r\n` line terminators.
- Ours L468–473: `strip_prefix("data:").or_else(|| strip_prefix("data: "))` — only handles `data:` and `data: `, not `data:<no-space-then-content>` (we DO handle that — first arm), and joins multi-line data with `\n` correctly. **This part is fine.**
- **Severity:** MEDIUM. Will silently malfunction against a CRLF-terminating server. Likely server is FastAPI/uvicorn (LF), but Go's `net/http` chunked encoder + some reverse proxies inject CRLF.
- **Fix:** normalize input as it arrives. Two options:
  ```rust
  // Option A: replace CRLF with LF in the chunk before push_str
  let s = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
  buf.push_str(&s);
  // Then `buf.find("\n\n")` works.
  ```
  ```rust
  // Option B: search for any of \r\r, \n\n, \r\n\r\n.
  ```
  Option A is shortest. Also handle the `\r\r` corner (rare) by treating bare `\r` as `\n` post-conversion if you want full RFC.

### M3. We don't track `Last-Event-Id` (`id:` lines) from received events
- SDK L405–410: `if (event.id) { lastEventId = event.id; hasPrimingEvent = true; onresumptiontoken?.(event.id); }`. The id is then re-used as `last-event-id` on the next reconnect.
- Ours: ignores `id:` lines entirely.
- **Severity:** MEDIUM, blocked behind H4 — useless without a reconnect loop. Add together with H4.
- **Fix:** add `if let Some(id) = event_block.lines().find_map(|l| l.strip_prefix("id:")).map(str::trim) { last_event_id = Some(id.to_string()); }`. Then thread to the reconnect path.

### M4. We don't honor server-pushed `retry:` field
- SDK L389–394 captures retry via parser's `onRetry` callback → `_serverRetryMs` overrides backoff.
- Ours: drops `retry:` lines entirely.
- **Severity:** MEDIUM, also blocked behind H4. Add when adding the reconnect loop.

### M5. Bearer token captured ONCE at task spawn, never refreshed
- SDK L214–217: `_commonHeaders()` re-evaluates `authProvider.token()` on **every** GET attempt (and the SDK reconnects → fresh token).
- Ours L412–416: snapshot the bearer at task spawn, never re-read. For long-lived sessions, when the OAuth token expires the task will keep streaming bytes off the old connection (which the server doesn't authenticate on every event, so it actually keeps working until the connection is reset by some other cause — but on reconnect with a stale `Authorization` header, we'd 401 forever).
- **Severity:** MEDIUM, blocked behind H4 (single-shot stream → no refresh opportunity exists yet). When adding reconnect, re-read `self.current_bearer()` at the top of each loop iteration; or for an even better implementation, call `acquire_oauth_token` on 401 (see H5).

### M6. `disconnect()` race: aborts GET task *before* DELETE
- Our `disconnect()` L1544 aborts the GET task FIRST, THEN sends DELETE. SDK `close()` L510–518 aborts the abort controller (which the GET stream is subscribed to) and that's the only teardown — SDK has no DELETE in `close()`; explicit termination is the separate `terminateSession()`.
- **Severity:** LOW-MEDIUM. Our ordering is intentional and well-commented (L1541–1543) — avoids the DELETE racing against the GET stream's keepalive. The risk: between the abort and the DELETE arriving, the server may still consider the session alive and process events from the half-closed GET. In practice this is fine; just noting the asymmetry.
- **Fix:** no change. Keep the comment.

---

## LOW severity (cosmetic / future-improvement)

### L1. SDK preserves user-supplied `Accept` and merges; we overwrite
- SDK L241–243: takes existing `Accept` (from user `requestInit?.headers`), splits by comma, adds `text/event-stream`, dedupes. Allows the integrator to add custom Accept entries.
- Ours L405: `header("Accept", "text/event-stream")` — overwrites any user-supplied accept. We do allow custom `server.headers` (L294–305) which get applied as `default_headers` on the client — and `reqwest`'s `.header()` adds rather than replaces. So actually we should be fine. **Re-verify:** ours uses `.header()` not `.headers()`, and `reqwest::RequestBuilder::header` appends. ✓ No bug. Cosmetic only.

### L2. SDK has a `setProtocolVersion` for re-keying after handshake
- SDK L750. We have `set_protocol_version` (http.rs ~L500s). Equivalent.

### L3. Concurrency: spec says server returns 409 on 2nd GET; SDK doesn't special-case
- Spec § Transports → "Listening for Messages from the Server": "The server MAY return a 409 ... if there is already an active connection."
- Neither we nor the SDK special-case 409. Our code lumps it into "warn + exit." SDK throws `ClientHttpFailedToOpenStream`. Both reasonable. The only situation this triggers is if we have a bug elsewhere (two `connect()` calls without an intervening `disconnect`). Our `spawn_standalone_get_sse` does abort an existing task before spawning a new one (L488–493), so we shouldn't hit it in normal flow. Defer.

### L4. SDK supports `replayMessageId` (POST-resume-from-id)
- SDK L424–426 rewrites the resumed message's `id` to the replay value. Not applicable to the GET path; mentioned only because it's adjacent code in `_handleSseStream`.

### L5. Network/connect errors swallowed as `debug!` not `error!`
- Ours L420–423: `tracing::debug!` on connection failure. SDK fires `onerror` (L301, L453) which the higher-level Client routes to user-visible error. For our app where the GET is best-effort and a server may genuinely not offer it, `debug!` is OK; but `info!` on the first failure of a given session would aid operator debugging.

---

## Prioritized fix list (for committing on this branch)

1. **M2 (CRLF normalization)** — 2-line fix, no design questions, prevents silent malfunction against Go-server backends. **Do now.**
2. **M1 (route notifications/progress + sampling + elicitation)** — write the dispatcher even if no consumer wires it yet; future-proofs the moment a server uses GET. Stage in this branch behind a `route_unsolicited_event` private method that today only logs (matching current behavior), but with the parsing pipeline in place so M3/M4/H4 land easily later. **Do now (small).**
3. **H4 (reconnect loop with backoff) + M3 (last-event-id) + M4 (retry: honored) + M5 (bearer refresh)** — one cohesive change. **Defer to follow-up** — current single-shot behavior is documented and safe.
4. **H5 (401 → OAuth refresh + retry-once)** — wire into the GET path. Small, well-scoped. **Do when H4 lands** (so the retry has a backoff to ride on).
5. **H2 (gate GET on 202 from initialized POST)** — optional optimization, our 405 path is already cheap. **Document as known minor inefficiency, defer.**
6. **L5 (log level)** — bump first-failure debug→info. **Trivial, do now.**

The "do now" group is ~30 lines of change. The "defer" group (H4+H5+M3+M4+M5) is the long tail and should land before this client is exposed to non-built-in MCP servers.

---

## Summary of "is it safe to test now?"

**Yes, against our built-in code-sandbox MCP server.** Our handler returns 405 on GET (axum default for POST-only routes), our code exits silently on 405 (L428–433), and the rest of the path never runs in this configuration.

**Caveats for external-server testing:**
- Against any server that *does* serve GET-SSE: we'll connect, drain events, log them, and silently ignore any `roots/list_changed` / `notifications/progress` / sampling / elicitation that arrives. This is **silent feature loss**, not a crash. Audit-known.
- Against a CRLF-terminating server: parser will silently buffer forever (until disconnect). Fix M2 before testing externally.
- Against an OAuth server with token TTL < session lifetime: standalone GET dies at first refresh boundary, no recovery. Fix H4+H5 before relying on OAuth in production.
