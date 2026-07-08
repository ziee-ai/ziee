# DECISIONS — message-scroll-stability

### DEC-1: What is the default fixed height for an inline file-view body, and the resize min/max?
**Resolution:** Generic viewers (text/markdown/image/etc.) default **400px**; the `inlineFill` tabular grid keeps **min(360px, 55vh)** (its existing definite-height it already needs to row-virtualize). Resize handle clamps to **[160px, 80vh]**. These become the `inlineFileHeight.ts` constants.
**Basis:** codebase — reuses the existing `max-h-[600px]` / `max-h-[min(360px,55vh)]` caps from InlineFilePreview as the ceiling family, and DelimitedTable/XlsxBody's `h-[min(360px,55vh)]` for tabular; 400 (< the old 600 ceiling) trims wasted space while staying stable.

### DEC-2: A short file in a tall fixed box leaves empty space — accept, or measure-and-lock to content?
**Resolution:** Accept the fixed capped height with internal scroll (no measure-and-lock). Empty space for short bodies is the deliberate trade; the ITEM-3 drag handle + persisted per-file height lets a user shrink any preview they care about.
**Basis:** user — the requester explicitly specified "fixed (capped) height with internal scroll" + "optional bottom drag-resize handle". Measure-and-lock reintroduces exactly the one post-mount delta this feature exists to remove.

### DEC-3: Does the fixed-height treatment apply to text message bubbles too, or only inline file views?
**Resolution:** Only inline **file views** (InlineFilePreview bodies) get a fixed height. Text/markdown bubbles are NOT height-capped by this feature; their post-settle height is stable once streaming/find/Shiki finish (and the show-more clamp already bounds the tall ones). The show-more fix for text is state-lift (ITEM-4), not fixed height.
**Basis:** codebase — the measured churn source is the lazy-mounted, content-driven file body (image decode / table parse); text reflow after settle is not a recurring per-scroll delta. Capping text would harm readability for no stability gain.

### DEC-4: Is the lifted view-state store per-conversation (reset on switch) or globally persistent?
**Resolution:** Per-conversation, reset on conversation switch (in-memory only; not persisted to disk/localStorage).
**Basis:** convention — mirrors `Chat.store`'s `conversationStateCache` lifecycle (cleared when the message window clears). Expand/resize choices are a within-session reading aid, not durable user settings.

### DEC-5: What key identifies inline-file view-state — file id or resource_link URI?
**Resolution:** The `resource_link` URI (`source.url`). Message-collapse state keys on `message.id`. The two live in separate maps so the key spaces never collide.
**Basis:** codebase — `InlineFilePreview` always has `source.url` (unique per link) but `file.id` is absent for external-MCP URL-based links; the URI is the one always-present stable identifier, and `MessageFilesView` already dedupes by URI.

### DEC-6: How is the resize handle made accessible (keyboard + touch + AT)?
**Resolution:** The handle is a `role="separator"` `aria-orientation="horizontal"` control with `aria-label="Resize file preview"`, `tabindex=0`, `aria-valuenow/min/max` (px); ArrowUp/ArrowDown resize by 24px steps, Home/End jump to min/max. Pointer drag uses `setPointerCapture` (works for touch + mouse) and does not preventDefault scroll outside the drag.
**Basis:** convention — WCAG 2.1.1 (keyboard) + the app's DoD (zero a11y HIGH findings); mirrors the ARIA slider/separator pattern. Addresses the PLAN_AUDIT ITEM-3 CONCERN.

### DEC-7: Keep `@tanstack/react-virtual`, or migrate the message list to `content-visibility:auto` (angle D)?
**Resolution:** Keep react-virtual for this feature. Fix within it via ITEM-2 (stable heights) + ITEM-4/5 (state lift) + ITEM-7 (in-place anchor). Migration to browser-native `content-visibility` is NOT in scope; it will be proposed separately ONLY if ITEM-1's post-fix measurement still shows a recorrection storm.
**Basis:** user — the requester said "only propose that, don't rip out virtualization without a plan-ack." Stable heights remove the recorrection trigger, so the anchor-restore / jump / lazy-load investment stays intact. `content-visibility` is already proven in-tree (RawCodeView) as the fallback path if needed.

### DEC-8: Is the correction-count instrumentation shipped in production?
**Resolution:** No. It is gated by `import.meta.env.DEV` and attached to `window.__MSGLIST_METRICS__` only in dev/gallery builds; tree-shaken out of the production bundle.
**Basis:** convention — mirrors the existing DEV-only `window.__GALLERY_OVERLAYS__` gallery hook; production must carry zero measurement overhead.

### DEC-9: Does overscan change?
**Resolution:** No — overscan stays at 8.
**Basis:** codebase — commit `92563b6c` proved dropping it to 4 regresses the prepend anchor (~120px drift). The state-lift (ITEM-4/5) makes remount cheap+lossless regardless of overscan, so there is no reason to touch it.

### DEC-10: On remount with persisted `seen=true`, does the body still defer its first fetch on initial page load?
**Resolution:** Yes on FIRST load — the store starts empty per conversation, so every preview begins `seen=false` and the initial-scroll-to-bottom keeps off-screen previews unfetched (unchanged). `seen` only becomes true after a preview has actually entered view once; thereafter remount renders it immediately (which is correct — it was already fetched).
**Basis:** codebase — preserves the existing lazy-fetch contract (ConversationPage instant initial scroll) while eliminating the remount re-lazy-mount churn. Addresses the PLAN_AUDIT ITEM-5 CONCERN.
