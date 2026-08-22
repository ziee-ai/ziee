# Design — model-facing tool argument contracts

This is the named design source for the `tool-argument-contracts` lifecycle. It
exists because the two defects being fixed are **one** failure mode, and no prior
doc states the rule that both violate.

## The rule

> **A schema advertised to the model is a promise the server keeps.**

A built-in MCP tool descriptor tells the model exactly what the server accepts:
the property names, their nesting, their enums. Every one of those statements is
either enforced by the server or it is a lie, and a lie in a tool schema does not
fail loudly — the model complies with the advertisement, the server ignores the
part it never implemented, and the call either fails blaming a field the model
did not get wrong, or silently does something else.

Three sub-rules follow, and they are the invariants this work is measured against.

### 1. A refusal must be actionable

Already stated, verbatim, by the in-tree module that owns model-facing argument
decoding, `src-app/server/src/common/tool_args.rs`:

> Anything that cannot become the declared shape is an `ArgError` whose message
> names **what was received**, **what is expected**, and **a literal JSON example
> the model can copy**.

That module applies the rule to *shape* refusals. It is a property of every
model-facing refusal, not only shape ones. `BACKGROUND_TASK_REQUIRED`
(`"spec.task must be a non-empty string"`) meets none of the three, and worse,
names the wrong field.

### 2. A supplied argument is never silently replaced by a default

Also verbatim from `tool_args.rs`:

> A supplied-but-undecodable argument is **never** turned into the default — that
> would hide the mistake instead of correcting it.

`spawn_background` reads `kind` from the top level only. A `kind` supplied inside
`spec` — which the schema invites, since `spec` is an open object whose visible
properties are the per-kind fields — is dropped and replaced by
`unwrap_or("subagent")`. When the rest of the spec is a sandbox spec that
produces a *wrong* error; when the spec also happens to carry `task` it produces
**no error at all** and runs the wrong job kind. Silent-wrong-thing is the worse
half of this defect and the reason the rule is absolute.

### 3. An advertised enum is enforced before the value is used

New here, and the one statement not already written down somewhere in the tree:

> A value the tool schema advertises as an `enum` is validated against that enum
> by the server **before** it is used to construct any URL, filesystem path, or
> process argument.

`flavor` is advertised as `"enum": ["minimal", "full"]` in **two** tool schemas
and validated in neither. The value flows into
`format!("ziee-sandbox-rootfs-{arch}-{flavor}.{ext}")` and becomes a live GitHub
Releases request. This is the §2 (outbound HTTP) shape of the same failure: an
unvalidated model-supplied string reaching a network call.

## Scope boundary

The **admin** rootfs-install path (`version_handlers.rs`) validates `flavor` as a
safe token rather than against `KNOWN_FLAVORS`, deliberately: an operator must be
able to install a flavor published after this binary was built. That escape hatch
is correct and stays. The enum promise is made only in the two *model-facing*
tool schemas, so that is exactly where it is kept — at the entry point, not in
`install_version`, which serves both callers.

## Non-goals

- Re-designing the `spawn_background` tool surface. `kind`/`spec` stays.
- Changing what a correctly-formed call does. Every fix here is a refusal path
  or a resolution that was previously a silent drop.

## References

- `src-app/server/src/common/tool_args.rs` — module docs (the existing contract
  for model-supplied object/array arguments, and its conformance battery).
- `agent-kit/docs/CODING_GUIDELINES.md` §2 (outbound HTTP & SSRF), §10 (API /
  type contract), §11 (built-in MCP server checklist).
