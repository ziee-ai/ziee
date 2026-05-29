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

/** Create a local model row under `providerId` (default engine llamacpp).
 * Returns its id. The row resolves to the engine's default version; no real
 * model file is downloaded (sufficient for listing/swap/delete-guard tests). */
export async function seedLocalModel(
  baseURL: string,
  token: string,
  providerId: string,
  name: string,
  engine: 'llamacpp' | 'mistralrs' = 'llamacpp'
): Promise<string> {
  const res = await fetch(`${baseURL}/api/llm-models`, {
    method: 'POST',
    headers: jsonHeaders(token),
    body: JSON.stringify({
      provider_id: providerId,
      name,
      display_name: `E2E ${name}`,
      engine_type: engine,
      engine_settings: engine === 'llamacpp' ? { ctx_size: 512, n_gpu_layers: 0 } : {},
      file_format: engine === 'llamacpp' ? 'gguf' : 'safetensors',
      enabled: true,
    }),
  })
  if (!res.ok) {
    throw new Error(`seedLocalModel failed: ${res.status} - ${await res.text()}`)
  }
  return (await res.json()).id
}

/**
 * Download an engine version from the REAL `ziee-ai/*` GitHub release (the
 * backend has no mirror env set, so it hits github.com — same path
 * `gold_smoke` proves). Detects the host platform/arch so the cpu artifact
 * matches; enables unsigned downloads (the fork releases aren't cosign-signed).
 * `version` is a real tag (e.g. `v0.0.1-alpha`) or `latest`. Returns version id.
 */
export async function downloadEngineViaApi(
  baseURL: string,
  token: string,
  engine = 'llamacpp',
  version = 'latest',
  setDefault = true
): Promise<string> {
  const headers = jsonHeaders(token)
  const gpu = await (
    await fetch(`${baseURL}/api/local-runtime/detect-gpu`, { headers })
  ).json()
  await fetch(`${baseURL}/api/local-runtime/settings`, {
    method: 'PUT',
    headers,
    // CPU cold-load + first token is slow; the 30s default is too short to
    // reach a healthy instance on a manual start (gold_smoke uses 180s too).
    body: JSON.stringify({ allow_unsigned_downloads: true, auto_start_timeout_secs: 180 }),
  })
  const dl = await fetch(`${baseURL}/api/local-runtime/versions/download`, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      engine,
      version,
      platform: gpu.platform,
      arch: gpu.arch,
      backend: 'cpu',
    }),
  })
  if (!dl.ok) {
    throw new Error(`downloadEngineViaApi failed: ${dl.status} - ${await dl.text()}`)
  }
  const versionId = (await dl.json()).version.id
  if (setDefault) {
    await fetch(`${baseURL}/api/local-runtime/versions/${versionId}/set-default`, {
      method: 'POST',
      headers,
    })
  }
  return versionId
}

/**
 * Download a real tiny chat GGUF (TinyLlama-1.1B Q4_K_M) from HuggingFace
 * under `providerId`, polling until the download completes. Mirrors the
 * backend `gold_smoke` model setup. Returns the committed model {id, name}.
 * Requires `HUGGINGFACE_API_KEY` in the backend env (~670 MB download).
 */
export async function downloadGgufModelViaApi(
  baseURL: string,
  token: string,
  providerId: string
): Promise<{ id: string; name: string }> {
  const headers = jsonHeaders(token)

  // Resolve the built-in Hugging Face repository id.
  const reposBody = await (
    await fetch(`${baseURL}/api/llm-repositories`, { headers })
  ).json()
  const repos = Array.isArray(reposBody) ? reposBody : (reposBody.repositories ?? [])
  const hf = repos.find((r: { name?: string }) => /hugging\s*face/i.test(r.name ?? ''))
  if (!hf) throw new Error('downloadGgufModelViaApi: Hugging Face repository not found')

  // Authenticate the repo so the LFS pull isn't anonymous (else HF 401/403).
  // The key is in the Playwright process env (same one the gate reads).
  const apiKey = process.env.HUGGINGFACE_API_KEY
  if (apiKey) {
    await fetch(`${baseURL}/api/llm-repositories/${hf.id}`, {
      method: 'POST',
      headers,
      body: JSON.stringify({
        auth_config: {
          api_key: apiKey,
          auth_test_api_endpoint: 'https://huggingface.co/api/whoami-v2'
        }
      })
    })
  }

  const name = `e2e-tinyllama-${Date.now()}`
  const start = await fetch(`${baseURL}/api/llm-models/download`, {
    method: 'POST',
    headers,
    body: JSON.stringify({
      provider_id: providerId,
      repository_id: hf.id,
      repository_path: 'TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF',
      repository_branch: 'main',
      name,
      display_name: 'E2E TinyLlama',
      file_format: 'gguf',
      main_filename: 'tinyllama-1.1b-chat-v1.0.Q4_K_M.gguf',
      source: { type: 'hub', id: 'TheBloke/TinyLlama-1.1B-Chat-v1.0-GGUF' },
      engine_type: 'llamacpp',
      engine_settings: { ctx_size: 2048, n_gpu_layers: 0 },
      enabled: true,
    }),
  })
  if (!start.ok) {
    throw new Error(`model download init failed: ${start.status} - ${await start.text()}`)
  }
  const downloadId = (await start.json()).id

  // Poll until the download commits the model (large GGUF → minutes).
  for (let i = 0; i < 120; i++) {
    await new Promise(r => setTimeout(r, 5000))
    const sres = await fetch(`${baseURL}/api/llm-models/downloads/${downloadId}`, {
      headers,
    })
    if (!sres.ok) continue
    const sd = await sres.json()
    if (sd.status === 'completed' && sd.model_id) {
      // The commit kicks off async Tier-2 validation, which spins up a probe
      // engine instance. Wait for it to settle so it doesn't race a later
      // manual start (which 409s while any instance exists).
      await waitForModelValidation(baseURL, token, sd.model_id)
      return { id: sd.model_id, name }
    }
    if (sd.status === 'failed' || sd.status === 'cancelled') {
      throw new Error(`model download ${sd.status}: ${sd.error_message ?? ''}`)
    }
  }
  throw new Error('model download did not complete within timeout')
}

/** Poll a model's validation_status until terminal (the probe instance is
 * stopped by then). Tolerant: returns after a bounded wait regardless. */
async function waitForModelValidation(baseURL: string, token: string, modelId: string) {
  const headers = jsonHeaders(token)
  for (let i = 0; i < 60; i++) {
    const res = await fetch(`${baseURL}/api/llm-models/${modelId}`, { headers })
    if (res.ok) {
      const status = (await res.json()).validation_status
      if (['valid', 'validation_warning', 'invalid', 'failed'].includes(status)) {
        return
      }
    }
    await new Promise(r => setTimeout(r, 2000))
  }
}
