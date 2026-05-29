import { Page, expect } from '@playwright/test'

/**
 * Shared helpers for the Local Runtime E2E specs.
 *
 * NOTE: these specs were authored alongside the backend test suite but
 * have NOT been executed yet (per the team's implement-before-run rule).
 * Selectors follow the documented antd surface; expect a verification
 * pass (selector/timing tweaks) on first real run.
 */

export const RUNTIME_SETTINGS_PATH = '/settings/llm-runtime'

/** Navigate to Settings → Local Runtimes and wait for the page to load. */
export async function gotoRuntimeSettings(page: Page, baseURL: string) {
  await page.goto(`${baseURL}${RUNTIME_SETTINGS_PATH}`)
  await page.waitForLoadState('load')
  // The page must not have bounced us elsewhere (the old slot-path bug
  // produced /settings//settings/llm-runtime and redirected away).
  await expect(page).toHaveURL(new RegExp(`${RUNTIME_SETTINGS_PATH}$`))
}

/** Open the "Add Provider" drawer and select the local provider type. */
export async function openAddLocalProvider(page: Page, baseURL: string) {
  await page.goto(`${baseURL}/settings/llm-providers`)
  await page.waitForLoadState('load')
  await page.locator('.ant-menu-item:has-text("Add Provider")').first().click()
  await expect(page.locator('.ant-drawer.ant-drawer-open')).toBeVisible()
  // Provider type select → "Local".
  const typeSelect = page.locator('.ant-drawer.ant-drawer-open .ant-select').first()
  await typeSelect.click()
  await page.locator('.ant-select-item:has-text("Local")').first().click()
}

/** Submit the currently-open drawer via its primary submit button. */
export async function submitOpenDrawer(page: Page) {
  const drawer = page.locator('.ant-drawer.ant-drawer-open').last()
  const submit = drawer.locator('.ant-btn-primary[type="submit"], .ant-btn-primary').last()
  await submit.click()
}

// ── API seeding (engine-dependent specs) ────────────────────────────────
// REST helpers so engine-dependent specs can set up state fast instead of
// driving the whole download/create UI. `token` = getCurrentUserToken(page).

const jsonHeaders = (token: string) => ({
  'Content-Type': 'application/json',
  Authorization: `Bearer ${token}`,
})

/** Create a local-type LLM provider. Returns its id. */
export async function seedLocalProvider(baseURL: string, token: string): Promise<string> {
  const name = `e2e-local-${Math.random().toString(36).slice(2, 10)}`
  const res = await fetch(`${baseURL}/api/llm-providers`, {
    method: 'POST',
    headers: jsonHeaders(token),
    body: JSON.stringify({ name, provider_type: 'local', enabled: true }),
  })
  if (!res.ok) {
    throw new Error(`seedLocalProvider failed: ${res.status} - ${await res.text()}`)
  }
  return (await res.json()).id
}

/** Create a local llamacpp model under `providerId`. Returns its id. */
export async function seedLocalModel(
  baseURL: string,
  token: string,
  providerId: string,
  name: string
): Promise<string> {
  const res = await fetch(`${baseURL}/api/llm-models`, {
    method: 'POST',
    headers: jsonHeaders(token),
    body: JSON.stringify({
      provider_id: providerId,
      name,
      display_name: `E2E ${name}`,
      engine_type: 'llamacpp',
      engine_settings: { ctx_size: 512, n_gpu_layers: 0 },
      file_format: 'gguf',
      enabled: true,
    }),
  })
  if (!res.ok) {
    throw new Error(`seedLocalModel failed: ${res.status} - ${await res.text()}`)
  }
  return (await res.json()).id
}

/**
 * Download + default an engine version from the configured release mirror
 * (`LLM_RUNTIME_RELEASE_MIRROR`, wired when `ZIEE_E2E_ENGINE_MIRROR` is set).
 * Detects the host platform/arch so the artifact matches. Returns version id.
 */
export async function downloadEngineViaApi(
  baseURL: string,
  token: string,
  engine = 'llamacpp'
): Promise<string> {
  const headers = jsonHeaders(token)
  const gpu = await (
    await fetch(`${baseURL}/api/local-runtime/detect-gpu`, { headers })
  ).json()
  // The mock release serves an unsigned artifact.
  await fetch(`${baseURL}/api/local-runtime/settings`, {
    method: 'PUT',
    headers,
    body: JSON.stringify({ allow_unsigned_downloads: true }),
  })
  const dl = await fetch(`${baseURL}/api/local-runtime/versions/download`, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      engine,
      version: 'latest',
      platform: gpu.platform,
      arch: gpu.arch,
      backend: 'cpu',
    }),
  })
  if (!dl.ok) {
    throw new Error(`downloadEngineViaApi failed: ${dl.status} - ${await dl.text()}`)
  }
  const versionId = (await dl.json()).version.id
  await fetch(`${baseURL}/api/local-runtime/versions/${versionId}/set-default`, {
    method: 'POST',
    headers,
  })
  return versionId
}
