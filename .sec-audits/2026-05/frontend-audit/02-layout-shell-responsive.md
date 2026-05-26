# Layout shell + responsive/scroll audit

## Summary
- 4 HIGH, 6 MED, 5 LOW findings across layout shell, responsive hooks, and cross-cutting scroll containers.
- `useWindowMinSize` returns *broken semantics* — names like `xs`, `xl`, `2xl`, `3xl` are mislabeled vs the breakpoint table (HIGH); `useMainContentMinSize` has a different (also wrong) mapping for the same set. This is the single most disruptive bug for the whole app.
- AppLayout sidebar drag has no `pointer-cancel` / `mouseleave` termination, no persistence of the dragged width to `AppLayout.store`, and the resize-on-mouseleave-page case can leak listeners (MED).
- **No hamburger menu on mobile** — when `xs` flips true, sidebar is force-collapsed; mobile users open the sidebar via the `SidebarToggleButton` (fixed top-left, 24×24 px) which is *not* a hamburger icon (it's the same collapse/expand chevron used on desktop). No focus trap, no Escape key, no body scroll lock when the overlay opens.
- `SettingsPage`, `HubPage`, and `ChatHistoryPage` show **double scroll containers** (page sets `overflow-hidden` then nests a `DivScrollY` inside another `overflow-y-auto` div). HubPage is the worst — 3 nested scroll containers.
- No `<Result status="403">` or `<Result status="404">` anywhere in the codebase. The plan's deep-link permission denial story has no UI primitive yet.

## Bugs

### B-1 — `useWindowMinSize` mislabels half its breakpoints  [HIGH]
**File:** `src-app/ui/src/modules/layouts/app-layout/hooks/useWindowMinSize.ts:37-50`
**What:** The hook is documented to expose 8 breakpoints {xxs 0, xs 480, sm 640, md 768, lg 1024, xl 1280, 2xl 1536, 3xl 1920}. The actual implementation returns:
```ts
xxs:  width <= breakpointValues.xs     // <= 480  (named xxs, threshold xs) ✗
xs:   width <= breakpointValues.sm     // <= 640  (named xs,  threshold sm) ✗
sm:   width <= breakpointValues.md     // <= 768
md:   width <= breakpointValues.lg     // <= 1024
lg:   width <= breakpointValues.xl     // <= 1280
xl:   width <= breakpointValues.xl     // <= 1280 (duplicate!)             ✗
'2xl':width <= breakpointValues['xl']  // <= 1280 (also duplicate!)        ✗
'3xl':width <= breakpointValues['2xl']// <= 1536                            ✗
```
Three pairs collapse to the same threshold (`xl`, `2xl`, `xl` are all <=1280), and the entire scale is shifted by one. Every consumer that thinks `windowMinSize.xs` means "≤480px" actually gets "≤640px", and `2xl`/`3xl` are essentially aliases.
**Impact:** All consumers (10 files) are affected — `AppLayout.tsx:122,199,252`, `LeftSidebar.tsx:145,152`, `Drawer.tsx:63,122-128`, `HubPage.tsx:95,113,139`, `SettingsPage.tsx:119`, `UserLlmProvidersPage.tsx:195,213`, `LlmProviderSettings.tsx`, `ChatRightPanel.tsx:103`, `UserAssistantsPage.tsx:112,174` (via `useMainContentMinSize`). The "mobile" cutoff is effectively 640px (small tablets in portrait), not 480px as documented.
**Fix sketch:** Re-write the mapping table so each key equals its own threshold (`xs: width <= breakpointValues.xs`, etc.), and pick a consistent semantic: either "is the viewport ≤ this breakpoint" or "is the viewport AT LEAST this breakpoint". The current name "minSize" plus the `<=` operator is itself confusing — `minSize.xs === true` means "viewport is *no bigger than* xs", which is "max-size" semantics, not "min-size".

### B-2 — `useMainContentMinSize` uses yet a *different* (also wrong) mapping  [HIGH]
**File:** `src-app/ui/src/modules/layouts/app-layout/hooks/useWindowMinSize.ts:52-61`
**What:** The internal `calculateMinSize` helper used by `useMainContentMinSize` returns different values from `useWindowMinSize` for `xl`, `2xl`, `3xl`:
```ts
xl:   width <= breakpointValues['2xl']  // <= 1536
'2xl':width <= breakpointValues['3xl'] // <= 1920
'3xl':width >  breakpointValues['3xl'] // STRICT GREATER THAN — opposite polarity!
```
The `'3xl'` case is `>` while every other key is `<=`. Two consumers (`UserAssistantsPage.tsx:16`, `ModelSelector.tsx:21`, `keyboard/extension.tsx:101`) use this hook and get *different* truthiness for the same width than `useWindowMinSize` consumers do.
**Fix sketch:** Unify both hooks behind one canonical `calculateMinSize(width)` function and use `<=` throughout. The two hooks should differ only in input source (window vs main-content), never in mapping.

### B-3 — AppLayout sidebar drag does not persist width; `currentWidth` is a `useRef` shared across remounts via closure only  [HIGH]
**File:** `src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:29,53-95`
**What:** `currentWidth = useRef(200)` is the only place the drag stores its new width. The store (`AppLayout.store.ts`) has no `sidebarWidth` field. After any unmount (route change isn't an issue because `AppLayout` wraps the Outlet, but page-reload or HMR will reset it), the sidebar snaps back to 200 px. Worse, when `setSidebarCollapsed(false)` runs from a width >MIN/2 transition (line 53-91), the store updates but the *width* lives only in the ref — so a remote consumer reading the store has no way to know the real visible width. Other components must observe `mainContentWidth` (a separate ResizeObserver hack) instead of the source of truth.
**Fix sketch:** Add `sidebarWidth: number` (default 200) to `AppLayout.store`, call `setSidebarWidth` at mouse-up, and read it via `Stores.AppLayout.sidebarWidth` instead of an internal ref.

### B-4 — Visual viewport listener mutates `document.body.style.height` on every viewport change and forces scrollTop=0  [HIGH]
**File:** `src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:153-176`
**What:** Two issues:
1. `document.body.style.height = ${height}px` is written on every `visualViewport.resize` event. iOS Safari fires this event continuously during keyboard show/hide animation, causing layout thrash and competing with `100dvh` on `.ant-app` (`index.css:29`). The combination of an explicit body height + `100dvh` ancestor is internally inconsistent.
2. `document.documentElement.scrollTop = 0` runs on every viewport resize. If a user opens the keyboard while scrolled in the chat, they get yanked to top. This is data-loss-class UX (lose scroll position mid-conversation).
Also, under React StrictMode the effect mounts twice — both listeners run, but only the second cleanup unsubscribes (first cleanup ran with the same ref); not a leak but a double-fire on initial mount.
**Fix sketch:** Don't write to `document.body.style.height` if `index.css` already uses `100dvh` — pick one. Remove the unconditional `scrollTop = 0` (it should only run on initial mount, if at all).

## Inconsistencies

### I-1 — `Drawer` accepts both `size` and `width` props; codebase mixes them  [MED]
**File:** `src-app/ui/src/modules/layouts/app-layout/components/Drawer.tsx:28,40`
**What:** The custom `Drawer` wrapper documents `width` as deprecated but still accepts it. Most call sites use `size={...}` but two use `width={...}`:
- `src-app/ui/src/modules/user/components/group/UserGroupsSettings.tsx:184` — `width={600}`
- `src-app/ui/src/modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer.tsx:50` — `width={500}`
**Fix sketch:** Migrate both call sites to `size={...}` and either remove the `width` prop or delete the deprecated branch entirely.

### I-2 — `SidebarNavItem.requiresPermission` is the old name; plan calls for `permission?: PermissionExpr`  [LOW]
**File:** `src-app/ui/src/modules/layouts/app-layout/types.ts:26`
**What:** Plan assumption says the permission plan extends `SidebarNavItem` to `permission?: PermissionExpr`. The file still shows `requiresPermission?: string` and `SidebarToolItem` has no permission field at all. No coexistence — flagging only as "plan not yet applied" per the instructions.

### I-3 — `LeftSidebar.tsx` reads slot but does NOT filter by `requiresPermission`  [MED]
**File:** `src-app/ui/src/modules/layouts/app-layout/components/LeftSidebar.tsx:128-142`
**What:** Even though the type defines `requiresPermission`, nothing in `LeftSidebar.tsx` actually filters items by user permissions. The TODO is in `HubPage.tsx:27` too ("Filter by permissions (TODO: integrate permission check)"). A non-admin user sees every sidebar item that any module registers; the route guard at the destination is the only barrier.

### I-4 — `HeaderBarContainer` padding hardcoded for sidebar collapsed/expanded state; ignores responsive  [LOW]
**File:** `src-app/ui/src/modules/layouts/app-layout/components/HeaderBarContainer.tsx:22-23`
**What:** Always reserves 48px left padding when sidebar collapsed, 12px otherwise — but on mobile (`windowMinSize.xs`) the sidebar is an absolute overlay, so the 48px gap is permanently wasted leftward space on every page header. No `windowMinSize.xs` branch.

### I-5 — Two CSS height systems compete: `100dvh` on `.ant-app` vs `document.body.style.height` from JS  [MED]
**File:** `src-app/ui/src/index.css:29` + `src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:157`
**What:** `.ant-app { height: 100dvh; width: 100dvw }` is the modern approach (handles iOS toolbar). But AppLayout *also* sets `document.body.style.height = visualViewport.height` on every resize, which is the old approach. The two will disagree by tens of pixels mid-keyboard-animation on iOS. *Investigation needed: confirm on iOS device which one wins.*

### I-6 — `BlankLayout` mutates `document.documentElement.style.backgroundColor` on theme change (same as AppLayout)  [LOW]
**File:** `src-app/ui/src/modules/layouts/blank/BlankLayout.tsx:11-15` and `AppLayout.tsx:146-150`
**What:** Both layouts independently set `document.documentElement.style.backgroundColor` from the theme token — one to `colorBgLayout`, the other to `colorBgContainer`. When navigating between a `/setup` (BlankLayout, bg=Layout) and `/chat` (AppLayout, bg=Container) the document background flickers between the two. Should live in one place (theme provider or App.store), not in each layout.

### I-7 — Inconsistent page header pattern: HubPage uses `<Flex>`, others use `<div>`  [LOW]
**File:** `src-app/ui/src/modules/hub/HubPage.tsx:87` vs `SettingsPage.tsx:112` vs `ChatHistoryPage.tsx:24` vs `UserAssistantsPage.tsx:104`
**What:** All four top-level pages have a similar shell (HeaderBarContainer + flex-1 body) but the root element type varies arbitrarily. Minor consistency issue.

## Inefficiencies

### E-1 — `useWindowSize` (react-use) re-renders consumer on every pixel of resize  [MED]
**File:** `src-app/ui/src/modules/layouts/app-layout/hooks/useWindowMinSize.ts:38`
**What:** `useWindowSize` returns `{width, height}` updated on every `resize` event (no throttling). The hook then derives a `MinSize` object *on every render* — a brand new object literal every call, defeating any `React.memo` / `useMemo` on consumers that take it as a prop. With 10 consumers in the tree, every window resize triggers ~10 re-renders per event.
**Fix sketch:** Memoize the returned object with `useMemo(() => ({...}), [width])`, or better, compute only the booleans needed by each consumer and bail-out on equality.

### E-2 — `useMainContentMinSize` subscribes to store but uses untyped `state: any`  [LOW]
**File:** `src-app/ui/src/modules/layouts/app-layout/hooks/useWindowMinSize.ts:70`
**What:** The subscribe callback types `state` as `any`. Cosmetic but loses safety. Also runs full `calculateMinSize` on every state change, even when `mainContentWidth` didn't change.

### E-3 — Sidebar drag handler attaches `mousemove`/`mouseup` to `document` but never to a window-level pointercancel  [MED]
**File:** `src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:115-117`
**What:** No `pointercancel` / `blur` / `contextmenu` cleanup. If the user starts dragging, then alt-tabs to another window, the mouseup never fires, and `mousemove`/`mouseup` listeners keep running. Returning to the tab and clicking elsewhere finally clears them. Resource leak class. Same applies to `ResizeHandle.tsx:185-187`.
**Fix sketch:** Use `setPointerCapture` on the handle element, listen for `pointermove`/`pointerup`/`pointercancel`, drop the document-level listeners.

### E-4 — `ResizeHandle.tsx:38` uses `setTimeout(...,1000)` to find parent element  [LOW]
**File:** `src-app/ui/src/modules/layouts/app-layout/components/ResizeHandle.tsx:37-53`
**What:** Comment says "hack to wait for the parent to be rendered when in a portal". A 1-second blocking timeout to find a parent element is fragile — on slow devices it can race past, and any drag attempt within the first second after Drawer open does nothing. Should use a layout effect or `MutationObserver` instead.

### E-5 — `LeftSidebar` re-sorts slot arrays on every render  [LOW]
**File:** `src-app/ui/src/modules/layouts/app-layout/components/LeftSidebar.tsx:136-142`
**What:** Three `[...primaryActions].sort(...)`, etc. — no memoization. Tiny but multiplied by every render triggered by `useWindowSize` (see E-1).

## Responsive / sizing / scrolling

### R-1 — No focus trap, no Escape-to-close, no body-scroll-lock on mobile sidebar overlay  [HIGH]
**File:** `src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:199-243`
**What:** When the sidebar slides in over the mobile viewport:
- No `aria-modal` / `role="dialog"` — screen readers treat it as inline content.
- No focus trap — keyboard tab order escapes into the (now hidden) main content.
- No Escape-to-close — the only close UX is tapping the backdrop (`handleMaskClick`).
- No body scroll lock — background scrolls through the overlay on iOS.
- Backdrop click handlers wire `onClick`/`onMouseDown`/`onTouchStart` to the same callback, which fires *three times* on a touchscreen tap.
**Fix sketch:** Replace ad-hoc div with Ant's `<Drawer placement="left">` on mobile (gets focus trap, Escape, mask-click for free), or add `useFocusTrap` + `useScrollLock` hooks.

### R-2 — Mobile users have no hamburger; sidebar opens via `SidebarToggleButton` (a chevron, fixed top-left)  [MED]
**File:** `src-app/ui/src/modules/layouts/app-layout/components/SidebarToggleButton.tsx`
**What:** Confirmed — there is NO hamburger icon (`<MenuOutlined>` or similar) anywhere in the layout. Mobile UX path to open nav:
1. User on phone (≤640 px per current broken hook B-1; ≤480 px per documented intent)
2. Sees `SidebarToggleButton` at `fixed left:12 top:0`, 24×24 px, rendering `<GoSidebarExpand>` (an icon that visually resembles `[||→]`)
3. Taps it → `Stores.AppLayout.toggleSidebar()` flips `isSidebarCollapsed`
4. Overlay slides in with the mask + sidebar with `transform: translateX(0)` (AppLayout.tsx:230)
The toggle button works, but:
- 24×24 px is below WCAG 2.5.5 minimum touch-target (44×44 px AA).
- The icon is a sidebar-collapse icon, not a hamburger — users may not associate it with navigation.
- It's hidden under the page title on small phones (HeaderBarContainer reserves 48 px left padding precisely to avoid this collision on desktop, but on mobile the same padding leaves no actual space *for* the button if header content overflows).

### R-3 — `useEffect(() => { if (windowMinSize.xs) collapse }, [windowMinSize.xs])` one-way collapse, no auto-expand on resize back to desktop  [MED]
**File:** `src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:121-125`
**What:** When the window crosses from desktop → mobile, sidebar auto-collapses. Going back desktop → mobile → desktop leaves the sidebar collapsed. Whether this is "correct" is a UX call, but it's *not symmetric* and the user has to manually click toggle.

### R-4 — Triple-nested scroll container in `HubPage`  [MED]
**File:** `src-app/ui/src/modules/hub/HubPage.tsx:144-158`
**What:** 
```
<Flex overflow-hidden>                    [1]
  <div overflow-hidden>                   [2]
    <DivScrollY overflow-y-auto>          [3]  ← scrolls
      <div max-w-4xl>
        <div overflow-y-auto>             [4]  ← also scrolls!
          <div py-3>...</div>
        </div>
      </div>
    </DivScrollY>
  </div>
</Flex>
```
Two scroll containers nested (`DivScrollY` already has `overflow-y-auto`; inner `<div>` adds another). Scroll wheel events get absorbed by the inner one until it hits its limit, then propagate to the outer — classic janky double-scroll.

### R-5 — `ChatHistoryPage` empty-state never shows a header bar separator  [LOW]
**File:** `src-app/ui/src/modules/chat/pages/ChatHistoryPage.tsx:51-69`
**What:** When `conversations.length === 0`, the empty state renders `m-auto`-centered text but the `HeaderBarContainer` is still present above. Visually OK, but the page's `h-full` parent + `overflow-y-hidden` + centered `m-auto` content means if the empty state ever grows beyond viewport (unlikely with current content), it gets clipped, not scrolled.

### R-6 — `OnboardingPage` uses `h-screen` directly, bypassing the AppLayout/BlankLayout DOM tree heights  [LOW]
**File:** `src-app/ui/src/modules/onboarding/OnboardingPage.tsx:120`
**What:** `className="flex h-screen overflow-hidden"` — but the page is mounted *under* either a layout, so `h-screen` (`100vh`) ignores the AppLayout's header. If onboarding ever runs *with* AppLayout chrome (it currently uses BlankLayout via routing — need to verify), it would extend 50 px below the viewport.
*Investigation needed:* confirm Onboarding route uses BlankLayout (couldn't find the route definition in scope).

### R-7 — `AuthPage` and `SetupPage` use `min-h-screen` + BlankLayout (which itself sets `100dvh` via index.css)  [LOW]
**File:** `src-app/ui/src/modules/auth/AuthPage.tsx:29`, `src-app/ui/src/modules/app/SetupPage.tsx:58`
**What:** `Layout className="min-h-screen"` inside `BlankLayoutComponent` inside `.ant-app { height: 100dvh }`. Three height systems stacked. The form is small enough that it's just centered correctly today, but iOS Safari with on-screen keyboard could push the centered card off-screen because `min-h-screen` ≠ `100dvh`.

### R-8 — `ChatRightPanel` hardcodes `z-[1000]` on its mobile overlay; collides with AntD Drawer default 1000  [MED]
**File:** `src-app/ui/src/modules/chat/core/components/ChatRightPanel.tsx:114`
**What:** Mobile right panel sets `z-[1000]`. AntD `<Drawer>` default `zIndex` is also 1000. If a user opens a drawer (e.g. via a settings link) *while* the chat right-panel mobile overlay is open, the stacking order depends on DOM order, not z-index — bug class "two elements with same z-index, last-rendered wins".
**Fix sketch:** Use a project-wide z-index scale (probably in `theme.ts`), don't hardcode `1000`.

### R-9 — `AppLayout` sidebar overlay uses `z-3` (Tailwind arbitrary `z-3` ≈ 3) with an inline `zIndex: 3`  [LOW]
**File:** `src-app/ui/src/modules/layouts/app-layout/AppLayout.tsx:218,226`
**What:** Mixes Tailwind `z-1`/`z-2`/`z-3` arbitrary classes with `style={{ zIndex: 3 }}`. The arbitrary Tailwind values aren't standard (`z-0` to `z-50` is the defined scale; `z-3` is custom) — confirm tailwind config compiles them. Also the gap between sidebar z and AntD drawer z (1000) is huge — fine, but `1000-3=997` worth of unused z space is brittle if anything injects a portal in between.

### R-10 — No `<Result status="403">` / `<Result status="404">` anywhere  [MED]
**File:** (no file — absence)
**What:** Searched all of `src-app/ui/src/` for `status="403"`, `status="404"`, and any `<Result>` import — zero hits. The plan assumes `SettingsPage` renders `<Result status="403">` inline for deep-link denials, but the primitive doesn't exist in the codebase yet. The auth flow currently redirects (`AuthGuard` → `/auth/login`), and unknown routes hit `<Navigate to="/" />` (RouterComponent.tsx:130-136) — no 404 page either.
**Fix sketch:** Add a shared `<PermissionDeniedResult>` and `<NotFoundResult>` primitive in `src/components/common/` before the plan extension lands.

### R-11 — `ConversationPage` puts `messagesEndRef` outside the scroll container, breaking `scrollIntoView` on auto-scroll  [MED]
**File:** `src-app/ui/src/modules/chat/pages/ConversationPage.tsx:73-77`
**What:** The scroll container is the `overflow-y-auto` div on line 73; `messagesEndRef` is nested inside it. Auto-scroll via `scrollIntoView({behavior:'smooth'})` will trigger scrolling on the *nearest scrollable ancestor*, which is the right element here — so this case is OK. But the `useEffect` has no deps array shown to me in the snippet read; actually it's `[messages]` (line 28), and on Strict-Mode double-invoke, the smooth-scroll fires twice. Minor.

### R-12 — `100dvh` impact on iOS Safari keyboard (`.ant-app`)  [LOW — investigation needed]
**File:** `src-app/ui/src/index.css:29-31`
**What:** `100dvh` resolves to the dynamic viewport height (shrinks when keyboard opens). Combined with the JS `document.body.style.height = visualViewport.height` (B-4), the page potentially gets *two* heights. Investigation needed on a real iOS device to confirm which wins (likely body's inline style overrides ancestor CSS once it's applied), and whether chat input remains accessible when keyboard is up.

## Appendix: Drawer width table

| File:line | Width | Module |
|---|---|---|
| `src-app/ui/src/modules/layouts/app-layout/components/Drawer.tsx:28` | **520 (default)** | layouts/app-layout |
| `src-app/ui/src/modules/mcp/components/system/GroupSystemMcpServersAssignmentDrawer.tsx:80` | size=600 | mcp |
| `src-app/ui/src/modules/mcp/components/system/McpServerGroupsAssignmentDrawer.tsx:92` | size=600 | mcp |
| `src-app/ui/src/modules/mcp/components/common/McpServerDrawer.tsx:317` | size=600 | mcp |
| `src-app/ui/src/modules/llm-provider/components/LlmProviderDrawer.tsx:97` | size=600 | llm-provider |
| `src-app/ui/src/modules/llm-provider/components/LlmProviderGroupsAssignmentDrawer.tsx:110` | size=600 | llm-provider |
| `src-app/ui/src/modules/llm-provider/components/GroupLlmProvidersAssignmentDrawer.tsx:79` | size=600 | llm-provider |
| `src-app/ui/src/modules/llm-provider/components/llm-models/AddRemoteLlmModelDrawer.tsx:69+` | default 520 | llm-provider |
| `src-app/ui/src/modules/llm-provider/components/llm-models/EditLlmModelDrawer.tsx:86+` | default 520 | llm-provider |
| `src-app/ui/src/modules/llm-provider/components/llm-models/AddLocalLlmModelDownloadDrawer.tsx:203+` | default 520 | llm-provider |
| `src-app/ui/src/modules/llm-provider/components/llm-models/AddLocalLlmModelUploadDrawer.tsx:373+` | default 520 | llm-provider |
| `src-app/ui/src/modules/hub/modules/assistants/components/AssistantDetailsDrawer.tsx:21` | default 520 | hub |
| `src-app/ui/src/modules/hub/modules/mcp/components/McpServerDetailsDrawer.tsx:22` | default 520 | hub |
| `src-app/ui/src/modules/hub/modules/llm-models/components/ModelDetailsDrawer.tsx:22` | default 520 | hub |
| `src-app/ui/src/modules/llm-repository/components/LlmRepositoryDrawer.tsx:193` | size=600 | llm-repository |
| `src-app/ui/src/modules/user/components/user/ResetPasswordDrawer.tsx:26+` | default 520 | user |
| `src-app/ui/src/modules/user/components/user/AssignGroupDrawer.tsx:28+` | default 520 | user |
| `src-app/ui/src/modules/user/components/user/CreateUserDrawer.tsx:75` | size=600 | user |
| `src-app/ui/src/modules/user/components/user/EditUserDrawer.tsx:91` | size=600 | user |
| `src-app/ui/src/modules/user/components/user/UserGroupsDrawer.tsx:94` | size=400 | user |
| `src-app/ui/src/modules/user/components/group/GroupMembersDrawer.tsx:28` | size=400 | user |
| `src-app/ui/src/modules/user/components/group/EditUserGroupDrawer.tsx:105` | size=600 | user |
| `src-app/ui/src/modules/user/components/group/UserGroupsSettings.tsx:184` | **width=600** (deprecated prop) | user |
| `src-app/ui/src/modules/assistants/components/AssistantFormDrawer.tsx:155` | size=600 | assistants |
| `src-app/ui/src/modules/llm-local-runtime/components/drawers/RuntimeDownloadDrawer.tsx:50` | **width=500** (deprecated prop) | llm-local-runtime |

**Divergence:** 520 (default, ~7 Drawers), 600 (dominant explicit, 12 Drawers), 400 (2 Drawers — both user-group member lists), 500 (1 Drawer, also using deprecated `width=`). Two call sites still use the deprecated `width` prop. No standardization on small/medium/large semantic sizes — every site picks a number.

## Appendix: Scroll container ownership

| Page | Container owner | Risk |
|---|---|---|
| `SetupPage.tsx` | `min-h-screen flex items-center` — body scroll | OK (small content) |
| `AuthPage.tsx` | `min-h-screen` inside BlankLayout — body scroll | OK |
| `OnboardingPage.tsx:194` | `flex-1 overflow-y-auto p-6` — own container in step content | OK; left pane also `overflow-y-auto` (R-6) |
| `HubPage.tsx:145-158` | **3 nested scrolls:** outer flex, DivScrollY, inner `overflow-y-auto` | **R-4 HIGH** double-scroll |
| `SettingsPage.tsx` | parent `overflow-hidden`, defers to `<Outlet>` child | OK at this level |
| `SettingsPageContainer.tsx:19` | `DivScrollY h-full` (single scroll) | OK |
| `ChatHistoryPage.tsx:43` | `DivScrollY` for list + outer `overflow-y-hidden` | OK (single scroll); empty state has `m-auto` which doesn't scroll if oversized (R-5) |
| `NewChatPage.tsx:29` | `flex flex-col h-full items-center justify-center` — no scroll | OK (content is fixed-height welcome screen) |
| `ConversationPage.tsx:73` | inner `overflow-y-auto` div is the scroll container | OK single scroll; auto-scroll behavior intact |
| `ChatRightPanel.tsx:119,141` | inner `flex-1 overflow-hidden` then child handles its own scroll | OK |
| `UserAssistantsPage.tsx:190` | `h-full flex flex-col overflow-y-auto` (single) | OK |
| `UserLlmProvidersPage.tsx:202,210` | sidebar `flex-1 overflow-y-auto` + main `flex flex-col py-3 px-3 overflow-y-auto` (TWO sibling scrolls) | MED — two independent scroll columns is intentional; but no shared scroll-sync |
| `ProjectsPage.tsx` | `p-8` — body scroll (page is stub) | OK (placeholder) |
| `SystemMcpServersPage.tsx` | wraps in `SettingsPageContainer` (DivScrollY) — single | OK |
| `SandboxSettingsPage.tsx` | wraps in `SettingsPageContainer` — single | OK |
| `LlmProviderSettings.tsx` | reads `windowMinSize` for layout; uses Outlet-style mobile dropdown vs desktop sidebar; scroll likely in child component | INVESTIGATION NEEDED — separate audit, but flagged |
| `LlmRepositoryDrawer.tsx` / other Drawer bodies | `DivScrollY` inside `<AntDrawer>` body (Drawer.tsx:153) | OK (Drawer wrapper standardized) |

**Compound-scroll offenders (body + child both scroll):**
- HubPage (R-4) — 3 levels
- UserLlmProvidersPage — sibling scrolls on two columns (intentional but no scroll-sync)

**Pages relying on body scroll instead of own container:**
- SetupPage, AuthPage, ProjectsPage (the last is a stub). Fine for short pages but if any grow they'll need wrapping.

**Z-index hotspots referenced in cross-page sweep:**
- `ChatRightPanel.tsx:114` — `z-[1000]` collides with default AntD `<Drawer>` z-index (R-8)
- `AppLayout.tsx:218,226` — `z-1`/`z-3` Tailwind arbitrary + inline `zIndex:3` mix (R-9)
- `SidebarToggleButton.tsx:10` — `fixed z-10` on its own scale
- No project-wide z-index scale exists.
