import { test, expect } from '../../fixtures/test-context'
import {
  loginAsAdmin,
  login,
  createTestUser,
  getAdminToken,
} from '../../common/auth-helpers'
import {
  openCreateAssistantDrawer,
  fillAssistantForm,
  submitAssistantForm,
  getTemplateAssistantRow,
} from '../06-assistants/helpers/assistant-helpers'

// Realtime cross-device sync for two more entities:
//
//   * `assistant_template` — Everyone audience. The mutation surface (the
//     template list) lives ONLY on the admin settings page, so the two-device
//     test runs admin↔admin: a template created on one admin device appears on
//     another admin device live. There is NO regular-user / cross-user template
//     surface, so there is no isolation case to assert (Everyone is the point).
//
//   * `api_key` — Owner-scoped. A user's saved provider key reaches the SAME
//     user's other device live (the masked-key tag flips), and a DIFFERENT user
//     never sees it.
//
// Run with --workers=1 (shared backend + DB).
//
// NAV NOTE: every authenticated page holds an open `GET /api/sync/subscribe`
// SSE stream, so `waitForLoadState('networkidle')` never settles and HANGS the
// test. Both helpers below navigate inline and wait for a stable selector
// instead. (This is also why we do NOT call `goToTemplateAssistantsSettings` —
// it still does a `networkidle` wait that would hang under the sync stream.)

/**
 * Land on the admin Template Assistants settings page WITHOUT a networkidle
 * wait. Waits for the "Template Assistants" card title — the same stable
 * "page rendered" signal the 06-assistants helper uses, minus the hang.
 */
async function gotoTemplateAssistantsSettings(
  page: import('@playwright/test').Page,
  baseURL: string,
) {
  // Template assistants live at /settings/assistant-templates (the admin page);
  // /settings/assistants is the per-user "My Assistants" page. Match the
  // working 06-assistants helper.
  await page.goto(`${baseURL}/settings/assistant-templates`)
  await page
    .getByRole('heading', { name: 'Assistant Templates', level: 4 })
    .waitFor({ timeout: 15000 })
  await page
    .locator('.ant-card-head-title:has-text("Template Assistants")')
    .waitFor({ timeout: 15000 })
}

/**
 * Land on the user LLM-providers settings page WITHOUT a networkidle wait,
 * then wait for the provider's detail panel (its h4 heading) to render so the
 * key tag / Save button are present. The page auto-selects the first provider,
 * so the named provider's panel appears on its own.
 */
async function gotoUserLlmProvidersAndSelect(
  page: import('@playwright/test').Page,
  baseURL: string,
  providerName: string,
) {
  await page.goto(`${baseURL}/settings/user-llm-providers`)
  // Stable "page rendered" signal: the provider sits in a role=menuitem
  // (desktop Menu or mobile Dropdown both expose this role).
  await page
    .getByRole('menuitem', { name: providerName })
    .first()
    .waitFor({ timeout: 15000 })
  await page.getByRole('menuitem', { name: providerName }).first().click()
  // The detail panel for the selected provider renders its name as an h4.
  await expect(
    page.getByRole('heading', { level: 4, name: providerName }),
  ).toBeVisible({ timeout: 15000 })
}

// ── REST fixtures (driven via baseURL, which proxies /api to this test's
// backend) ──────────────────────────────────────────────────────────────────

/**
 * Create a `custom` provider with no system key (the only enabled-with-no-key
 * combination the backend accepts). This surfaces the orange "No admin key"
 * tag and the "Save Key" button — exactly the starting state the api_key test
 * mutates.
 */
async function createProviderViaApi(
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
      `createProviderViaApi(${name}) failed: ${res.status} ${await res.text()}`,
    )
  }
  const data = await res.json()
  return { id: data.id, name: data.name }
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

// ─────────────────────────────────────────────────────────────────────────────

test.describe('Realtime sync — assistant template (Everyone audience)', () => {
  test('a template assistant created on admin device A appears on admin device B without reload', async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // Device A — admin. loginAsAdmin onboards the admin on this test's fresh
    // backend. Load A fully before B.
    await loginAsAdmin(page, baseURL)
    await gotoTemplateAssistantsSettings(page, baseURL)

    // Device B — a second context for the SAME admin (templates are admin-only,
    // so both sync peers are admins; Everyone audience means every authenticated
    // connection learns of the change).
    const ctxB = await browser.newContext()
    const pageB = await ctxB.newPage()
    try {
      await loginAsAdmin(pageB, baseURL)
      await gotoTemplateAssistantsSettings(pageB, baseURL)

      const name = `Sync Template ${Date.now()}`

      // Create on device A (template drawer flow).
      await openCreateAssistantDrawer(page, false)
      await fillAssistantForm(page, { name, enabled: true })
      await submitAssistantForm(page)

      // Device B must show the new template row WITHOUT a manual reload — the
      // SSE `sync:assistant_template` event makes the TemplateAssistants store
      // refetch. `getTemplateAssistantRow` is async → await before expect.
      await expect(await getTemplateAssistantRow(pageB, name)).toBeVisible({
        timeout: 15_000,
      })
    } finally {
      await ctxB.close()
    }

    // No isolation case: assistant_template is Everyone audience, and the only
    // mutation/read surface is the admin-only settings page. There is no
    // regular-user template list to assert absence against.
  })
})

test.describe('Realtime sync — api key (owner-scoped)', () => {
  test("a saved API key reaches the owner's other device but NOT a different user", async ({
    page,
    browser,
    testInfra,
  }) => {
    const { baseURL } = testInfra

    // User A = admin, device 1. loginAsAdmin onboards the admin FIRST so
    // getAdminToken below can authenticate.
    await loginAsAdmin(page, baseURL)

    const adminToken = await getAdminToken(baseURL)
    const uniq = Date.now()

    // Provider both users can see (assigned to the default Users group).
    const providerName = `sync-apikey-${uniq}`
    const provider = await createProviderViaApi(baseURL, adminToken, providerName)
    await assignProviderToDefaultGroup(baseURL, adminToken, provider.id)

    // A second, distinct user (auto-joins the default Users group, so it can see
    // the provider + gets a live sync stream).
    const username = `apikey_other_${uniq}`
    const password = 'Password123!'
    await createTestUser(
      baseURL,
      adminToken,
      username,
      `${username}@example.com`,
      password,
      ['profile::read', 'profile::edit', 'user_llm_providers::read'],
    )

    // Device 1 (owner) lands on the providers page now that the provider exists.
    await gotoUserLlmProvidersAndSelect(page, baseURL, providerName)

    const ctxA2 = await browser.newContext() // owner, device 2 — positive control
    const pageA2 = await ctxA2.newPage()
    const ctxB = await browser.newContext() // different user — isolation
    const pageB = await ctxB.newPage()
    try {
      // Load A2 fully before B.
      await loginAsAdmin(pageA2, baseURL)
      await gotoUserLlmProvidersAndSelect(pageA2, baseURL, providerName)

      await login(pageB, baseURL, username, password)
      await gotoUserLlmProvidersAndSelect(pageB, baseURL, providerName)

      // Baseline: every device starts with no personal key → orange tag.
      await expect(page.getByText('No admin key')).toBeVisible()
      await expect(pageA2.getByText('No admin key')).toBeVisible()
      await expect(pageB.getByText('No admin key')).toBeVisible()

      // Owner saves a personal key on device 1.
      await page
        .getByPlaceholder('Enter your API key (e.g. sk-...)')
        .fill('sk-owner-personal-key')
      const saveBtn = page.getByRole('button', { name: 'Save Key' })
      await expect(saveBtn).toBeEnabled()
      await saveBtn.click()

      // Device 1's own panel flips to the green "Your key configured" state.
      await expect(page.getByText('Your key configured')).toBeVisible()

      // Positive control: the owner's OTHER device reflects the masked-key
      // change live — the SSE `sync:api_key` event makes UserLlmProviders
      // refetch, so the tag flips WITHOUT a reload. Proves the event fired +
      // was delivered, making B's absence below meaningful.
      await expect(pageA2.getByText('Your key configured')).toBeVisible({
        timeout: 15_000,
      })
      await expect(
        pageA2.getByRole('button', { name: 'Update Key' }),
      ).toBeVisible()

      // Isolation: a DIFFERENT user had the same delivery window (A2 already
      // received it) yet never sees the owner's key — their panel stays orange.
      await expect(pageB.getByText('Your key configured')).not.toBeVisible()
      await expect(pageB.getByText('No admin key')).toBeVisible()
    } finally {
      await ctxA2.close()
      await ctxB.close()
    }
  })
})
