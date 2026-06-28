# Permission-gating E2E tests

Permission-specific regression coverage for the frontend gating
applied across the user/llm-provider/llm-repository/mcp/assistants/
hardware/llm-local-runtime/code-sandbox modules.

See `.claude/PERMISSION_GATING.md` for the gating pattern.

## Anatomy

- **`no-403.ts`** — a Playwright fixture that fails the test if any
  `/api/*` response returns 403. The highest-value regression catcher:
  every test that runs under this fixture as a non-admin user will
  surface missing UI gates as 403s in the network log. Run the
  whole E2E suite under this fixture as a non-admin user and any
  missing gate falls out as a test failure.

- **`fixtures.ts`** — helpers to create permission-scoped test users
  on the fly (admin creates them via API), then log in as them. The
  fixtures used (the exported helpers in `fixtures.ts`):
  - `root` — `is_admin: true` (the seeded admin). Verifies bypass.
  - `member` — default `users` group only (no admin perms).
  - `readonly_users` — only `users::read` + `groups::read`. Verifies
    the read-vs-manage form-disable path.
  - `hub_mcp_only` — only `hub::mcp_servers::read`. Verifies Hub's
    partial-tab visibility.
  - `auth-providers-reader` (`loginAsAuthProvidersReader`) — only
    `auth_providers::read`. Verifies the auth-providers list renders
    without Add/Edit/Switch/Delete/Test controls.
  - `auth-providers-manager` (`loginAsAuthProvidersManager`) —
    `auth_providers::read` + `auth_providers::manage`. Verifies all
    auth-provider admin surfaces are visible.
  - `loginWithPerms(...)` — generic helper to create + log in as a
    user with an arbitrary explicit permission set (for one-off tests
    the named fixtures above don't cover).

- **Per-module specs** — one file per module from the rollout
  checklist. Asserts what a permission-restricted user should NOT
  see (sidebar entries, action buttons, form submit buttons). Also
  asserts deep-link 403s render in place.

## Assertion patterns

```ts
// Sidebar / menu hiding
await expect(page.getByRole('link', { name: 'Users' })).toHaveCount(0)

// Deep-link 403
await page.goto('/settings/users')
await expect(page.getByText('403')).toBeVisible()

// Button absence
await expect(page.getByRole('button', { name: /create user/i })).toHaveCount(0)

// Form disabled + save hidden (read-only)
await expect(page.getByLabel('Username')).toBeDisabled()
await expect(page.getByRole('button', { name: /update user/i })).toHaveCount(0)
```

## Adding a new module's spec

1. Use `loginAsMember()` (or a more specific fixture) from
   `fixtures.ts` at the top of the spec.
2. For each gated surface in the module, write a test that:
   - Navigates to the page (or asserts the sidebar entry is absent).
   - Asserts the gated control is absent (`toHaveCount(0)`).
3. Wrap the spec under the `no-403` fixture so the suite also flags
   any accidental backend 403 fires.

## Running

```bash
# All permission specs
cd src-app/ui && npx playwright test tests/e2e/permissions/

# Single module
npx playwright test tests/e2e/permissions/users.spec.ts
```

The per-test infrastructure (backend + vite + postgres DB) is
handled by `tests/fixtures/test-context.ts` exactly like every other
E2E test — nothing special needed at the runner level.
