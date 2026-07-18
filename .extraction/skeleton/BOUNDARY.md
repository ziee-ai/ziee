# Gate E11 — skeleton-server app-agnostic boundary proof

**Status: PASS** (Phase-1 capstone, plan §4 "Early skeleton second-consumer milestone")

`sdk/examples/skeleton-server` is a real, minimal, bootable server that links
**ONLY** the SDK platform crates (`ziee-core` + `ziee-framework`, `ziee-identity`
transitively) plus third-party crates, and **NO ziee domain crate** (chat /
memory / mcp / auth / control-mcp / …). It registers one module (`skeleton`)
via the real `#[distributed_slice(MODULE_ENTRIES)] ModuleEntry` mechanism,
exposing `GET /api/ping` → `"pong"` through the framework's
`create_modules` → `initialize_modules` → `build_api_router` pipeline, boots on
an ephemeral loopback port with a **lazy, never-connected** `PgPool`
(`connect_lazy` — router assembly needs no live DB), self-requests `/api/ping`,
asserts the body equals `"pong"`, prints `SKELETON OK`, and exits 0.

If a domain coupling ever leaks into the framework, this crate either stops
compiling or `cargo tree` shows a domain crate — that is the executable
definition of "app-agnostic," kept in SDK CI forever.

## Evidence

### 1. Build (exit 0)

```
$ cd sdk && cargo build -p skeleton-server
   Compiling ziee-core v0.0.0 (.../sdk/crates/ziee-core)
   Compiling ziee-identity v0.0.0 (.../sdk/crates/ziee-identity)
   Compiling ziee-framework v0.0.0 (.../sdk/crates/ziee-framework)
   Compiling skeleton-server v0.0.0 (.../sdk/examples/skeleton-server)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 9.28s
```

### 2. Boundary proof — `cargo tree -p skeleton-server -e normal | grep -iE 'ziee'`

```
skeleton-server v0.0.0 (.../sdk/examples/skeleton-server)
├── ziee-core v0.0.0 (.../sdk/crates/ziee-core)
└── ziee-framework v0.0.0 (.../sdk/crates/ziee-framework)
    ├── ziee-core v0.0.0 (.../sdk/crates/ziee-core) (*)
    └── ziee-identity v0.0.0 (.../sdk/crates/ziee-identity)
        └── ziee-core v0.0.0 (.../sdk/crates/ziee-core) (*)
```

Only `ziee-core`, `ziee-framework`, and `ziee-identity` (pulled transitively by
the framework) appear. The app crate `ziee` and every domain crate
(chat/memory/mcp/…) are **absent** → the platform boundary holds.

### 3. Run (exit 0)

```
$ cargo run -p skeleton-server
SKELETON OK
true-exit=0
```
