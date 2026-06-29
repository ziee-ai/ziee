import {
  createTestUser,
  getAdminToken,
  login,
  loginAsAdmin,
} from '../../common/auth-helpers'
import { byTestId } from '../testid'
import { expect, test } from '../../fixtures/test-context'

// Realtime sync for the LLM provider / model area. These three entities are
// admin-permission-scoped or group-scoped (NOT owner-scoped like assistant),
// so the audience is "everyone who can read the entity" rather than a single
// user:
//
//   - `llm_provider`      → holders of `llm_providers::read`  (admins)
//   - `llm_model`         → holders of `llm_models::read`     (admins)
//   - `user_llm_provider` → holders of `user_llm_providers::read` (every user)
//
// The backend publishes BOTH the admin entity and the group-scoped
// `user_llm_provider` on the same mutation (see llm_provider/handlers/admin.rs
// + llm_model/handlers/models.rs), so admins and regular users each refresh
// their own surface. We prove:
//   1. a provider/model change on device A reaches the admin's OTHER device live;
//   2. enabling a model reaches a DIFFERENT, regular user's chat model picker.
//
// Run with --workers=1 (shared backend + DB).
//
// CRITICAL: `waitForLoadState('networkidle')` HANGS on any page where the SSE
// sync stream is connected (the stream is never "idle"). This spec deliberately
// navigates inline and waits on stable selectors, keeping it self-contained on
// the live app shell rather than coupling to the llm nav helpers.

/** Navigate to the admin LLM providers list WITHOUT a networkidle wait. */
async function gotoProvidersList(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  await page.goto(`${baseURL}/settings/llm-providers`)
  await page.waitForLoadState('load')
  // The "Add Provider" menu item is the page's stable landmark.
  await expect(byTestId(page, 'llm-provider-nav-add-provider')).toBeVisible({
    timeout: 15_000,
  })
}

/** A provider's sidebar menu entry (how providers render in the list). */
function providerMenuItem(page: import('@playwright/test').Page, name: string) {
  return page.getByTestId(/^llm-provider-nav-/).filter({ hasText: name })
}

/**
 * Create a `custom` provider via the admin API. `custom` + no api_key is the
 * only enabled-with-no-key combination the backend accepts, and a custom
 * provider makes NO outbound network call (mirrors llm/user-llm-providers).
 */
async function createCustomProvider(
  baseURL: string,
  adminToken: string,
  name: string,
): Promise<{ id: string; name: string }> {
  const res = await fetch(`${baseURL}/api/llm-providers`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${adminToken}`,
    },
    body: JSON.stringify({ name, provider_type: 'custom', enabled: true }),
  })
  if (!res.ok) {
    throw new Error(
      `createCustomProvider(${name}) failed: ${res.status} ${await res.text()}`,
    )
  }
  const data = await res.json()
  return { id: data.id, name: data.name }
}

/**
 * Create an ENABLED remote model on a provider via the admin API. No file /
 * download needed: `POST /api/llm-models` takes JSON only (mirrors the
 * "Add Remote Model" UI path). An enabled model is what the user-facing
 * ModelSelector renders.
 */
async function createEnabledModel(
  baseURL: string,
  adminToken: string,
  providerId: string,
  name: string,
  displayName: string,
): Promise<{ id: string }> {
  const res = await fetch(`${baseURL}/api/llm-models`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      Authorization: `Bearer ${adminToken}`,
    },
    body: JSON.stringify({
      provider_id: providerId,
      name,
      display_name: displayName,
      enabled: true,
      engine_type: 'none',
      file_format: 'safetensors',
      capabilities: { chat: true },
    }),
  })
  if (!res.ok) {
    throw new Error(
      `createEnabledModel(${name}) failed: ${res.status} ${await res.text()}`,
    )
  }
  return { id: (await res.json()).id }
}

/** Assign a provider to the default Users group so every user can see it. */
async function assignProviderToDefaultGroup(
  baseURL: string,
  adminToken: string,
  providerId: string,
): Promise<void> {
  const groupsResp = await fetch(`${baseURL}/api/groups`, {
    headers: { Authorization: `Bearer ${adminToken}` },
  })
  const groupsBody = await groupsResp.json()
  const defaultGroup = groupsBody.groups.find(
    (g: { is_default?: boolean }) => g.is_default,
  )
  if (!defaultGroup) throw new Error('No default group found')

  const assignResp = await fetch(
    `${baseURL}/api/llm-providers/${providerId}/groups`,
    {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        Authorization: `Bearer ${adminToken}`,
      },
      body: JSON.stringify({ group_id: defaultGroup.id }),
    },
  )
  if (!assignResp.ok) {
    throw new Error(
      `assignProviderToDefaultGroup failed: ${assignResp.status} ${await assignResp.text()}`,
    )
  }
}

test.describe('Realtime sync — LLM provider / model (admin + cross-role)', () => {
  // -------------------------------------------------------------------------
  // ENTITY: llm_provider  (audience: llm_providers::read → admins)
  // An admin creates a CUSTOM (no-network) provider on device A; the same
  // admin's device B (on the providers list) sees it without a reload.
  // -------------------------------------------------------------------------
  test('a custom provider created on device A appears on the admin device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // FRESH backend → create the admin first so the SSE stream + token exist.
    await loginAsAdmin(page, baseURL)
    await gotoProvidersList(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      // Load device B fully (its sync stream must be connected) BEFORE the
      // mutation, so the create lands inside its live delivery window.
      await loginAsAdmin(pageB, baseURL)
      await gotoProvidersList(pageB, baseURL)

      // Device A creates the provider through the real UI drawer (inline, to
      // keep this cross-device spec self-contained rather than reusing the
      // llm create helpers).
      const name = `Sync Provider ${Date.now()}`
      await byTestId(page, 'llm-provider-nav-add-provider').click()
      await byTestId(page, 'llm-provider-form').waitFor({ state: 'visible', timeout: 15_000 })

      // Provider Type select → "Custom" is the last option (index 8). The
      // combobox is readonly, so navigate with the keyboard (mirrors
      // llm/helpers/provider-helpers `selectProviderType`).
      await byTestId(page, 'llm-provider-type-select').click()
      await byTestId(page, 'llm-provider-type-select-opt-custom').click()

      await byTestId(page, 'llm-provider-name-input').fill(name)

      // Submit via the drawer's primary submit button (verb-only "Add" label;
      // scope by class so the "Add Provider" menu item can't collide).
      await byTestId(page, 'llm-provider-submit-btn').click()
      // Success closes the drawer.
      await expect(byTestId(page, 'llm-provider-form')).toHaveCount(0, {
        timeout: 15_000,
      })

      // Device B must show the new provider in its sidebar list WITHOUT a
      // reload — the `sync:llm_provider` event refetches the admin provider
      // list. Playwright auto-waits.
      await expect(providerMenuItem(pageB, name)).toBeVisible({
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }
  })

  // -------------------------------------------------------------------------
  // ENTITY: llm_model  (audience: llm_models::read → admins)
  // An admin adds a model to a provider on device A (via the API — the
  // "Add Remote Model" path is JSON-only, no multi-GB download/upload); the
  // admin's device B, viewing that provider's detail page, sees the model
  // appear live.
  // -------------------------------------------------------------------------
  test('a model added to a provider on device A appears on the admin device B detail page without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(baseURL)

    // Seed a provider to host the model.
    const provider = await createCustomProvider(
      baseURL,
      adminToken,
      `Model Host ${Date.now()}`,
    )

    // Device A on the providers list (it originated nothing yet; the model
    // create is the device-A action below).
    await gotoProvidersList(page, baseURL)

    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      // Device B opens the provider DETAIL page (where models render via the
      // "Models" card). `LlmProvider.store` reloads providers-with-models on
      // `sync:llm_model`, so this page re-renders live. Navigate inline; the
      // "Models" card is the detail page's stable landmark.
      await loginAsAdmin(pageB, baseURL)
      await pageB.goto(`${baseURL}/settings/llm-providers/${provider.id}`)
      await pageB.waitForLoadState('load')
      await expect(byTestId(pageB, 'llm-models-section-card')).toBeVisible({
        timeout: 15_000,
      })
      // Confirm the empty state first so the later assertion is meaningful.
      await expect(byTestId(pageB, 'llm-models-empty')).toBeVisible({
        timeout: 10_000,
      })

      // Device A adds the model (admin action). Emits `sync:llm_model`.
      const displayName = `Sync Model ${Date.now()}`
      await createEnabledModel(
        baseURL,
        adminToken,
        provider.id,
        `sync-model-${Date.now()}`,
        displayName,
      )

      // Device B's detail page shows the model's display name WITHOUT a
      // reload — the model name renders as plain text in the Models card.
      await expect(byTestId(pageB, 'llm-models-section-card')).toContainText(displayName, {
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }
  })

  // -------------------------------------------------------------------------
  // ENTITY: user_llm_provider  (CROSS-ROLE; audience: user_llm_providers::read
  // → every user). An admin enables a model on a group-assigned provider on
  // device A; a DIFFERENT, regular user's chat ModelSelector updates live.
  // This is "admin creates an enabled model → the user sees it".
  // -------------------------------------------------------------------------
  test("an enabled model created by the admin appears in a regular user's chat model picker without reload", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Admin first (fresh backend), then the regular user.
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(baseURL)

    const uniq = Date.now()
    const username = `model_picker_user_${uniq}`
    const password = 'Password123!'
    await createTestUser(
      baseURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      ['profile::read', 'user_llm_providers::read'],
    )

    // Seed a provider visible to every user: custom (no network) + assigned to
    // the default Users group. Create it WITHOUT models so the picker starts
    // empty for this provider — the device-A action below adds the first one.
    const provider = await createCustomProvider(
      baseURL,
      adminToken,
      `Shared Provider ${uniq}`,
    )
    await assignProviderToDefaultGroup(baseURL, adminToken, provider.id)

    // Device A = admin (on the providers list; will add the model below).
    await gotoProvidersList(page, baseURL)

    const ctxUser = await browser.newContext() // regular user — the receiver
    const pageUser = await ctxUser.newPage()
    try {
      // Regular user opens the chat page, where the ModelSelector
      // (toolbar_model slot) lives. Load it fully BEFORE the mutation so its
      // `sync:user_llm_provider` listener is live. Navigate inline; the
      // model-selector test id is the stable landmark.
      await login(pageUser, baseURL, username, password)
      await pageUser.goto(`${baseURL}/`)
      await pageUser.waitForLoadState('load')
      const selector = pageUser.getByTestId('model-selector')
      await expect(selector).toBeVisible({ timeout: 15_000 })

      // Admin (device A) creates an ENABLED model on the shared provider.
      // Backend emits `sync:user_llm_provider` to all user_llm_providers::read
      // holders → the user's ModelPicker store refetches its scoped view.
      const displayName = `Picker Model ${uniq}`
      await createEnabledModel(
        baseURL,
        adminToken,
        provider.id,
        `picker-model-${uniq}`,
        displayName,
      )

      // The user's ModelSelector must surface the new model WITHOUT a reload.
      // The provider started with zero models (so nothing was selected), and
      // the ModelPicker store auto-selects the first enabled model on refetch —
      // so once the `sync:user_llm_provider` event lands and the store reloads,
      // the model shows as the selected value in the picker. Assert that text:
      // it's robust to antd's internal Select DOM (v6 renders
      await expect(selector).toContainText(displayName, { timeout: 15_000 })
    } finally {
      await ctxUser.close()
    }
  })
})
