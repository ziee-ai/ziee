// A deliberately-broken stdio "MCP server" for the mcp-toolcollect-timeout
// regression test. It spawns and stays alive but NEVER speaks MCP: it reads and
// ignores stdin and never writes an `initialize` response, so the client's
// `().serve(transport)` handshake never completes. This reproduces the exact
// bug the fix addresses — a stdio server whose handshake hangs forever — so the
// test proves the handshake await is time-bounded (returns/skips) instead of
// stalling the whole chat send. Runs under the embedded Bun runtime (the `node`
// launcher resolves to bundled Bun), so no network/toolchain is required.
process.stdin.resume();
process.stdin.on("data", () => {});
process.stdin.on("end", () => {});
setInterval(() => {}, 1 << 30);
