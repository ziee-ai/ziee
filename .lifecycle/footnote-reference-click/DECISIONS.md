# DECISIONS — every input resolved up front

### DEC-1: Fix downstream (component overrides) or upstream (override Streamdown's clobberPrefix / sanitize schema)?
**Resolution:** Downstream — make the `a`/`li`/`h2` overrides prefix-count-agnostic. Do not override Streamdown's rehype/sanitize config.
**Basis:** codebase — `rehype-sanitize` is the XSS boundary; the project intentionally keeps Streamdown's default plugin surface (see the `img`/no-katex NOTES in useStreamdownComponents.tsx). Overriding clobberPrefix on one side leaves the href/id asymmetry unfixed (href is not clobbered), so a downstream, prefix-agnostic scope is both safer and actually correct.

### DEC-2: How to tolerate the prefix variance robustly?
**Resolution:** Match the footnote suffix with `^(?:user-content-)*(fn|fnref)-(.+)$` (and `^(?:user-content-)*footnote-label$`), rebuilding the scoped id as `${contentId}-${kind}-${suffix}`. Handles 0/1/2+ prefixes.
**Basis:** codebase — mirrors the existing intent of the inline logic (strip the `user-content-` prefix, re-scope to contentId) but decoupled from the exact prefix count Streamdown emits, per the task's "don't hardcode a brittle prefix" directive.

### DEC-3: Where do the pure helpers live and how are they tested?
**Resolution:** New `src-app/ui/src/modules/chat/core/utils/footnoteScope.ts` + colocated `footnoteScope.test.ts` (node:test).
**Basis:** convention — mirrors `components/common/imageSrcPolicy.ts` + `imageSrcPolicy.test.ts`, the exact "extract pure logic for unit tests" pattern already cited in useStreamdownComponents.tsx.

### DEC-4: Does `scopeHref` keep the heading-hash retargeting and external-link behavior?
**Resolution:** Yes. `scopeHref` returns: footnote hash → `#${contentId}-<kind>-<suffix>`; other `#…` → `#${contentId}-h-${slugifyHeading(safeDecode(rest))}`; anything else (incl. undefined) → unchanged. The component keeps deciding external-vs-hash by testing the returned value with `startsWith('#')`, exactly as today.
**Basis:** codebase — preserves the current a-override branch structure so no non-footnote link behavior changes.

### DEC-5: Which footnote "kinds" must scope? Does the ref's own `id` (fnref) need scoping?
**Resolution:** Both `fn-` (definition + href target) and `fnref-` (the ref anchor id). Scope both for internal consistency even though back-reference (↩) links are hidden by the override.
**Basis:** codebase — the existing code already scoped both `user-content-fn-` and `user-content-fnref-`; keeping parity avoids a half-scoped DOM and keeps the fnref anchor unique per message.

### DEC-6: How does the e2e seed a two-message conversation for per-message scoping (TEST-6)?
**Resolution:** Add a local `seedTwoAssistantMessages` variant in the spec that passes two `assistantTextMessage(...)` entries to `mockGetMessages` (which already accepts an array), reusing the existing mock helpers; no new fixture infrastructure.
**Basis:** codebase — `mockGetMessages` in `sse-mock-helpers` already takes a message array; the existing single-message `seedAssistantWithText` is the template.

### DEC-7: Citations-system (`/settings/citations`) integration?
**Resolution:** Out of scope for this bug. Note as a possible follow-up in the PR body only.
**Basis:** user — the task says call it out but do not silently expand scope; these are inline GFM markdown footnotes, not bibliography entries.

### DEC-8: PR base branch and author identity.
**Resolution:** PR targets `khoi` (integration branch). Commits authored `khoi <khoi@tinnguyen-lab.com>`, no Claude/AI attribution. Strip `.lifecycle/` before the final commit.
**Basis:** user — explicit mid-session instruction + [[no-claude-attribution]] / [[git-commit-identity]] memory notes.
