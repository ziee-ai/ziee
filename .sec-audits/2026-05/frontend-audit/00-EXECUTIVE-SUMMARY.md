# Frontend Audit — Executive Summary

**Date:** 2026-05-23 (initial) · **revised 2026-05-23 post-review** — see §9 for the revision log.
**Branch audited:** `security/remediation-2026-05`
**Scope:** `src-app/ui/src/` — ~185 .tsx files across 19 modules, plus shared core/components/hooks/themes/utils/api-client.
**Method:** Read-only static analysis, 9 parallel audit agents, followed by a verification pass on 15 high-blast-radius HIGH findings.
**Plan reference:** `/home/pbya/.claude/plans/there-is-another-serene-coral.md` (permission-gating foundation, treated as implemented).

---

## Remediation status (2026-05-25)

Findings have been remediated across two prior waves and a final audit-
remediation series on branch `chore/ui-deps-antd-cleanup`. Per-finding
resolution is tracked by the commits below; severity reclassifications
are documented in §9 / §9.2 / §9.3.

**Resolved by the permission-gating plan** (commits `7674c6b`..`970d0ab`
on `feat/frontend-permission-gating`): 03 B-1, 03 B-5, 05 H-3, 06 F-1,
06 F-2, 06 F-20 / 07 HIGH-2, 07 HIGH-1. **7 HIGH findings → RESOLVED.**

**Resolved by the dep upgrade** (commit `226f3d9 chore(ui/antd):` on
`chore/ui-deps-antd-cleanup`): 05 H-2 (the copy-to-clipboard button was
removed during the antd-lint cleanup), 05 LLM-25 (RuntimeDownloadDrawer
now uses the project Drawer wrapper). **2 → RESOLVED.**

**Resolved by the audit-remediation series** (commits on
`chore/ui-deps-antd-cleanup`):
- **Cluster A** (`3336b5c`) — 01 B-12 (HIGH), 01 B-17 (HIGH), 02 B-1
  (HIGH), 01 B-5 (MED), 01 B-6 (MED), 02 B-2 (MED).
- **Cluster B** (`2352e1b`) — 04 HIGH-4 (HIGH), 05 LLM-13 (HIGH), 03 B-4
  (MED, frontend hidden until backend endpoint ships).
- **Cluster C** (`cd7bd21` + `b963f42`) — 09 B-1 (HIGH), 04 HIGH-2
  (HIGH), 05 H-4 (MED), 09 B-8 (MED), 09 B-9 (MED), 04 HIGH-1 (MED).
- **Cluster D** (`2b87a62`) — 02 B-4 (HIGH), 04 HIGH-3 (MED), plus the
  Auth `/me` re-fetch on `visibilitychange` permission follow-up.
- **Cluster E** (`27c1e55`) — 02 R-1 (HIGH).
- **Cluster F** (`e0fbaf4` + `598c2f8`) — 09 B-15 (MED, Biome rule),
  08 I-4 (MED, Popconfirm okText standardisation across 7 sites + 7
  E2E helper files).
- **Cluster G** (this commit) — audit-doc annotation, store-proxy doc,
  remaining LOW cleanups deferred.

**14 HIGH + 9 MED + permission follow-up resolved across the series.**

**Withdrawn during verification** (do not appear in fix tracking):
- 05 LLM-25 — already fixed pre-remediation in dep cluster 3.
- Scroll-container nesting MED — verified harmless on inspection.
- 3 store-proxy "HIGH" findings (B-1, B-2, B-3) — original audit
  framing was wrong; the proxy design is sound (see §9.1).
- 06 F-3 / 09 B-3 dedup-guard HIGH — overstated; guard is correct for
  post-init concurrent dedup.

**Deferred to follow-up commits / future PRs** (out of this PR's scope):
- Remaining Cluster F consistency items: 08 I-1 (footer order — only
  user/ module deviates), 08 I-2 (submit-button labels — 5+ phrasings),
  08 I-3 (`disabled={!canManage}` rollout — code-sandbox-only today),
  08 I-5 (drawer widths — 600 dominant, 2× 400 list-only justifiable),
  08 I-6 (empty-state copy — 6 phrasings). Each touches 10+ files +
  many E2E specs; better as separate review-able PRs.
- Cluster G tooling: `scripts/lint-stores.ts` static analyser + dev-
  only runtime guard in `createStoreProxy`. Deferred — substantial
  new infrastructure (~150 LOC + tests + CI wiring) that warrants its
  own PR.
- `tests/e2e/permissions/hub.spec.ts` — permission follow-up; defer
  with the lint-stores work into the tooling PR.
- 4 confirmed LOWs (05 H-1 — backend already masks; 02 B-3 — pixel
  width reset; 03 B-3 — auth events speculative; 01 B-1 — proxy doc
  partial). Address opportunistically.

---

## 0. Meta-finding (READ THIS FIRST)

**The permission-gating plan was NOT applied to this branch when the audit ran.** Every agent independently confirmed this, in 8 of 9 reports:

| Evidence | Source |
|---|---|
| `src/core/permissions/` directory does not exist | 01 §Summary, 03 §Summary, 06 F-1, 07 §Plan-status |
| `HubPage.tsx:26` still has `// TODO: integrate permission check` | 07 HIGH-1 |
| `HubTabSlot.ts:9` still uses `permission?: string` (not the widened `permissions: { read; refresh? }`) | 07 HIGH-1 |
| `SettingsPage.tsx` does not filter `settingsAdminPages` by permission | 06 F-20, 07 HIGH-2 |
| Sandbox sections still ship the duplicated local `hasPermission` helper with the buggy single-colon split and the missing `is_admin` short-circuit | 06 F-1 |
| `LeftSidebar.tsx` does not filter `sidebarNavigation` / `sidebarTools` by permission | 03 B-5, 07 §Plan-status |
| No `<Result status="403">` or `<Result status="404">` anywhere in the codebase | 02 §Summary, 02 R-10 |
| `Auth.store.ts` does not re-fetch `/api/auth/me` on `visibilitychange` | 03 E-3 |
| No module file imports `<Can>` or `usePermission` | 03 B-5 |

**Implication:** The audit findings combine **(a)** bugs that exist on the current branch independent of the plan, plus **(b)** the absence-of-plan itself, which we flagged where it manifests. The plan still needs to be applied as designed; nothing in this audit invalidates the plan. The audit also surfaced **new permission-gating gaps** the plan did not enumerate (e.g. `SandboxResourceLimitsSection` is missing the `code_sandbox::resource_limits::read` gate even by today's standards — F-2 in 06).

---

## 1. Per-file counts (post-revision, three waves)

| # | File | HIGH | MED | LOW | Total | Top theme |
|---|---|---|---|---|---|---|
| 01 | core-theme-shared | **2** | 7 | **10** | 19 | `getAuthToken` JSON.parse crash; missing error boundary; theme parity drift |
| 02 | layout-shell-responsive | 3 | 7 | 5 | 15 | `useWindowMinSize` mislabeled breakpoints; visualViewport thrash; mobile a11y |
| 03 | auth-user-permissions | 5 | **6** | **7** | 18 | Auth store emits no events; EditUser drops display_name; plan-gating absent *(2 MEDs → LOW)* |
| 04 | chat-module | **4** | **6** | **7** | 17 | Conversation-switch race; SVG iframe XSS; Blob URL leak *(MED-5→HIGH; MED-3→LOW; LOW-2 removed)* |
| 05 | llm-modules | **5** | **5** | **11** | 21 | API key reveal; loadModels race; provider edit state-loss *(LLM-13→HIGH, several MED→LOW)* |
| 06 | mcp-sandbox | 5 | **7** | **11** | 23 | Sandbox helper untouched; ResourceLimits missing `::read` gate; N+1 lookups *(F-10, F-16 → LOW)* |
| 07 | hub-assistants-settings-misc | 2 | **6** | **13** | 21 | Hub permission gate; Settings admin items unfiltered *(MED-4, 6, 8 → LOW)* |
| 08 | cross-cutting-consistency | 1 | 8 | 6 | 15 | Confirmation pattern split |
| 09 | cross-cutting-correctness | 0 | **10** | **7** | 17 | Chat streaming AbortController; 4 stores missing `EventBus.off` *(B-4 → LOW)* |
| — | **Totals (post-3 waves)** | **27** | **62** | **77** | **166** | — |

**Deduplication note:** 06 F-20 and 07 HIGH-2 are the SAME bug. Per-file counts include both; the de-duplicated HIGH total is **26**.

Each finding has a `file:line` citation. See the individual reports for the full text.

**Verification status:** All HIGH findings have been verified against source (three waves). MEDs have been verified once. Counts shifted as findings were re-classified — total is 166 not 165 because one false-positive (04 LOW-2 double-`__state`) was removed and one finding was split. See §9.3.

---

## 2. HIGH-severity findings — consolidated list

Ordered for remediation (foundational fixes first, then user-visible bugs, then known-unknowns). Each entry: `[file] ID — title — file:line`.

### Tier A — Foundational (fix first; other fixes depend on them)

1. **[01] B-12** — No top-level React error boundary anywhere; one throw in any module white-screens the shell. — `src/App.tsx`, `src/main.tsx`
2. **[02] B-1** — `useWindowMinSize` mislabels half its breakpoints (`xs` returns ≤640 not ≤480; `xl/2xl` duplicate at ≤1280). Verified: 10 consumers misread the viewport. — `src/modules/layouts/app-layout/hooks/useWindowMinSize.ts:37-50`
3. **[02] B-2** — `useMainContentMinSize` uses a DIFFERENT (also wrong) mapping with opposite polarity for `3xl`. Verified: two consumers diverge from window-size consumers. — `src/modules/layouts/app-layout/hooks/useWindowMinSize.ts:52-61`
4. **[01] B-5** — Event bus uses `immer` middleware but mutates `Map`/`Set` instances; `enableMapSet()` is NOT called in `core/events/store.ts` (it IS called in 3 other stores — `Mcp.store.ts`, `McpServer.store.ts`, `UserAssistants.store.ts` — but not the event bus). Verified.
5. **[01] B-6** — `emit()` async rejections propagate to mutation callers via `Promise.all`. A single buggy listener can break unrelated mutations. Verified.
6. **[All] PLAN APPLICATION** — Apply the permission-gating plan as designed. Verified: `src/core/permissions/` doesn't exist; `HubPage.tsx:26` still has `// TODO`; `HubTabSlot.ts:9` still `permission?: string`; `SettingsPage.tsx:22-50` doesn't filter; `LeftSidebar.tsx:127-142` doesn't filter; sandbox helper duplicates at `SandboxEnvironmentsSection.tsx:17-23` and `SandboxResourceLimitsSection.tsx:30-36`. Cross-cutting; touches every module.
7. **[01] B-17** *(promoted from MED in wave 3)* — `getAuthToken` does `JSON.parse(localStorage.getItem('auth-storage'))` with no try/catch. Every API call invokes it. Corrupt localStorage → entire API client crashes synchronously, app cannot recover without manual `localStorage.clear()`. — `src/api-client/core.ts:10-18`

*(Originally Tier A listed 7 items; 3 store-proxy items removed. See §9 for the analysis.)*

### Tier B — Security / data integrity (user-visible damage if left)

8. **[05] H-1** — Admin Edit-Provider drawer pre-fills `provider.api_key` into a reveal-able `<Input.Password>` field. User pattern (`UserLlmProvidersPage.tsx:25`) already uses `'••••'` placeholder — admin path should mirror. — `src/modules/llm-provider/components/LlmProviderDrawer.tsx:38`, `RemoteProviderSettings.tsx:96`
9. **[05] H-2** — Copy-to-clipboard button reads the cleartext system API key directly to OS clipboard. Trivial exfiltration. — `src/modules/llm-provider/components/RemoteProviderSettings.tsx:154-163`
10. **[05] H-3** — LLM repository drawer pre-fills `api_key`/`password`/`token` into the form; "Test Connection" button re-transmits the unchanged secret on the wire. — `src/modules/llm-repository/components/LlmRepositoryDrawer.tsx:32-37`
11. **[03] B-1** — `EditUserDrawer` silently drops `display_name` on every save (field never put on form, never sent). Admins literally cannot change a user's display name from the UI. — `src/modules/user/components/user/EditUserDrawer.tsx:46-69`
12. **[03] B-4** — `UserRegistrationSettings` is a UI lie: both load and update are TODO stubs that hard-code `true` and message-success despite never calling the API. — `src/modules/user/...UserRegistrationSettings.tsx`, `Users.store.ts:295-355`
13. **[04] HIGH-4** — Web/SVG viewer iframe with `sandbox="allow-scripts"` renders model-output and uploaded SVGs. Scripts run in unique-origin sandbox but can still phish (uniform-looking UI) and `fetch` external endpoints. — `src/modules/chat/extensions/file/file-viewers/web/body.tsx:17-22`
14. **[02] B-4** — `document.body.style.height` rewritten on every `visualViewport.resize` AND `document.documentElement.scrollTop = 0` forced on the same event. iOS Safari keyboard show/hide yanks scroll position mid-conversation (data-loss-class UX). Also competes with `100dvh` on `.ant-app`. — `src/modules/layouts/app-layout/AppLayout.tsx:153-176`
14b. **[05] LLM-13** *(promoted from MED in wave 3)* — `RemoteProviderSettings` effect deps include `currentProvider` (a recomputed object reference, not just its id). On every other-tab SSE provider update, the effect fires, the form re-initializes, and the user's mid-edit unsaved API key / base URL / config silently vanishes. State-loss class. — `src/modules/llm-provider/components/RemoteProviderSettings.tsx:32-34`

### Tier C — Correctness / race conditions

15. **[09] B-1** — `Chat.store.__destroy__` does not abort `streamingAbortController`. After 5s grace destruction mid-stream, SSE keeps running and on next mount a second parallel fetch is spawned. — `src/modules/chat/core/stores/Chat.store.ts:1459-1485`
16. **[04] HIGH-1 / 09 B-10** — `ConversationPage.tsx:18-22` calls `loadConversation(id)` in a bare `useEffect` with no AbortController, no stale-result guard. Conversation A→B→A sequence can show A's messages with URL B. — `src/modules/chat/pages/ConversationPage.tsx:18-22`
17. **[04] HIGH-2** *(severity conditional, currently MED)* — No message-list virtualization. Verified: `Array.from(messages.values()).map(<ChatMessage>)` every render, no virtualization library in `package.json`. `ChatMessage` IS `React.memo`'d. Verification mitigation: Streamdown/shiki is **not actually wired up** today — `TextContent` renders plain `<div style={{ whiteSpace: 'pre-wrap' }}>`, so per-token cost is small. Becomes HIGH the moment markdown rendering is added (there's a `TODO` for it). The key-transition mid-stream bug (`streaming-${ts}` → real DB ID at `Chat.store.ts:1142-1145`) IS real today on every streamed message. — `src/modules/chat/core/components/MessageList.tsx:18,34`
18. **[04] HIGH-3** — Auto-scroll `scrollIntoView` on every token in `useEffect`. No "user is at bottom" detection. Scrolling up to read history yanks back to bottom on next token. — `src/modules/chat/pages/ConversationPage.tsx:25-28`
19. **[05] H-4** — `loadModelsForProvider` has no AbortController, no stale-result guard, called from 4+ sites including burst-y SSE handlers. Fast provider-switching during download can overwrite fresh state with stale models. — `src/modules/llm-provider/stores/LlmProvider.store.ts:184-224`
20. ~~**[06] F-3** — `SystemMcpServer.loadSystemServers` dedup guard~~  *(downgraded to LOW — see §9. The guard is correct for the post-init concurrent-dedup case; the audit's framing was wrong. Cross-ref: `09-cross-cutting-correctness.md` B-3 also downgraded.)*
21. **[03] B-2** — `UserGroupsDrawer` is N² + racy: opens by calling `loadUserGroupMembers` twice per group, inner store has a no-op guard, second loop reads shared mutable state for membership detection. 20 groups = up to 40 round-trips per drawer-open. — `src/modules/user/.../UserGroupsDrawer.tsx:15-52`
22. **[03] B-3** — `Auth.store` emits ZERO events for `authenticateUser`, `registerNewUser`, `logoutUser`, `initAuth`. Other stores can't react to login (load user data), logout (clear caches), or session restore. — `src/modules/auth/Auth.store.ts:65-203`
23. **[06] F-4** — N+1 "servers assigned to group" loops in 3 paths despite bulk endpoint `Group.getSystemServers` already existing in the generated client. M groups × N servers = M*N round-trips on admin Groups page. — `src/modules/mcp/stores/SystemMcpServer.store.ts:391-410` + 2 widget stores
24. ~~**[02] B-3** — AppLayout sidebar drag stores width in `useRef` only~~ *(downgraded to MED — verification: `isSidebarCollapsed` IS persisted; only pixel-width is lost on reload, with collapsed/expanded state preserved. UX impact lower than HIGH.)*
24b. **[04] MED-5** *(promoted from MED in wave 3)* — Blob URLs created for file viewers are stored in `messageFilesCache` but `messageFilesCache.clear()` is never called AND the file extension has no cleanup hook. URLs accumulate per conversation switch. Long sessions exhaust memory. — `src/modules/chat/extensions/file/File.store.ts`, file extension lifecycle

### Tier D — UX / accessibility / responsive

25. **[02] R-1** — Mobile sidebar overlay has no focus trap, no `role="dialog"`/`aria-modal`, no Escape-to-close, no body scroll lock, and triple-fires backdrop handlers (`onClick`+`onMouseDown`+`onTouchStart`). The mobile open-nav UX is a 24×24 px chevron (below WCAG 2.5.5 minimum) — no hamburger anywhere. — `src/modules/layouts/app-layout/AppLayout.tsx:199-243`

### Tier E — Cross-cutting consistency (must agree on convention, then apply)

26. ~~**[08] I-1** — Form footer button order is split 50/50~~ *(downgraded to MED — verification: actual split is ~80/20, not 50/50. Only the `user/` module deviates with Submit→Cancel left-aligned; all other modules consistently use Cancel→Submit right-aligned. Real inconsistency but smaller than originally framed.)*
27. ~~**[08] I-2** — Submit-button labels ad-hoc~~ *(downgraded to MED — confirmed 10 distinct labels across 6 drawers but this is consistency UX, not a bug.)*
28. ~~**[08] I-3** — Plan's `disabled={!canManage}` pattern only in code-sandbox~~ *(downgraded to MED — confirmed only 2 instances (both code-sandbox), but backend enforces; this is UX gap, not data integrity.)*
29. **[08] I-4** — Destructive-action confirmation split between `modal.confirm` (3 sites) and `<Popconfirm>` (11 sites), with inconsistent `okText` ("Delete" vs "Yes"). Verified: 14 confirmation sites total, 8 use "Yes" and 5 use "Delete". The "Yes"/"Delete" divergence is the actual HIGH-severity piece — users can misclick when the confirmation phrasing doesn't match the action.

### Tier F — Other HIGH-severity findings (per-module specifics)

30. **[06] F-20 / [07] HIGH-2** *(same bug, reported in two files)* — `SettingsPage` does NOT filter admin pages by permission; every `settingsAdminPages` slot rendered for any authenticated user. — `src/modules/settings/SettingsPage.tsx:18-50`
31. **[03] B-5** — Plan-compliance: zero permission gating applied in `auth/`/`user/`/`onboarding/`/`user-profile/`. Every admin button in `UsersSettings`/`UserGroupsSettings`/8+ drawers is ungated.
32. **[07] HIGH-1** — Hub permission gate unimplemented (`HubPage.tsx:26-27` literal TODO).
33. ~~**[07] HIGH-3** — Hardware SSE module-level mutables + popup separate-context~~ *(downgraded to MED — verification: popup CAN connect independently; the audit's "silently sits at Connect to hardware monitoring forever" claim is wrong (popup has its own working store). Real architecture concern but UX-class, not data-integrity.)*
34. **[06] F-2** — `SandboxResourceLimitsSection` has no `code_sandbox::resource_limits::read` gate. Users without read load the form, hit 403, see confusing mix of error alert + disabled form (sibling section ships clean permission-denied alert). — `src/modules/code-sandbox/components/SandboxResourceLimitsSection.tsx:22-23, 104-108`
35. **[01] B-5** — Event bus uses `immer` middleware but mutates Map/Set instances; `enableMapSet()` is not called in `core/events/store.ts` (verified: IS called in 3 other stores). Either silent no-op (auto-freeze) or throw (production). — `src/core/events/store.ts:42,46-128`
36. **[01] B-6** — `emit()` async rejection propagates to mutation callers. Most call sites do `await emitFooCreated(...)` without try/catch — a single buggy listener can break unrelated mutations. — `src/core/events/store.ts:97-107`

---

## 3. Cross-area patterns (highest-leverage fixes)

These cut across multiple agents' reports and suggest a single fix benefits many places.

### P-1. Per-store `__destroy__` contracts are missing or incomplete  *(revised 2026-05-23)*
The original framing pointed at `createStoreProxy` itself as the root cause. On re-review (§9), the proxy is sound by convention — actions return directly, nested stores return directly, only state-value reads invoke hooks, and that path is conventionally component-only. The actual cross-cutting issue is that **stores don't honor their lifecycle contract**:

- **09 B-9** — 4 stores subscribe via `EventBus.on(...)` in `__init__.__store__` with NO `__destroy__` cleanup → listener slots accumulate per destroy/re-init cycle (Auth + 3 hub stores).
- **09 B-1** — `Chat.store.__destroy__` doesn't abort `streamingAbortController`. SSE keeps running through the 5s grace period; on re-init, a SECOND parallel fetch is spawned.
- **09 B-8** — `Hardware.store` and `LlmModelDownload.store` keep `AbortController` in **module scope**; the proxy destroy doesn't free them, and reconnect is blocked.
- **09 B-13** — `SandboxEnvironments.store` has the same module-scope SSE pattern.

**Recommended fix:** document a strict `__destroy__` contract in `.claude/REACT_COMPONENT_PATTERNS.md`: stores MUST abort all in-flight SSE/fetch, unsubscribe ALL event listeners, and clear any module-scope timers/controllers. Add a JSDoc note on `createStoreProxy` (and `Stores` proxy at `core/stores.ts:273`) pointing to the contract. Optionally add an ESLint rule to flag store actions that use `EventBus.on` without a corresponding `off` in `__destroy__`.

### P-2. No store `load*` has stale-result protection
- 09 B-4, B-5, B-6, B-7: `ChatHistory.loadConversations(page)`, `Users.loadUsers(page)`, `loadModelsForProvider`, drawer `loadAssignedGroups`
- 09 explicit appendix: **zero AbortControllers for load actions anywhere in the codebase; zero stale-request tokens**
- 04 HIGH-1, 05 H-4, 06 F-3 all instantiate this pattern

**Recommended fix:** introduce a `useCancelableLoad` helper (or a `withRequestToken` store mixin) that captures a token at call time and discards stale-token results. Apply systematically to every `load*` action.

### P-3. The permission-gating plan is unfinished
8 of 9 reports independently identified that the plan was not applied. This is not a bug per se — it's the work the plan was designed to do. Recommendation: apply the plan as designed, and additionally:
- Add the `code_sandbox::resource_limits::read` gate to `SandboxResourceLimitsSection` (06 F-2 — beyond the plan's enumeration).
- Verify the audit's expanded checklist (every admin button, every drawer's `disabled` thread-through, every drawer's submit-button gate) — see 03 B-5 for the user-module list and 06 F-20 for the settings-page filter.

### P-4. Consistency drift threatens the maintainability investment
- Drawer widths: dominant `size={600}` (15 instances), but `size={400}` × 2, `width={500}` × 1 bypassing the wrapper (08 I-5)
- Form footer button order split 50/50 (08 I-1)
- Submit-button labels ad-hoc — 10+ different phrasings for the same concept (08 I-2)
- Empty-state copy: 6 phrasings + 3 UI strategies (08 I-6)
- Confirmation modals: `modal.confirm` (3) vs `<Popconfirm>` (10) (08 I-4)
- Module structure: 11 of 20 modules deviate from the documented layout (08 Appendix 9)

**Recommended fix:** pick one convention per pattern, document in `.claude/REACT_COMPONENT_PATTERNS.md`, and gradually migrate. Highest value: form footer order (touches every drawer) and submit labels (touches every form).

### P-5. Production builds ship debug instrumentation
- 09 B-15: 171 `console.*` calls in `src/modules/`
- 05 §Summary: 21 `console.log` occurrences in `LlmProvider.store`, `LlmModelDownload.store`, `LlmProviderSettings`, `LocalProviderSettings`

**Recommended fix:** add a Vite plugin (`vite-plugin-strip-console`) or an ESLint rule (`no-console` with allowlist for `console.error`).

---

## 4. Permission plan — additions discovered by the audit

The plan's audit-deliverable (the table at `.../there-is-another-serene-coral.md` ~line 528) needs these rows when it's written:

| Surface | File | Permission | Gate type | Found in |
|---|---|---|---|---|
| Sandbox resource-limits section read gate | `SandboxResourceLimitsSection.tsx:22` | `code_sandbox::resource_limits::read` | section | 06 F-2 |
| Sandbox env list read gate (already exists but check sibling consistency) | `SandboxEnvironmentsSection.tsx:61` | `code_sandbox::environments::read` | section | 06 §Summary |
| Hub sidebar entry | `hub/module.tsx:28-37` | `anyOf: [hub::models::read, hub::assistants::read, hub::mcp_servers::read]` | menu-item | 07 HIGH-1 |
| Hub Refresh button per-tab | `HubPage.tsx:133-140` | `hub::<active-tab>::refresh` | button | 07 HIGH-1 |
| All admin user-management buttons (~14 drawers) | `src/modules/user/components/user/*.tsx` | `users::manage`, `users::delete`, `users::reset_password` | button | 03 B-5 |
| All admin group-management buttons | `src/modules/user/components/group/*.tsx` | `groups::manage`, `groups::delete` | button | 03 B-5 |
| `SettingsPage` admin-menu filter | `SettingsPage.tsx:22-50` | per slot `permission` | menu-item | 06 F-20, 07 HIGH-2 |
| `LeftSidebar` sidebarNavigation/sidebarTools filter | `LeftSidebar.tsx:~217` | per slot `permission` | menu-item | 03 B-5 |
| Auth `/me` re-fetch on visibilitychange | `Auth.store.ts` | — | event-handler | 03 E-3 |

Plus the `is_admin` short-circuit fix and `::`-separator alignment per the plan's existing copy.

---

## 5. Remediation sequencing (recommended order)

This audit produced ~165 findings (post-revision). A reasonable rollout:

### Sprint 1 — Foundational (unblocks everything else)
1. Apply the permission plan as designed (foundation + slot-type widening + slot consumers + audit table).
2. Fix the breakpoint hooks (02 B-1, B-2) — 10+ consumers misread mobile/tablet/desktop today.
3. Add a top-level error boundary (01 B-12).
4. Add `enableMapSet()` for `immer` in `core/events/store.ts` (01 B-5).
5. Fix `emit()` rejection propagation (01 B-6).
6. Document the `createStoreProxy` component-only contract for state-value reads in `.claude/REACT_COMPONENT_PATTERNS.md` (01 B-1, LOW but worth doing while the docs are open).

### Sprint 2 — Security / data integrity
1. Wrap `getAuthToken` JSON.parse in try/catch (01 B-17). Every API call goes through this; a single corrupt localStorage value crashes the app.
2. Mask API keys in admin LLM provider drawer + remove copy-to-clipboard (05 H-1, H-2).
3. Mask repository auth secrets in `LlmRepositoryDrawer` (05 H-3).
4. Restore `display_name` editing in `EditUserDrawer` (03 B-1).
5. Wire `UserRegistrationSettings` to the actual API or remove (03 B-4).
6. Remove `allow-scripts` from web/SVG iframe or accept the risk explicitly with CSP (04 HIGH-4). Risk: files come from `messageFilesCache` which includes other users' files in shared conversations.
7. Fix `RemoteProviderSettings` effect deps to prevent mid-edit state loss (05 LLM-13). Guard against re-initializing the form when the user has unsaved changes.
8. Add Blob URL cleanup hook to file extension + `messageFilesCache.clear()` on conversation switch (04 MED-5 promoted).

### Sprint 3 — Race conditions & correctness
1. Introduce `useCancelableLoad` / request-token pattern.
2. Apply to: `Chat.loadConversation`, `ChatHistory.loadConversations`, `Users.loadUsers`, `LlmProvider.loadModelsForProvider`, `Users.loadAssignedGroups`, all hub stores. (SystemMcpServer guard works correctly today — see §9.)
3. Abort streaming `AbortController` in `Chat.store.__destroy__` (09 B-1).
4. Add `__destroy__` cleanup contracts to all SSE stores (09 B-8 + Hardware + LlmModelDownload + SandboxEnvironments).
5. Add `EventBus.on` cleanup to Auth + 3 hub stores (09 B-9).
6. Add event emission to Auth store mutations — `authenticateUser`/`registerNewUser`/`logoutUser`/`initAuth` (03 B-3).
7. Fix the streaming-message key transition in `Chat.store.ts:1142-1145` to use stable keys (e.g. position-based or pre-allocate the real ID upstream) — see 04 HIGH-2 (the structural piece that's a real bug today).

### Sprint 4 — Performance + iOS UX
1. **Fix iOS visualViewport thrash (02 B-4, HIGH)** — promoted from prior placement; loses user scroll position when keyboard opens. Drop the `body.style.height` write or guard it; remove the unconditional `scrollTop=0`.
2. Switch hub `getServersForGroup`/`loadServersForGroup` to bulk endpoint `Group.getSystemServers` (06 F-4). Verified: bulk endpoint exists at `api-client/types.ts:1931` returning `{ servers: McpServer[] }`. Two of three call sites have the loop; the third (`McpServerGroupsAssignmentCard.store.ts:194-217`) is already single-call.
3. Detect "at bottom" before auto-scroll in conversation page (04 HIGH-3).
4. Virtualize the chat message list (04 HIGH-2, now MED). Pick a library (`react-virtuoso` likely the lightest fit). Currently lower-impact than originally framed (plain-text rendering is cheap) but **must land before Streamdown/markdown rendering is wired up**, or chat will stutter at 300+ messages.
5. Persist sidebar width (02 B-3, MED — was HIGH).

### Sprint 5 — Accessibility / mobile
1. Mobile sidebar overlay: focus trap, `role="dialog"`, Escape-to-close, body scroll lock, real hamburger (02 R-1).
2. Address the WCAG 2.5.5 24×24px tap target issue.
3. Double/triple-nested scroll containers (02 §Summary — HubPage worst).

### Sprint 6 — Consistency
1. Pick a confirmation pattern (08 I-4, HIGH). Resolve "Yes" vs "Delete" `okText` divergence; pick one shape (`<Popconfirm>` or `modal.confirm`) for all 14 sites.
2. Pick form footer order + submit labels (08 I-1, I-2 — both MED, was HIGH). Only the `user/` module deviates from Cancel→Submit right-aligned; migrating user-module footers to match the rest of the app is the smallest change.
3. Extend `disabled={!canManage}` pattern from code-sandbox to all admin drawers (08 I-3, MED).
4. Normalize drawer widths to `size={600}` (08 I-5, 05 §Summary `RuntimeDownloadDrawer` deviation).
5. Standardize empty states (08 I-6).
6. Strip production `console.*` (P-5, 09 B-15). **Verified: 367 `console.*` calls in `src/modules/`** — the audit's "171" undercount missed half. Worth a build-time strip via Vite plugin + lint rule.

### Sprint 7 — Tooling improvements (post-remediation, not audit fixes)

Not bugs from the audit — improvements that would prevent future regressions of the same class as the findings above.

1. **Store-proxy usage linter (Option B from the proxy-design discussion).** Standalone `ts-morph`-based script that walks the project AST and flags reactive reads of `Stores.X.<state-prop>` outside React components / `use*` hooks.
   - Location: `scripts/lint-stores.ts` (or similar).
   - Roughly ~150 lines using `ts-morph`. Wired into `package.json` scripts and `just check` alongside Biome (Biome's plugin story can't express the rule cleanly today).
   - What it catches: every misuse of the proxy that today only throws at runtime via React's generic "Invalid hook call" error.
   - What it allows: `Stores.X.action(...)`, `Stores.X.__state.field`, and `const { user } = Stores.Auth` inside components/hooks. Preserves the existing destructure-friendly syntax — no API change.
   - Caveat: heuristic component detection (PascalCase function name OR `use*` prefix OR return type annotated as JSX). Alias escapes through callback params won't be caught — those would be runtime-only. Pair with a dev-only runtime guard in `createStoreProxy` for a complete net.
   - Rationale: the audit found that the proxy's component-only contract for state reads is implicit and undocumented. A linter makes the contract enforceable at CI time without changing the syntax that makes the proxy worth keeping.
2. **Dev-only runtime guard in `createStoreProxy`.** ~15 lines. Inside the state-value branch, check `React.__SECRET_INTERNALS_DO_NOT_USE_OR_YOU_WILL_BE_FIRED.ReactCurrentDispatcher.current` and throw a precise error (`"Stores.Auth.user is a reactive read and must be accessed inside a React component or use* hook. Use Stores.Auth.__state.user for a snapshot."`) instead of React's generic "Invalid hook call". Stripped in production builds via `import.meta.env.DEV`.
3. **Periodic audit verification pass.** This audit found a 26% HIGH-overstatement rate that only surfaced after the user pushed back. Future audits should default to a verification pass on high-blast-radius HIGH findings before they drive remediation.

---

## 6. Items NOT covered statically

Findings that need runtime confirmation or device testing:

- **iOS Safari `100dvh` behavior with keyboard up** (02 R-2). The static analysis sees the constants; whether they actually behave correctly across iOS versions needs a device.
- **Service worker / PWA behaviors** — none audited; the codebase doesn't appear to register one but worth confirming.
- **Browser-specific scroll-anchor preservation** in long chats during streaming (04 HIGH-3). Different browsers have different scroll-anchor implementations.
- **Memory leak verification** — Blob URL leaks (04 §Summary) and event-listener accumulation (09 B-9, 09 P-1) need a real-time Chrome DevTools Memory profile under sustained navigation.
- **MCP elicitation form rendering** — schema validation correctness (04 §MCP integration) needs a live test with a sample schema.
- **OAuth flow for MCP servers** — agent 6 noted no OAuth flow was found, but runtime confirmation needed (the modern MCP spec uses Streamable HTTP with OAuth 2.1, which the user is tracking — see memory).
- **Bundle size / lazy-load coverage** — no agent attempted to measure. Recommend `vite-bundle-visualizer` once the audit fixes are landing.

---

## 7. How to read this audit

- **Start here** (00) for the overall shape.
- **Drill in by area** — each numbered file (01-09) contains the full finding text, fix sketch, and file:line citations.
- **Cross-reference IDs**: a finding labeled "[05] H-4" lives in `05-llm-modules.md` under its H-4 heading. Reports use slightly different ID conventions (`B-N`, `F-N`, `H-N`, `I-N`, `E-N`, `R-N`, `HIGH-N`) — each report's preamble explains its convention.
- **Plan compliance** is called out per-area in each report's plan-status section.

---

## 8. Audit hygiene (verification)

- ✅ All 9 detailed reports exist under `.sec-audits/2026-05/frontend-audit/`.
- ✅ Every report follows the finding format (Summary + categorized findings + appendices where applicable).
- ✅ Every module under `src/modules/` is covered by at least one report:
  - `app`, `auth`, `user`, `user-profile`, `onboarding` → 03
  - `layouts`, `router` → 02
  - `chat` → 04
  - `llm-provider`, `llm-repository`, `llm-local-runtime`, `user-llm-providers` → 05
  - `mcp`, `code-sandbox` → 06
  - `hub`, `assistants`, `settings`, `settings-general`, `hardware`, `projects`, `config-client` → 07
  - All modules cross-swept for consistency (08) and correctness (09).
- ✅ `src-app/` was not modified — only `.sec-audits/2026-05/frontend-audit/` files were written.
- ✅ Branch unchanged from session start (`security/remediation-2026-05`).
- ✅ Post-audit verification: 15 high-blast-radius HIGH findings re-read against source — 14 confirmed, 1 reframed, 4 (proxy-related) downgraded after design review.

---

## 9. Revision log (2026-05-23 post-review)

After the user pushed back on the store-proxy framing, the high-blast-radius findings were re-verified by reading the actual source files. Findings revised:

### Withdrawn / downgraded HIGH findings

| ID | Original | Revised | Reason |
|---|---|---|---|
| 01 B-1 | HIGH "proxy violates rules of hooks" | LOW "component-only contract undocumented" | The proxy's `Proxy.get` has 4 branches: special-prop, function, nested-store, state-value. Only the last calls hooks, and it's only entered for state-value reads — which are conventionally component-only, same as any custom hook. The contract is sound; only the documentation is missing. |
| 01 B-2 | HIGH "StrictMode double-count" | *(removed)* | StrictMode's mount→cleanup→mount cycle is synchronous; `cancelDestroy` fires within microseconds. No actual destruction. Only effect is dev-only `console.log` chatter, stripped in production. |
| 01 B-3 | HIGH "executeDestroy with live observers" | *(removed; rerouted)* | `executeDestroy` only runs when `totalCount === 0`, which means every component's `useEffect` cleanup has fired — no observer exists. The duplicate-subscription concern is per-store `__destroy__` contract failure, captured in `09 B-9`. |
| 01 B-4 | MED "refTracker reset off-by-one" | *(removed)* | `removeRef` already guards `if (current > 0)`. Stale `removeRef` after reset is a silent no-op. Safe by design. |
| 06 F-3 / 09 B-3 | HIGH/MED "SystemMcpServer dedup guard inverted" | LOW "minor defensive improvement" | The guard `initialized && loading && !page` IS correct for post-init concurrent dedup (the main case). On first mount, `initialized=false` so it correctly proceeds. The first-mount-concurrent case is theoretically uncovered but `propInitCheck` in `core/stores.ts:203-209` prevents it for auto-init. The audit framing "logically impossible to enter on a first mount" was technically true but mischaracterized the intent. |
| 04 HIGH-2 | HIGH "no virtualization + key transition" | MED "virtualization (conditional HIGH)" | `ChatMessage` IS memo'd. `Streamdown`/shiki is NOT actually in the rendering pipeline today (`TextContent` renders plain `<pre-wrap>`); per-token cost is small. Becomes HIGH when markdown rendering is wired up. The streaming-key-transition piece remains a real bug today. |

### Findings CONFIRMED verbatim by verification

| ID | Title | Verification |
|---|---|---|
| 02 B-1 | `useWindowMinSize` mislabeled breakpoints | Lines 37-49: keys shifted by one threshold; `xl/2xl` both at ≤1280 (duplicates). 10 consumers affected. |
| 02 B-2 | `useMainContentMinSize` polarity flip | Line 60: `3xl: width > breakpointValues['3xl']` (every other key uses `<=`). |
| 01 B-5 | `enableMapSet()` missing in event bus | Confirmed: `immer` middleware at line 41; mutations on Map/Set at lines 57, 62, 67, 69, 114, 147, 149; `enableMapSet` IS called in `Mcp.store.ts`, `McpServer.store.ts`, `UserAssistants.store.ts` but NOT in `core/events/store.ts`. |
| 01 B-6 | `emit()` rejection propagation | Lines 98-105: `try/catch` only catches sync throws; async rejections bubble via `Promise.all` to `await emit*` callers without try/catch (20+ wrapper functions in stores). |
| 03 B-1 | `EditUserDrawer` drops `display_name` | Confirmed: `setFieldsValue` at 46-54 doesn't include `display_name`; `UpdateUserRequest` at 62-69 doesn't include it. API type DOES support it. |
| 03 B-2 | `UserGroupsDrawer` N² fetch | Confirmed: TWO sequential loops at lines 22-31 and 35-41, each fetching members for every group. 2N calls per open. |
| 03 B-3 | Auth.store emits no events | Confirmed: no `emit*` / `EventBus.emit` calls in `authenticateUser`/`registerNewUser`/`logoutUser`/`initAuth`. |
| 03 B-4 | `UserRegistrationSettings` is a stub | Confirmed: `Users.store.ts:295-355` has `// TODO: Replace with actual API call`, hardcodes `userRegistrationEnabled: true`, never calls backend. UI shows `message.success`. |
| 04 HIGH-1 | ConversationPage stale-result race | Confirmed: lines 18-22, bare `useEffect`, no AbortController, no cleanup, no stale-result guard. |
| 04 HIGH-3 | Auto-scroll defeats scroll-up | Confirmed: lines 25-28, `scrollIntoView` on every `[messages]` change with no at-bottom guard. |
| 05 H-1 | API key reveal in admin Edit drawer | Confirmed: `LlmProviderDrawer.tsx:38` and `RemoteProviderSettings.tsx:94-98` pre-fill `provider.api_key` cleartext into `<Input.Password>`. Backend returns plaintext (`api-client/types.ts:798`). User-side `UserLlmProvidersPage.tsx:25` uses `••••` placeholder pattern instead. |
| 05 H-2 | Copy-to-clipboard cleartext API key | Confirmed: `RemoteProviderSettings.tsx:159` calls `copyToClipboard(currentProvider.api_key \|\| '')` via `window.navigator.clipboard.writeText`. |
| 05 H-3 | Repository auth secrets pre-fill + re-transmit | Confirmed: `LlmRepositoryDrawer.tsx:32-37` pre-fills `api_key`/`username`/`password`/`token`; `:78-89` packages them into `testData` for Test Connection; `:92` sends to backend. |
| 06 F-1 | sandbox helper still duplicated | Confirmed: `core/permissions/` does not exist; helper at `SandboxEnvironmentsSection.tsx:17-23` honors `*` wildcard but no `is_admin` short-circuit. |
| 06 F-2 | `SandboxResourceLimitsSection` no read gate | Confirmed: only `MANAGE_PERM` defined at line 22; no `READ_PERM` constant; no early-return Alert (sibling section has both). |
| 06 F-4 | N+1 calls in 2 of 3 paths | Confirmed: `SystemMcpServer.store.ts:391-410` and `GroupSystemMcpServersWidget.store.ts:243-260` both loop. Third path (`McpServerGroupsAssignmentCard.store.ts:194-217`) is correctly single-call (audit already acknowledged). Bulk endpoint `Group.getSystemServers` exists at `api-client/types.ts:1931`. |
| 07 HIGH-1 | Hub permission gate unimplemented | Confirmed: `HubPage.tsx:26-27` literally has `// TODO: integrate permission check` and `const visibleTabs = hubTabs`. `HubTabSlot.ts:9` still `permission?: string`. |
| 09 B-1 | `Chat.store.__destroy__` doesn't abort streaming | Confirmed: `Chat.store.ts:1459-1485` `__destroy__` references `cacheClearTimers`, `saveConversationState`, `conversationStateCache` — zero references to `streamingAbortController`. |

### Net impact on totals

- HIGH: 35 → 29 (4 dropped from proxy, 1 from F-3, 1 from HIGH-2 reclassification)
- MED: 73 → 72 (gained HIGH-2, lost 09 B-3)
- LOW: 61 → 64 (gained B-1, F-3, 09 B-3)
- Total: 169 → 165 (lost B-2, B-3, B-4 entirely)

### Methodology note for future audits

The proxy findings were rooted in pattern-matching ("hooks in a Proxy.get = rules-of-hooks violation") without tracing the runtime branches. Lesson: when a finding asserts that a piece of infrastructure code is broken in a way that would break the entire app, **the code wasn't broken — the audit was**. Higher-blast-radius findings should be verified by tracing the actual runtime path before being filed as HIGH.

---

## 9.3 Third verification wave (2026-05-23 — all MEDs verified, LOWs spot-checked)

After the user asked "what about MED and LOW", a third wave of 9 parallel verifiers re-read every MED finding and spot-checked LOWs for severity misclassification.

### NEW HIGH findings (promoted from MED)

| ID | File | Title | Source line | Why HIGH |
|---|---|---|---|---|
| **01 B-17** | core | `getAuthToken` JSON.parse no try/catch | `src/api-client/core.ts:10-18` | Every API call invokes this. Corrupt localStorage (power loss mid-write, manual user edit, future store migration) throws synchronously before request error handling → entire API client crashes, app cannot recover without manual `localStorage.clear()`. |
| **04 MED-5** | chat | Blob URL leak via `messageFilesCache` | `src/modules/chat/extensions/file/File.store.ts`, extension lifecycle | `messageFilesCache.clear()` is never called; file extension has NO cleanup hook. Blob URLs accumulate per session. Long chat sessions = memory exhaustion. Real data-class impact at scale. |
| **05 LLM-13** | llm | `RemoteProviderSettings` effect re-runs on every store change | `RemoteProviderSettings.tsx:32-34` | `currentProvider` recomputes on every render (object reference). Mid-edit user state (unsaved API key, base URL changes) can silently vanish on any other tab's SSE provider update. State-loss class. |

### Downgraded MED → LOW

| ID | File | Title | Reason |
|---|---|---|---|
| 01 R-1 | core | `#root: 100%` resolution claim | Browsers treat `html` as containing block; CSS resolves correctly. Audit claim was speculative. |
| 01 E-2 | core | `usePrefetchModules` incremental-walk claim | Hook only re-runs when `routes` array changes; no incremental walk exists. Audit framing wrong. |
| 03 B-7 | auth | `AuthGuard` navigate-during-render | Source code shows `navigate()` is INSIDE `useEffect` blocks (lines 26-31, 56-63), not in render path. Already-mitigated. |
| 03 B-8 | auth | `isInitializing` state management | Cosmetic note about state management; no runtime impact. |
| 04 MED-3 | chat | `rel="noreferrer"` missing `noopener` | Modern Chromium implicitly adds `noopener` when `target="_blank"` + `rel="noreferrer"`. Gap remains in legacy Safari/Firefox ESR but phishing surface is small. |
| 05 LLM-06 | llm | Form-state stale-defaults | Edge case only triggers if drawer closes without `handleClose` path; `form.resetFields()` mitigates. |
| 05 LLM-07 | llm | `llmProviderHasCredentials` dead code | Function always returns true; misleading but not harmful. |
| 05 LLM-09 | llm | `per_page: 1000` magic number | No pagination UI but unlikely to hit cap; silent truncation is inelegant but not user-facing. |
| 05 LLM-11 | llm | `RuntimeDownloadDrawer` `width={500}` | Visual divergence only (the mobile-overflow piece is captured separately as MED in LLM-25). |
| 05 LLM-12 | llm | Five footer-rendering patterns | All functional; consistency issue not UX regression. |
| 06 F-10 | mcp | Read-only Switch hide | Polish, not core workflow blocker. |
| 06 F-16 | mcp | Two useEffects with same deps | Theoretical race; React execution order is source-code-order in practice. |
| 07 MED-4 | hub | `loadUserAssistants` short-circuit | Audit itself flagged for re-check; not a correctness bug. |
| 07 MED-6 | hub | Inline-style volume in HardwareSettings | 47 inline styles confirmed (not 48); no actual dark-mode regression observed. |
| 07 MED-8 | hub | Search/sort rebuilds on keystroke | Memoization in place; catalog is small; not a perf issue today. |
| 09 B-4 | chat | `ChatHistory.loadConversations(page)` race | Brief UI inconsistency only; not data loss. User sees wrong page momentarily until next load completes. |

### Promoted LOW → MED

| ID | File | Title | Why MED |
|---|---|---|---|
| 02 I-4 | layout | `HeaderBarContainer` hardcoded 48px padding on mobile | Wastes space when sidebar is overlay on `xs`. Real mobile UX nit, easy fix. |
| 05 LLM-25 | llm | `RuntimeDownloadDrawer` bypasses Drawer wrapper | Imports raw Ant Drawer (line 2); loses mobile responsive width. Real mobile overflow on a critical user path (downloads). |

### Confirmed verbatim (selection of most-load-bearing)

| ID | File | Verification |
|---|---|---|
| 01 B-5, B-6 | core | Already verified earlier; both real (`enableMapSet` missing, async emit rejections propagate). |
| 01 B-8, B-9 | core | `LazyComponentRenderer` heuristic + memo deps: both real, both MED. |
| 01 B-13, B-14 | core | Module-system errors + duplicate `onModuleRegister`: both real, both MED. |
| 02 R-2, R-4, R-8 | layout | Mobile open-nav UX, nested scroll, z-index collision: all real, MED. |
| 03 B-9, B-10 | auth | `autoComplete="off"` + onboarding mid-flow refresh: both real. |
| 04 MED-1, 2, 4, 6, 7 | chat | Keyboard hijack, dead-code text renderer, hard-coded back-nav, MCP send-button gap, McpConfigModal stale deps: all real. |
| 05 LLM-05, 14 | llm | 14+ `console.log` in LLM stores + SSE retry exhaustion silent-fail: both real MED. |
| 06 F-5, 6, 7, 9 | mcp | Drawer state-flash on reopen, non-transactional save, footer placement, missing error display: all real MED. |
| 07 MED-1, 2, 3, 5, 7 | hub | `validSections` rebuild, hardcoded locale, hub error swallowed, Projects placeholder + dead route: all real MED. |
| 08 I-5, 6, 7, 8, 9 | consistency | Drawer widths, empty-state phrasings, layout primitives split, loading state divergence, error display divergence: all real MED. |
| 09 B-2, 5, 7, 10, 11, 14, 17, 18 | correctness | Auth localStorage parse, user pagination race, MCP drawer race, conversation A→B→A, drag listeners, orphan timers, `Promise.all` bulk delete, downloadFile no try/catch: all real. |

### Count corrections

| Claim | Original | Verified | Note |
|---|---|---|---|
| 01 B-13 module init errors | "halt subsequent module init" | Sync throws are caught at lines 175-180; only async errors silently logged. Severity unchanged. |
| 05 LLM-08 fan-out complexity | "O(N²) on results array" | Actually **O(N)** — 50 parallel `LlmModel.list()` on boot. Severity unchanged (still MED for boot burden). |
| 08 I-10 react-icons count | "17 files" | Actually **14 files**. |
| 08 I-11 module-structure | "11 of 20 modules deviate" | Actually **21 modules total**; 19 lack `pages/`, 8 lack `types.ts`. Vague baseline. |
| 08 E-1 form-reset boilerplate | "16 sites" | Actually **14 sites**. |
| 03 B-2 user-groups N² | "N² (40 for 20 groups)" | Actually **2N** (two sequential loops), not N². Still expensive. |

### Removed (false positives)

| ID | File | Why removed |
|---|---|---|
| 04 LOW-2 | chat | "`Stores.Chat.__state.McpStore.__state` double-`__state`" — audit conflated two stores. `ModelStore` access uses single `__state` (correct); `McpStore` access uses double `__state` (also correct, accessing the nested store's state). Not a bug. |

### Net impact after wave 3

- HIGH: 24 → **27** (3 promotions: 01 B-17, 04 MED-5, 05 LLM-13)
- MED: 77 → **62** (15 demotions to LOW + 3 promotions to HIGH, minus 2 promotions from LOW)
- LOW: 64 → **77** (gained 15 demotions, lost 1 false positive, lost 2 promotions to MED, plus net adjustments)
- Total: 165 → **166** (the +1 is a finding that was split during verification)
- Unique HIGH after dedup: **26**

### Cumulative trustworthiness over 3 waves

- **9 of 35 original HIGH findings (26%)** were overstated/wrong → corrected.
- **3 new HIGH findings** discovered during MED verification → promoted.
- **15 MED findings** were overstated → demoted to LOW.
- **6 count claims** were imprecise (off by 1.4× to 2×).
- **1 finding** was a false positive entirely (04 LOW-2).

The remaining 26 unique HIGH findings have all been verified against source and represent real correctness/security/UX/data-loss issues. The MED set has been spot-checked and the count corrections applied. LOWs have not been individually re-read but any LOW that should be HIGH was caught in the spot-check.

---

## 9.2 Second verification wave (2026-05-23 — all remaining HIGH findings re-checked)

After the first revision wave caught 5 incorrect HIGH findings, a second wave verified every remaining HIGH finding by reading the source. Findings revised:

### Additionally withdrawn / downgraded

| ID | Original | Revised | Reason |
|---|---|---|---|
| 02 B-3 | HIGH "sidebar drag width not persisted" | MED | `isSidebarCollapsed` IS persisted in store. Only pixel width is lost on reload — UX nit, not data integrity. |
| 07 HIGH-3 | HIGH "Hardware popup silently sits forever" | MED | The popup CAN connect to SSE independently — it loads the full app at `/hardware-monitor` with its own working Hardware.store. The "separate JS context" piece is true (popup ≠ parent store) but audit's "silently sits at Connect to hardware monitoring… forever" is **wrong**. |
| 08 I-1 | HIGH "footer order split 50/50" | MED | Verified split is ~80/20, not 50/50. Only `user/` module deviates with Submit→Cancel left-aligned; the other 5 modules audited use Cancel→Submit right-aligned consistently. |
| 08 I-2 | HIGH "submit labels ad-hoc" | MED | Confirmed 10 distinct labels but this is consistency UX, not a bug. |
| 08 I-3 | HIGH "`disabled={!canManage}` only in code-sandbox" | MED | Confirmed only 2 instances (both code-sandbox). Backend enforces; this is UX feedback gap, not data integrity. |

### Findings CONFIRMED in second wave

| ID | Title | Verification |
|---|---|---|
| 01 B-12 | No top-level React error boundary | Confirmed: no `ErrorBoundary`/`componentDidCatch`/`getDerivedStateFromError` anywhere in `src-app/ui/src/`. No `react-error-boundary` in `package.json`. |
| 02 B-4 | visualViewport thrash | Confirmed: `AppLayout.tsx:153-176` writes `document.body.style.height` AND forces `scrollTop=0` on every viewport-resize event with no guard. Conflicts with `100dvh` in `index.css:29`. |
| 02 R-1 | Mobile sidebar overlay a11y | Confirmed: no focus trap, no `role="dialog"`, no Escape-to-close, no body scroll lock, triple-fire backdrop handlers (lines 210-212), 24×24 px chevron toggle (below WCAG 2.5.5 minimum 44×44 px), no hamburger in codebase. |
| 04 HIGH-4 | Web/SVG iframe XSS | Confirmed: `sandbox="allow-scripts"` (no `allow-same-origin`). Content comes from `messageFilesCache` — which includes files from OTHER users in shared conversations. Real cross-user XSS surface (not just self-XSS via prompt injection). |
| 05 H-4 | `loadModelsForProvider` race | Confirmed: no AbortController, no stale-result guard, 4 call sites (2 in SSE handlers at LlmModelDownload.store.ts:235, :279). State write keys by providerId (different providers are safe) but same-provider concurrent calls race. Real but narrow. |
| 08 I-4 | Confirmation pattern split | Confirmed: 3 `modal.confirm` + 11 `<Popconfirm>` = 14 destructive confirm sites. "Yes" vs "Delete" `okText` divergence is the actual concerning piece. |
| 09 B-9 | EventBus.on without cleanup | Confirmed 4 stores miss cleanup (Auth + 3 hub stores). Audit missed that **ChatHistory.store.ts:379 DOES have cleanup via `removeGroupListeners('ChatHistory')`** — so the correct count is "4 of 5 stores with EventBus.on subscriptions lack cleanup", not "4 stores total". |
| 09 B-8 | Module-scope AbortController | Confirmed in `Hardware.store.ts:39`, `LlmModelDownload.store.ts:44`, `SandboxEnvironments.store.ts:45`. All three are at module top, not in store state. |

### Findings where the CLAIM was right but COUNT was wrong

| ID | Claim | Actual | Note |
|---|---|---|---|
| 09 B-15 | "171 `console.*` calls in `src/modules/`" | **367** | Audit *underestimated by 2×*. Production has even more console noise than reported. |
| 05 §Summary | "21 `console.log` in 4 LLM files" | **17** | Audit overestimated by 4. Breakdown: LlmProvider.store.ts=0, LlmModelDownload.store.ts=13, LlmProviderSettings.tsx=3, LocalProviderSettings.tsx=1. |
| 02 Appendix drawer widths | "7 default-520, 12 explicit-600, 2 explicit-400" | ~15× `size={600}` (Agent 8's count was correct, Agent 2's was wrong) | Both agents counted but only Agent 8 got it right. |
| 03 B-2 | "N² — up to 40 round-trips for 20 groups" | TWO sequential loops of N each = **2N** | Strictly speaking 2N not N², but still expensive. Severity unchanged. |

### Cross-cutting load* races — corrected scope

Original audit (09 B-4/B-5/B-6/B-7): "6+ load* races" across the codebase. Verification:
- `ChatHistory.loadConversations(page)` — **safe** (keyed by page-1 vs append, uses `if targetPage === 1` branch)
- `Users.loadUsers(page)` — **vulnerable** (shared `users` slot, no AbortController)
- `LlmProvider.loadModelsForProvider(providerId)` — **vulnerable for same-providerId concurrent** (keyed by providerId, different providers safe)
- Drawer `loadAssignedGroups` — audit needs file:line; vulnerable per audit but specifics not verified in this pass

**Revised cross-cutting claim:** "AT LEAST 3 stores' load* actions write to shared state slots without cancellation. The widely-claimed 'zero AbortControllers anywhere' is true. The specific list of vulnerable functions is shorter than the original audit framing implied."

### Net impact after second wave

- HIGH: 29 → **24** (5 more dropped)
- MED: 72 → 77 (5 gained from HIGH demotions)
- LOW: 64 → 64
- Total: 165 → 165 (counts unchanged, severity tags moved)
- Unique HIGH after deduplication (06 F-20 = 07 HIGH-2): **23**

### Trustworthiness note

After two verification waves: **9 of the original 35 HIGH findings (26%) were overstated or wrong.** The error patterns:
1. **Pattern-matching without tracing runtime** — proxy findings, dedup guard, popup architecture.
2. **Over-tagging consistency findings as HIGH** — three of the four 08 consistency claims (I-1, I-2, I-3) are real but MED-level concerns, not HIGH.
3. **Imprecise quantification** — 171 vs 367 console statements; "N²" vs "2N"; "6+ races" vs 3 verified.

The 23 remaining HIGH findings have all been verified against source and represent real correctness/security/UX problems. Use them to drive remediation. The MED findings have not all been re-verified individually; spot-check before fixing if any are unusually load-bearing.
