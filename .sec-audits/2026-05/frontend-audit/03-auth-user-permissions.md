# Auth, User, Permissions audit

Scope: `src/modules/auth/`, `src/modules/user/`, `src/modules/user-profile/`,
`src/modules/onboarding/`, `src/core/permissions/` (planned).
Lenses (emphasis order): Bugs → Inconsistencies → Inefficiencies → Responsive.

## Summary

The permission-gating plan (`there-is-another-serene-coral.md`) is **not yet
applied**. `src/core/permissions/` does not exist; no slot type carries
`permission?: PermissionExpr`; no module file imports `<Can>` or
`usePermission`; the only permission helper in the tree is still the
buggy single-colon split in
`src/modules/code-sandbox/components/SandboxEnvironmentsSection.tsx:17-23`.
Every admin button (delete user, deactivate, reset password, manage
groups, register-enable toggle) renders for every authenticated user; the
sidebar entries `Users` and `User Groups` show up for everyone regardless
of `users::read` / `groups::read`.

Within the scope itself, the biggest correctness problems are
**independent of the plan**: the user-edit drawer drops the `display_name`
on save (Edit form never renders the field, sends `undefined` and the
backend silently clears it); the "Groups" drawer N²-fetches every group's
members on open (no membership endpoint is being used); the
`UserRegistrationSettings` switch is a complete no-op stub that lies to
the user; the `Auth.store` login/register/logout path emits **no events**
(stores subscribing to "current user changed" will never react); and the
"non-root admin trying to edit themselves" UI offers a Delete button that
will 403 even with the right permission, with no client-side guard.

Top 5 findings (most severe first):
1. **B-1 HIGH** — `EditUserDrawer` drops `display_name` on every save.
2. **B-2 HIGH** — `UserGroupsDrawer` triggers N² API calls and races on store.
3. **B-3 HIGH** — `Auth.store` never emits events for login/register/logout.
4. **B-4 HIGH** — `UserRegistrationSettings` is a UI lie (stub, no API).
5. **B-5 HIGH** — Plan-compliance: zero permission gating applied anywhere
   in scope; every admin-only button is reachable by every authenticated
   user (backend 403s, UI doesn't know).

---

## Bugs (B-N)

### B-1 HIGH — `EditUserDrawer` silently drops `display_name`
`src/modules/user/components/user/EditUserDrawer.tsx:46-69`

`setFieldsValue` populates `username`, `email`, `is_active`, `permissions` but
NOT `display_name`. `handleEditUser` then constructs `UpdateUserRequest`
**without** `display_name` either, so the field is sent as `undefined`. Two
problems:

1. The form is missing the field entirely — admin can't change a user's
   display name from the UI.
2. Depending on how the backend interprets a missing `display_name`
   (PATCH-style merge vs. PUT-style overwrite), repeated saves may
   round-trip-clear the field. The handler at
   `server/src/modules/user/handlers/user.rs:202-212` passes
   `request.display_name` directly to `Repos.user.update(…)` — `Option<String>` —
   so `None` is "no change", but the form still can't ever set it. Either
   way the UI is broken: there is no way to edit display_name through
   `EditUserDrawer`, even though `CreateUserDrawer.tsx:109-111` does
   collect it on create.

Fix: add the `display_name` Form.Item in the edit drawer; include it in
`setFieldsValue` initialization and in the `UpdateUserRequest`.

### B-2 HIGH — `UserGroupsDrawer` makes O(N²) calls and races on shared store state
`src/modules/user/components/user/UserGroupsDrawer.tsx:18-52`

To compute "which groups does this user belong to?", the drawer loops
**twice** over `groups`:

```ts
const membershipPromises = groups.map(async group => {
  await Stores.UserGroups.loadUserGroupMembers(group.id)
  …
})
await Promise.all(membershipPromises)

// then AGAIN, sequentially:
for (const group of groups) {
  await Stores.UserGroups.loadUserGroupMembers(group.id)
  const members = Stores.UserGroups.__state.currentGroupMembers
  …
}
```

Two N round trips per drawer open. Worse, the second loop reads
`Stores.UserGroups.__state.currentGroupMembers` which is **shared,
mutable, last-write-wins** — the in-flight first loop is racing with the
second loop, and `loadUserGroupMembers` itself contains an early-return
guard at `UserGroups.store.ts:230-237` that bails when
`loadingGroupMembers && currentGroupId === groupId`, so the parallel
calls in the first `Promise.all` largely no-op against each other.

If the user has 20 groups, this drawer fires up to 40 sequential network
calls, half of them no-ops via the early-return, and the membership
detection only works because the **sequential** second loop happens to
overwrite `currentGroupMembers` after each call.

Real fix: add a backend endpoint `GET /api/users/{id}/groups` (the user
detail page already needs this for B-3 follow-on work). Workaround:
collapse the two loops into one sequential one and stop double-calling.

### B-3 HIGH — `Auth.store` emits no events on login / register / logout / `me`
`src/modules/auth/Auth.store.ts:65-163, 185-224`

The plan's "Stale permissions" concern (lines 619-625) calls out that the
helper must re-fetch on permission changes, but a deeper problem is that
even the *initial* auth flow doesn't notify the rest of the app. The only
event the auth store interacts with is `onboarding.user_updated` (subscribed
to in `__init__.__store__`, see lines 167-181) — it never emits anything
itself. Specifically missing:

- `authenticateUser` (login) — sets `user`, `token`, `permissions[]` would
  need to be re-fetched, but no event fires. Any store that wants to
  "load on login" has to poll `Stores.Auth.isAuthenticated`.
- `registerNewUser` — same.
- `logoutUser` — same. Critically, on logout `users[]`, `chats[]`,
  `messages[]`, conversation drafts etc. should all be cleared. They
  aren't (other stores have no signal).
- `initAuth` — on fresh `/me` load (token in localStorage, page reload),
  no `auth.session_restored` event. Stores that depend on permissions
  (or even just user identity, e.g. for default assistants per user) have
  to assume.

Fix: declare an `auth.*` event family (`auth.logged_in`, `auth.logged_out`,
`auth.session_restored`, `auth.user_updated`, `auth.permissions_changed`)
and emit at the end of each successful mutation. Pair with the plan's
"re-fetch on tab focus" wiring.

### B-4 HIGH — `UserRegistrationSettings` switch is a no-op stub
`src/modules/user/components/user/UserRegistrationSettings.tsx:1-67`,
`src/modules/user/stores/Users.store.ts:295-355`

The store's `loadUserRegistrationSettings` and
`updateUserRegistrationSettings` are both `TODO: Replace with actual API
call when backend endpoint exists` (lines 309, 338). They hard-code
`true` on load and just `set({ userRegistrationEnabled: enabled })` on
write. The Switch component reports "User registration enabled
successfully" via `message.success` regardless. This is a UI lie — an
admin who toggles the switch will see "Disabled successfully" and a
disabled-looking control, and then any new visitor will still be able to
register.

If the feature isn't implemented backend-side yet, the entire card should
be hidden behind a feature flag or removed; do not ship a fake setting.

### B-5 HIGH — Plan-compliance: zero permission gating applied in scope
Every file in scope.

- `src/core/permissions/` does not exist (confirmed via `ls`).
- `Auth.store.ts:22` does store `permissions?: string[]` and
  `initAuth` populates it from `/api/auth/me`, but **no UI in the entire
  scope reads it**.
- `user/module.tsx:88-103` registers two `settingsAdminPages` slot entries
  with no `permission` field; `onboarding/module.tsx:33-41` registers a
  `sidebarTools` entry with no `permission` field; `auth/module.tsx` has
  no slot.
- `UsersSettings.tsx:99-148` renders Edit / Reset Password / Groups /
  Delete buttons unconditionally; the deactivate Switch is wrapped in a
  Popconfirm but not gated by `users::toggle_status`.
- `UserGroupsSettings.tsx:124-129, 153-220` renders the "+" / Edit /
  Delete / Members / Create-Group affordances without checking
  `groups::create` / `groups::edit` / `groups::delete` /
  `groups::read`.
- `CreateUserDrawer.tsx:78-138`, `EditUserDrawer.tsx:94-145`,
  `ResetPasswordDrawer.tsx:36-86`, `AssignGroupDrawer.tsx:38-77`,
  `EditUserGroupDrawer.tsx:108-156` — none derive `canManage` from a
  permission check; submit buttons always show, form inputs are never
  `disabled={!canManage}`.
- `code-sandbox/components/SandboxEnvironmentsSection.tsx:17-23` is the
  only permission helper in the tree, and it has the documented
  `is_admin` short-circuit gap AND uses `permission.indexOf(':')` against
  permission strings that are `::`-separated — meaning `code_sandbox::*`
  in a permissions array would NOT match `code_sandbox::environments::manage`
  with this helper (it splits at the first `:` and tries to match
  `code_sandbox:*`, which doesn't exist).

Recommendation: ship the plan's foundation in scope, migrate the sandbox
helper, and apply gates to every audit-table surface in this module before
any new admin features land.

### B-6 HIGH — Non-root admin cannot delete themselves but UI still shows Delete
`src/modules/user/components/user/UsersSettings.tsx:132-149`,
`src/modules/user/components/group/GroupListItem.tsx:74-91`

The backend has self-lockout guards only for root admin
(`server/src/modules/user/handlers/user.rs:178, 261` — refuses to deactivate
admin). It does NOT prevent a non-root user with `users::delete` from
deleting themselves, which would lock them out mid-session. The UI offers
the Delete button on the user's own row with no client guard.

Additionally, the row for the root admin **shows** all the same controls
(Delete, Deactivate-Switch) — the backend will 403/400 on the actual call
but the user sees the button and clicks it and gets a toast.

Fix: hide Delete on `user.id === Stores.Auth.user?.id` (self), and hide
Delete + Switch on `user.is_admin`. Plan should call this out as a
self-lockout guard separate from the permission helper.

### B-7 MED — `AuthGuard` navigates during render (React warning) and races initAuth
`src/modules/auth/AuthGuard.tsx:44-48, 55-62`

```tsx
if (needsSetup) {
  navigate('/setup', { replace: true })   // navigate() in render path
  return null
}
…
if (user && !isOnGuideRoute) {
  const firstIncomplete = guides.find(…)
  if (firstIncomplete) {
    navigate(`/onboarding?id=${firstIncomplete.id}`, { replace: true })
    return null
  }
}
```

`useNavigate()` should be called inside an effect, not during the render
phase. React-Router v6 warns about this and in some scenarios it loops.
The `needsSetup` case at line 45 is even hit synchronously on first render
before the effect at lines 26-31 has a chance — both paths fire, which is
a redundant double-navigate.

Also: `initAuth` (line 23) is called in a `useEffect(..., [])` — fine —
but `initAuth` sets `isInitializing: false` only at the end of the try
block (`Auth.store.ts:202`). If a stale token returns 401, the catch
branch (lines 211-223) sets `isInitializing: false`, `isAuthenticated:
false`, `token: null`. Good. **But** the persisted Zustand state
(`persist` middleware at line 60, `partialize: state => ({ token })` at
line 228) doesn't clear localStorage on logout — `logoutUser` calls
`set({ token: null })`, which the persist middleware should serialize,
but the **error path of `initAuth` does not call `Stores.Auth.logoutUser`**;
it just sets `token: null` locally. If two tabs are open and one logs in
again, the partialize for that tab persists the new token, but the other
tab is still racing. (See E-3 for the broader stale-permissions issue.)

Fix: move navigation into the existing `useEffect` (extend with `user`,
`isAuthenticated`, `guides` deps). Have the initAuth error branch call
the existing `logoutUser` (or a `clearAuth()` helper) so persistence is
consistent.

### B-8 MED — `Auth.store` `isInitializing` initial value misleads the guard
`src/modules/auth/Auth.store.ts:54`

`isInitializing: true` is the initial value, and `initAuth()` sets it
to `false`. But `partialize` (line 228) only persists `token`, so
`isInitializing` defaults to `true` on every page load — correct. However
the **default state** at line 48-56 is reused only for the *initial create*;
on logout the store calls `set({ user: null, token: null, isAuthenticated:
false, isLoading: false, error: null })` (lines 106-112) which leaves
`isInitializing` at whatever it was (`false`). Subsequent navigation back
to a protected route while logged out will see `isInitializing: false`,
`isAuthenticated: false` — the guard returns `<AuthPage />` — correct, but
this only works by accident, not by intent.

Cosmetic fix: have `logoutUser` set `isInitializing: false` explicitly
and document. Add a `__destroy__` / reset helper to centralize.

### B-9 MED — `LoginForm` autoComplete="off" on `<Form>` blocks password managers
`src/modules/auth/LoginForm.tsx:50`, `RegisterForm.tsx:55`

Both forms set `autoComplete="off"` on the parent `<Form>`. The
individual inputs do set `autoComplete="username"` / `current-password`
/ `new-password` correctly, but browser behavior with the form-level
`off` is inconsistent — Chrome respects the per-input attributes but
Firefox sometimes does not, and 1Password/Bitwarden detection logic
varies. Best practice: remove `autoComplete="off"` from the Form;
keep the per-input values. (Removing `autoComplete="off"` is the
specifically-documented WCAG / accessibility recommendation for login
forms.)

### B-10 MED — Onboarding mid-flow refresh: `manualStep` lost, base-step recomputed
`src/modules/onboarding/OnboardingPage.tsx:62-77`

`manualStep` is a `useState<number | null>(null)` that the user-clicked
"Next" advances (line 93). On a page refresh:
- `manualStep` resets to `null` (state loss across reload — expected).
- `baseStep = getInitialStepIndex(guide)` (lines 62-65) jumps to the
  first step in `guide.steps` not in
  `user.completed_onboarding_step_ids`.

This is mostly fine **iff** every step calls
`completeStep(guide.id, step.id)` before advancing. It does:
`handleGlobalNext` calls `completeStep` at line 85 before incrementing.
But the call **only fires after `beforeNextRef.current?.()`** succeeds
(line 84). Steps that have side effects (e.g.
`FinishStep.tsx:18-21` does `installSelectedMcpServers`) might leave the
user in a confused half-installed state on refresh: the MCP servers got
installed (or partially installed) but the step never completed, so on
refresh they go back to the previous step.

Also: `currentStepIndex` lazily falls back to `manualStep ?? baseStep`,
but `baseStep` is `useMemo`'d only on `[user, guide, getInitialStepIndex]`
— and `getInitialStepIndex` is a `useCallback` with `[user]` deps. So a
fresh `completeStep` that updates `user.completed_onboarding_step_ids`
via the `onboarding.user_updated` event will re-render with a new
`user`, recompute `baseStep` to the next step, and the next-step click at
line 93 sets `manualStep = currentStepIndex + 1` — but that might be a
double-advance (baseStep already advanced too).

Mitigation: most of the time the explicit `setManualStep(currentStepIndex + 1)`
"wins" over `baseStep` because non-null beats null. So this is more
fragile than broken. But on refresh mid-step, partial side effects can
strand the user.

Fix: gate side effects on "step has been completed server-side" rather
than running them in `beforeNext`. Or move `completeStep` to before
`beforeNext`, accept the cost of marking-complete-without-effects.

### B-11 LOW — `RegisterForm` allows registration even when `userRegistrationEnabled` is false
`src/modules/auth/RegisterForm.tsx:1-149`,
`src/modules/auth/AuthPage.tsx:13-48`

`AuthPage` switches between Login and Register modes via a
`handleSwitchToRegister` button on `LoginForm`. Neither file reads
`Stores.Users.userRegistrationEnabled` (which is fake anyway per B-4) —
so the "Sign Up" link in the login form always works. If/when B-4 gets
a real backend, the toggle has to also gate the Register form
visibility.

### B-12 LOW — Onboarding `WelcomeStep` shows `display_name` but never falls back
`src/modules/onboarding/guides/getting-started/components/WelcomeStep.tsx:22`

```tsx
Welcome{user?.display_name ? `, ${user.display_name}` : ''}!
```

If the user has no `display_name` (common — `Create` form has it
optional), the greeting is just "Welcome!". Falling back to `username`
would be more personal.

### B-13 LOW — `EditUserGroupDrawer` `permissions` count is misleading on the list card
`src/modules/user/components/group/GroupListItem.tsx:130-134`

```tsx
<Descriptions.Item label="Permissions">
  <Text code>
    {Object.keys(group.permissions || {}).length} permissions
  </Text>
</Descriptions.Item>
```

`group.permissions` is `string[]` (see `types/Group`), so
`Object.keys(string[])` returns `["0","1",…]` — gives the length. It
works by accident. Should be `(group.permissions || []).length`. If a
future schema change makes `permissions` an object (e.g. for action
mapping), this silently breaks.

---

## Inconsistencies (I-N)

### I-1 — Create vs Edit drawer field set divergence
- `CreateUserDrawer.tsx:80-118`: collects username, email, password,
  **display_name**, permissions.
- `EditUserDrawer.tsx:100-129`: edits username, email, **is_active**,
  permissions. **No display_name**, **no password** (intentional —
  separate `ResetPasswordDrawer`), **no `is_active` on create**.

Inconsistencies:
- `display_name` available on Create, not on Edit (see B-1).
- `is_active` on Edit but not Create — admins create users as active
  (server-side default).
- The two drawers also disagree on `loading={creating}` vs `loading=`
  unset: `CreateUserDrawer.tsx:121-130` passes `loading={creatingUser}`
  to the Submit button AND `disabled={creatingUser}` to Cancel; the
  Edit drawer (lines 132-143) passes neither — the Update User button
  doesn't show a spinner, and Cancel is enabled mid-save.

### I-2 — Two unrelated permission validators duplicated 3×
`CreateUserDrawer.tsx:10-35`, `EditUserDrawer.tsx:11-36`,
`UserGroupsSettings.tsx:26-51`, `EditUserGroupDrawer.tsx:11-36`

The exact same `validatePermissions` function is copy-pasted four times.
Should live once in `user/utils/validatePermissions.ts` or even in
`core/permissions/` next to the new helper (it's the same shape — turn
a JSON string into a `string[]` and check each value is a known
permission). Same code, four maintenance points; if the permission
catalog grows or the JSON schema changes (e.g. supports `allOf`/`anyOf`
expressions per the plan), all four have to be updated in lockstep.

### I-3 — `Permissions` enum constant in raw textarea
The four drawers ask the admin to paste a JSON array of permission
strings into a textarea (`["users::read", "users::edit"]`). The
backend has 80+ permission strings; expecting an admin to know them by
heart and avoid typos is a UX trap. A multi-select with grouping by
module (the catalog returned via `/api/permissions` or built from the
TS `Permissions` enum) would prevent typos and make the form
discoverable.

### I-4 — Drawer footer button order disagrees
Some drawers: Submit | Cancel (`CreateUserDrawer.tsx:120-133`,
`EditUserDrawer.tsx:131-143`, `AssignGroupDrawer.tsx:62-74`,
`ResetPasswordDrawer.tsx:69-82`).

Others: Cancel | Submit (`EditUserGroupDrawer.tsx:146-152` —
`<Button onClick={handleClose}>` first, `<Button type="primary">`
second).

Pick one. Ant Design's `Modal` default is `[Cancel, OK]`; that's the
platform convention. Five drawers are wrong if we go by Ant convention,
one is right.

### I-5 — Some drawers use `size={600}`, the create-group `Drawer` uses `width={600}`
`UserGroupsSettings.tsx:184`: `width={600}` on the inline Create Group
Drawer.
`EditUserGroupDrawer.tsx:106`: `size={600}` on the EditUserGroupDrawer.
The codebase's `Drawer` wrapper at
`@/modules/layouts/app-layout/components/Drawer` accepts both, but the
two props mean different things on different breakpoints. Audit Agent 2
would weigh in here; from this scope, the inconsistency is a smell.

### I-6 — `EditUserGroupDrawer` and `CreateUserDrawer` are not symmetric on `name="…"`
Most drawers add `name="create-user"` (CreateUserDrawer.tsx:78),
`name="edit-user-form"` (EditUserDrawer.tsx:95),
`name="edit-user-group-form"` (EditUserGroupDrawer.tsx:109). The
ResetPassword, AssignGroup, UserGroupsDrawer, GroupMembersDrawer, and
the inline Create-Group form have NO `name` — `<Form form={form}>`.
Names matter for ARIA auto-labeling and for `<input name>` autoFill
hints. Pick consistent.

### I-7 — `module.tsx` `dependencies` list inconsistent
- `auth/module.tsx:15`: `dependencies: ['router']`
- `user/module.tsx:34`: `dependencies: ['router']`
- `user-profile/module.tsx`: no `dependencies` field (line 4 has the
  whole config)
- `onboarding/module.tsx:20`: `dependencies: ['router']`
- `guides/getting-started/module.tsx:13`: `dependencies: ['onboarding']`

`user-profile` slots into `sidebarFooter`, which only exists because the
layout module registered it — so `user-profile` arguably depends on
`layouts` or similar. None of these matter at runtime today (the module
system seems to tolerate missing deps), but the inconsistency makes the
plan's reasoning ("modules declare what they need") weaker.

---

## Inefficiencies (E-N)

### E-1 — `UserGroupsDrawer` double-loads + ignores `useEffect` deps stability
`src/modules/user/components/user/UserGroupsDrawer.tsx:15-52`

See B-2 for the N² fetch. Beyond the bug: the `useEffect` deps are
`[isOpen, user, groups]`. `groups` is a reference from the proxy store
and gets a new array reference on every event the UserGroups store
processes — meaning every group-membership change anywhere in the app
re-runs the entire N² fetch. Open the drawer, change a permission
elsewhere, watch the drawer refetch all member lists. Should depend on
`[isOpen, user?.id]` and read `groups` inside the effect via a getState
call.

### E-2 — `UsersSettings.tsx` re-pages from the store, ignoring local cache
`src/modules/user/components/user/UsersSettings.tsx:154-159`

Every page-change calls `Stores.Users.loadUsers(newPage, newPageSize)`
unconditionally, even if the user is just clicking the same page or
toggling the size selector to the same value. `loadUsers` itself
(`Users.store.ts:72-106`) doesn't memoize by `(page, pageSize)` — it
re-fetches every time. With 50 users per page, this is fine; with 5000,
this is wasteful.

### E-3 — No `visibilitychange` listener; permissions can silently go stale
`src/modules/auth/AuthGuard.tsx`, `Auth.store.ts:185-224`

The plan calls this out (line 619-625). Concrete attack vector: admin
demotes Alice's group → Alice's open tab still uses cached
`permissions[]` and successfully renders admin buttons until she
clicks one and the backend 403s. Adding a
`document.visibilitychange` listener that re-calls `initAuth` (or a
lighter `refreshMe`) on tab focus is a 5-line fix. The store already
has `subscribeWithSelector` so the per-store reactivity is fine.

Suggested implementation: in `AuthGuard`'s `useEffect` add

```ts
const onVisible = () => {
  if (document.visibilityState === 'visible' && isAuthenticated) {
    Stores.Auth.initAuth()
  }
}
document.addEventListener('visibilitychange', onVisible)
return () => document.removeEventListener('visibilitychange', onVisible)
```

Pair with a 30-second debounce so rapid alt-tabs don't hammer `/me`.

### E-4 — Onboarding step components fetch in `useEffect(..., [])`, lose data on remount
`src/modules/onboarding/guides/getting-started/components/ApiKeysStep.tsx:36-45`,
`McpServersStep.tsx:21-25`

Each step component fetches via `Stores.ApiKeysStep.loadProviders()` /
`Stores.McpServersStep.loadMcpServers()` on first mount. Click "Back",
then "Next" — the components remount, but the data fetch only happens
because `__init__.providers` for `ApiKeysStep` was registered (which is
a once-per-store-lifetime hook). For `McpServersStep`, the
`loadMcpServers` call happens in `useEffect` (line 24); going Back and
Next re-runs it. The stores cache the result internally
(`loadingProviders`, `loadingServers` flags) so the second call is a
no-op, but it's a sign that the lifecycle isn't quite right — and the
`reset()` action in `ApiKeysStep.store.ts:78-85` explicitly does
**NOT** reset `providers`/`userKeys` because `__init__.providers`
won't fire again. That hack ought to be a comment in the store
declaration, not a 4-line block comment inside `reset()`.

### E-5 — `OnboardingPage` `useMemo` recomputes guides on every render
`src/modules/onboarding/OnboardingPage.tsx:47-50`

```ts
const guides = useMemo(
  () => ((slots.get('onboarding') as OnboardingSlot[]) || []).sort(…),
  [slots],
)
```

`slots` is a Zustand state-shaped Map — getting a new reference on every
state event. The `useMemo` is invalidated on every re-render. Tiny win,
but the pattern repeats across the codebase.

### E-6 — `AuthGuard` runs `initAuth` even on every login → home navigation
`src/modules/auth/AuthGuard.tsx:21-24`

`useEffect(() => { Stores.Auth.initAuth() }, [])` — fine for one mount.
But the `AuthGuard` mounts/unmounts as the user navigates between
non-auth and auth routes, and each fresh mount re-fires `initAuth`. The
store does guard with `if (state.isLoading) return` (line 187-188), but
the network call still happens on every fresh mount where the previous
mount unmounted before completing. Easy fix: a `hasInitialized` flag on
the store, or simpler — move the call out of `AuthGuard` entirely into
the `App.store` `__init__` block (already runs on app boot).

---

## Responsive / sizing / scrolling (R-N)

### R-1 — `OnboardingPage` left pane has fixed `w-64`, no narrow breakpoint
`src/modules/onboarding/OnboardingPage.tsx:124`

`<div className="w-64 flex-shrink-0 …">` — 256px hard-coded for the guide
list pane. On a <500px viewport the layout becomes unusable (left pane
eats half the screen, right pane gets unreadable). No `md:hidden` /
`sm:w-full` / accordion fallback. Compare with the SettingsPage which
uses a Dropdown on `xs`.

### R-2 — `UsersSettings` row toolbar overflows on narrow viewports
`src/modules/user/components/user/UsersSettings.tsx:191-211`

```tsx
<div className="flex items-start gap-3 flex-wrap">
  <div className="flex-1">
    <div className="flex items-center gap-2 mb-2 flex-wrap">
      <div className={'flex-1 min-w-48'}>
        … username …
      </div>
      <div className={'flex gap-1 items-center justify-end'}>
        {getUserActions(user)}    // 5 buttons + Switch
      </div>
    </div>
  </div>
</div>
```

The right-hand action bar has 5 controls (Switch, Edit, Reset Password,
Groups, Delete). On a 400-500px viewport the actions wrap below the
username row, which is fine, but `gap-1` is tight and the buttons clip.
Should collapse to an overflow menu (Ant `Dropdown`) below `sm`.

### R-3 — `EditUserGroupDrawer` `<TextArea rows={6}>` makes the perm-editor unreadable
`src/modules/user/components/group/EditUserGroupDrawer.tsx:139`,
`UserGroupsSettings.tsx:207`,
`CreateUserDrawer.tsx:117`,
`EditUserDrawer.tsx:128`

Six rows of a JSON-array editor isn't enough for a group with 20
permissions. The textarea isn't `autoSize` so it has its own vertical
scroll and the drawer also scrolls — nested scrollbars when reviewing
permissions. Use `autoSize={{ minRows: 6, maxRows: 20 }}`.

### R-4 — Onboarding step content `overflow-y-auto` in a `Suspense` parent
`src/modules/onboarding/OnboardingPage.tsx:194-210`

```tsx
<div className="flex-1 overflow-y-auto p-6">
  …
  {StepComponent && (
    <Suspense fallback={…}>
      <StepComponent {...stepProps} />
    </Suspense>
  )}
</div>
```

ApiKeysStep's two-column layout sets `flex flex-1 mb-4` with an inner
`Menu` and a content pane; the parent `<div>` has `flex flex-1 mb-4`
which under the `overflow-y-auto` doesn't compute a useful flex basis.
On <600px viewport, the two-column collapse rule is missing — the menu
just shrinks to whatever it can and the right pane fits in 200px.
Mobile-collapse to a top-tab pattern would help.

### R-5 — `UserProfileWidget` truncation in collapsed sidebar
`src/modules/user-profile/UserProfileWidget.tsx:55-102`

The widget uses `isSidebarCollapsed` to switch to a tooltip — good. But
the `Dropdown` `placement="topLeft"` (line 80) collapses oddly when the
sidebar is on the right side (RTL or theme variant). Hard to verify
without running the app; flag for QA.

---

## Appendix: Plan compliance status

**Files that show plan applied:**
- None. The plan introduces `src/core/permissions/` and adds
  `permission?: PermissionExpr` to slot types; neither has happened.

**Files where plan not yet applied (in scope):**
- `src/modules/auth/Auth.store.ts` — `permissions[]` is populated in
  `initAuth` and persisted, but never consumed by any UI in scope.
  Plan calls for an optional event emission on permission changes; not
  done.
- `src/modules/auth/AuthGuard.tsx` — no permission gate on routes,
  no `visibilitychange` listener for stale-permission refresh.
- `src/modules/user/module.tsx:88-103` — `settingsAdminPages` entries
  for Users and User Groups have no `permission` field; should carry
  `permission: 'users::read'` and `permission: 'groups::read'`.
- `src/modules/user/components/user/UsersSettings.tsx:99-148` — every
  action button is ungated; should wrap with `<Can>` per audit row
  (Edit → `users::edit`, Reset Password → `users::reset_password`,
  Groups → `groups::assign_users`, Delete → `users::delete`, the active
  Switch → `users::toggle_status`).
- `src/modules/user/components/user/UsersSettings.tsx:171-176` — the
  "Create User" `+` button (`Stores.CreateUserDrawer.openCreateUserDrawer`)
  is ungated; should wrap with `<Can permission="users::create">`.
- `src/modules/user/components/user/CreateUserDrawer.tsx:120-133` and
  `EditUserDrawer.tsx:131-143` — submit buttons render unconditionally;
  should `<Can permission="users::create">` / `<Can permission="users::edit">`
  the submit + derive `disabled={!canManage}` for the inputs.
- `src/modules/user/components/user/ResetPasswordDrawer.tsx:69-82` —
  same pattern, `users::reset_password`.
- `src/modules/user/components/user/AssignGroupDrawer.tsx:62-74` and
  `UserGroupsDrawer.tsx:121-145` — `groups::assign_users`.
- `src/modules/user/components/group/UserGroupsSettings.tsx:124-129,
  178-225` — `groups::create` for the create-group affordance + drawer.
- `src/modules/user/components/group/GroupListItem.tsx:49-93` — Members
  → `groups::read`, Edit → `groups::edit`, Delete → `groups::delete`.
- `src/modules/user/components/group/EditUserGroupDrawer.tsx:108-156` —
  `groups::edit` on submit + input disable.
- `src/modules/onboarding/module.tsx:33-41` — sidebar tools entry
  registered with no `permission`. Onboarding doesn't have a permission
  string in the backend (`server/src/modules/user/permissions.rs:11-26`
  only has `profile::read`/`profile::edit`); the route is per-user, so
  no gating needed, but document this explicitly.
- `src/modules/user-profile/module.tsx:11-17` — `sidebarFooter` widget
  registered with no permission. User profile widget is per-user, so no
  gating needed, but the slot type may need `permission?: PermissionExpr`
  in the foundation.

**Files where the existing gating helper needs migrating beyond the plan:**
- `src/modules/code-sandbox/components/SandboxEnvironmentsSection.tsx:17-23`
  — already known per plan. The fix is the foundation `hasPermission`
  + `is_admin` short-circuit + correct `::` separator.

**Files that would need updates beyond what the plan calls out:**
- `Auth.store.ts:65-163` — emit `auth.*` events for login/register/logout
  so the rest of the app can react (e.g. clear caches on logout, fetch
  new permissions on login). Not strictly a plan item but the
  foundation's "re-fetch on tab focus" mention is the same family of
  problem. Document explicitly as part of the foundation PR.
- `UserGroupsDrawer.tsx:15-52` — the N² membership fetch is independent
  of permissions but blocks the "Groups" action gate from being useful
  (clicking "Groups" today is already slow; gating the button doesn't
  help). Suggest pairing the gate rollout with a real
  `GET /api/users/{id}/groups` endpoint.
- `EditUserDrawer.tsx:46-69` — add `display_name` field (B-1) before
  enabling the disabled-via-permission pattern; otherwise the read-only
  user view drops a field that's only visible in Create.
- `UsersSettings.tsx:99-148` — add self-lockout and root-admin lockout
  guards (B-6) in addition to the permission gates, since the backend
  cannot derive "self" from a permission check.
- `UserRegistrationSettings.tsx` — kill or wire to a real backend (B-4)
  before adding a permission gate; gating a fake setting is worse than
  having no setting.
