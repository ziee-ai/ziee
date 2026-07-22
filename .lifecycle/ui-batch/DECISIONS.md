# DECISIONS — ui-batch

Every human/product input the implementation needs, resolved up front. Zero
open questions remain. Product choices were put to the human as explicit option
pickers at plan time (Basis: `user`); everything else is resolved by codebase
precedent (Basis: `codebase` / `convention`).

### DEC-1: Should the model-selector trigger keep a fixed width cap, or size to its content?

**Resolution:** Size to content. Drop `max-w-[130px]`, add `w-auto` so the trigger
grows to fit the selected model name, with truncation reserved for genuine
pressure. "GPT OSS 120B" and most real names render in full in a normal wide
composer with no ellipsis at all.
**Basis:** user — presented as an option picker ("Widen to ~176px" vs "Keep 130px,
only add the ellipsis"); the human chose the wider treatment and then refined it
further to fully dynamic: *"make it DYNAMIC instead of a fixed cap — always show
the full model name whenever there is room, ellipsis only under real pressure.
Drop the fixed max-w entirely."*

### DEC-2: Is a toolbar that reflows when the model name changes acceptable?

**Resolution:** Yes — accepted explicitly. A content-sized trigger means the
composer toolbar shifts when switching between a short and a long model name. Fit
is preferred over a stable width.
**Basis:** user — *"Accepted tradeoff: the toolbar reflows when switching between
short and long model names — that is fine, we prefer fit over stable width."*

### DEC-3: What bounds a pathological model name — a container-query percentage or a static max-width?

**Resolution:** A static, generous `max-w-[20rem]` soft ceiling. Rejected the
"~45% of composer width" reading via `@container` + `cqw`: container queries ARE
available in this build (the kit already uses `@container/card-header`), but
`container-type: inline-size` implies `contain: layout`, which changes the
containing block for the composer's positioned descendants. That is real risk for
no benefit here, because the ceiling is not what handles pressure — flex shrink
is (DEC-9). The ceiling only binds when there IS room, so a generous static value
is sufficient and side-effect-free.
**Basis:** codebase — the only in-repo container-query precedent is a kit Card
internal; no app-level surface establishes a container, and the composer root
carries positioned descendants.

### DEC-4: Which left edge should the three sidebar captions share — 12px or 20px?

**Resolution:** 12px, matching "Recent chats". "Navigation" and "Tools" move left;
no menu row moves. The consequence — every caption then hangs 8px left of its own
rows — is accepted, because that is exactly the relationship "Recent chats"
already has with its rows today, so the result is internally consistent.
**Basis:** user — presented as an option picker with rendered previews of both
alignments (12px "match Recent chats" vs 20px "move Recent chats right"), after
measuring that all rows sit at 20px and only the Recent-chats caption is the
12px outlier. The human chose 12px.

### DEC-5: How is the 12px inset achieved — a kit seam, a negative margin, or dropping the group wrapper?

**Resolution:** Drop the kit Menu's `{ type: 'group' }` wrapper for Navigation and
Tools and render a shared caption component as a sibling above a flat `<Menu>`.
Verified first that no seam exists: the full `MenuProps` + `MenuItem` union in
`sdk/packages/kit/src/kit/menu.tsx` exposes **no** `titleClassName`,
`labelClassName`, or per-item `className`; the group title is a hardcoded
`<div className="px-3 py-1 text-xs font-medium text-muted-foreground">` at
`menu.tsx:174`, and `MenuProps` reaches only the `<ul>` (`className`) and the
`<nav>` (`style`). The one in-place lever — `label` is `React.ReactNode`, so
`label: <span className="-ms-2">Navigation</span>` would cancel the inset — is
rejected: `-ms-2` is a magic number coupled to the Menu's own internal `px-2`
(silently wrong if the sidebar's padding ever changes) and it leaves the
typography drift unfixed. Forking the kit is out of scope regardless: `sdk/` is a
separate submodule repo needing its own PR plus a pointer bump.
**Basis:** codebase — confirmed by reading the kit source, at the human's explicit
instruction to check for a seam before accepting the structural change.

### DEC-6: Is it acceptable that collapsing the split also drops the persisted workspace, so browser-Back does not restore it?

**Resolution:** Yes. `reset()` empties the workspace, and the store's debounced
`saveWorkspace` removes an empty workspace from storage, so navigating Back after
New Chat lands on a single-pane view rather than resurrecting the old split. This
is the intended semantics: New Chat means a fresh single-pane surface.
**Basis:** user — *"CONFIRMED — we accept the deliberate side effect that browser-Back
after New Chat will not resurrect the old split; New Chat means a fresh
single-pane surface, proceed as planned."*

### DEC-7: Where does the split-collapse live — the `/chat` route, or the sidebar "New Chat" handler?

**Resolution:** The route — `NewChatPage`'s mount effect, beside the
`Stores.Chat.reset()` already there. This states the invariant where it belongs
("`/chat` is a single-pane surface") so it holds for every entry: the sidebar
action, `ChatHistoryPage.tsx:136,188`, `OnboardingPage.tsx:129,161`,
`useClosePane`'s `navigate('/chat')`, and a deep link. Patching only the sidebar
handler would leave the other five paths able to strand a stale split.
**Basis:** convention — mirrors the existing `reset()` call sites, which all pair
the collapse with the navigation that makes it true
(`useOpenConversation.ts:82,143`; `useNavigateAwayOnDelete.ts:60`).

### DEC-8: Does the truncated trigger need a tooltip or `title` showing the full name?

**Resolution:** No. The open dropdown already renders every model name in full —
`popupMatchSelectWidth={false}` (`ModelSelector.tsx:142`) lets the popup size past
the trigger — which satisfies the requirement that the truncated choice stay
legible. A `Tooltip` wrapping a `Select` trigger would put two triggers on one
node, which this codebase explicitly treats as a bug (`ChatInput.tsx:84-87`:
"two triggers on ONE node thrash and flicker"). After DEC-1, truncation is also
now the rare case rather than the norm.
**Basis:** convention — the anti-pattern is documented at the sibling call site in
the same file being edited.

### DEC-9: What actually absorbs the pressure when the composer is narrow?

**Resolution:** Flex shrink inside the toolbar's right group, not the width cap.
The right group loses its blanket `shrink-0` and gains `min-w-0`; the Send
`Button` takes `shrink-0` individually; the `toolbar_model` slot gets `min-w-0`.
Send therefore cannot shrink, and the model label yields first. This is what makes
a generous ceiling (DEC-3) safe.
**Basis:** user — *"place the selector in a flex context that yields BEFORE the send
button (send stays shrink-0, the model name gives way first)."*

### DEC-10: Where does the shared caption component live?

**Resolution:** `src-app/ui/src/components/common/SidebarSectionTitle.tsx`. Both
consumers are in different modules (`layouts` and `chat`), so a module-owned
location would force a cross-module import. Bonus verified during the plan audit:
`src/components/common/` is outside the gallery's `surfaceRoots`
(`["src/modules", "src/components/ui"]`, `src-app/ui/gallery.config.json:3`), so a
component there needs no `coverage.ts` entry, no state-matrix cell, and no regen —
confirmed empirically, since none of its existing `.tsx` siblings
(`ListPagination`, `SettingsSectionStatus`, …) appear in the coverage artifacts.
**Basis:** codebase.

### DEC-11: Does the icon-only (collapsed) sidebar change?

**Resolution:** No — behavior is preserved exactly. This sidebar never passes the
kit's `collapsed` prop (`grep collapsed LeftSidebar.tsx` → only a comment at
`:110`); icon-only mode is implemented by nulling each item's `label` (`:119,125,131`).
The kit's `{!collapsed && …}` caption guard is therefore inert here, meaning the
"Navigation"/"Tools" captions already render as text in the icon rail TODAY. The
replacement renders under the same unconditional condition. Whether a text caption
belongs in an icon rail is a genuine pre-existing question, but changing it would
smuggle an unrelated behavior change into an alignment fix.
**Basis:** codebase — deliberate scope boundary, recorded so it reads as a decision
rather than an oversight.

### DEC-12: Does this feature introduce any operational tunable that must become an admin-configurable setting?

**Resolution:** No. Applying the mandatory configurable-settings rule item by item:
the `max-w-[20rem]` ceiling and the caption's padding token are **layout
constants**, not operational tunables — they are not resource limits, retention
periods, rate/quota limits, concurrency caps, feature toggles, model selection, or
thresholds. They carry no operator-facing consequence and no security boundary;
they are pure presentation, of exactly the same kind as every other spacing value
in the design system, whose single source of truth is `src/index.css` +
`DESIGN_SYSTEM.md` rather than a settings row. Promoting a CSS max-width to a
database-backed admin setting would be unprecedented in this codebase and would
add a settings table, migration, permission pair, sync entity and admin card for
zero operator value. No settings row, migration, or `<feature>::settings::*`
permission is created.
**Basis:** convention — the existing singleton-settings pattern
(`code_sandbox_settings` / `session_settings` / `memory_admin_settings`) is
reserved for runtime/operational limits; presentation constants live in the design
system.
