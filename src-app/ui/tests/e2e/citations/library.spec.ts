import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'
import { byTestId } from '../testid'

// The citations library lives at Settings → Citations. These specs route-mock
// the citations REST surface so they assert the UI contract (card list,
// verification badges, import result, export) deterministically without the
// resolve/network path (covered by the backend mock-resolve + real-egress tiers).

type Entry = {
  id: string
  csl_json: Record<string, unknown>
  doi: string | null
  pmid: string | null
  pmcid: string | null
  arxiv_id: string | null
  title: string | null
  year: number | null
  citation_key: string
  verification_status: 'verified' | 'mismatch' | 'not_found' | 'unverified'
  verified_at: string | null
  source: string | null
  created_at: string
  updated_at: string
}

function entry(over: Partial<Entry>): Entry {
  return {
    id: crypto.randomUUID(),
    csl_json: { type: 'article-journal', title: over.title ?? 'A paper', author: [{ family: 'Smith', given: 'J.' }] },
    doi: '10.5555/known',
    pmid: null,
    pmcid: null,
    arxiv_id: null,
    title: 'A paper',
    year: 2021,
    citation_key: 'smith2021',
    verification_status: 'verified',
    verified_at: new Date().toISOString(),
    source: 'doi',
    created_at: new Date().toISOString(),
    updated_at: new Date().toISOString(),
    ...over,
  }
}

type State = { entries: Entry[] }

async function mockApi(page: Page, state: State) {
  await page.route(/\/api\/citations(\?.*)?$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ entries: state.entries }),
      })
    }
    return route.continue()
  })

  await page.route(/\/api\/citations\/import$/, async (route, req) => {
    if (req.method() === 'POST') {
      const body = JSON.parse(req.postData() ?? '{}') as { items: { id?: string }[] }
      const results = body.items.map(it => {
        const fabricated = (it.id ?? '').includes('fake')
        if (fabricated) {
          return {
            input: it.id,
            entry_id: null,
            citation_key: null,
            dedup_outcome: 'failed',
            verification_status: 'not_found',
            possible_duplicate_of: null,
            mismatch_fields: null,
            reason: 'identifier did not resolve',
          }
        }
        const e = entry({ doi: it.id ?? '10.5555/known', title: 'Imported paper' })
        state.entries.push(e)
        return {
          input: it.id,
          entry_id: e.id,
          citation_key: e.citation_key,
          dedup_outcome: 'inserted',
          verification_status: 'verified',
          possible_duplicate_of: null,
          mismatch_fields: null,
          reason: null,
        }
      })
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results }) })
    }
    return route.continue()
  })

  await page.route(/\/api\/citations\/export(\?.*)?$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ format: 'ris', output: 'TY  - JOUR\nTI  - A paper\nER  -\n' }),
      })
    }
    return route.continue()
  })

  await page.route(/\/api\/citations\/styles$/, async route =>
    route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ styles: ['apa', 'nature', 'vancouver'] }) }),
  )
}

async function gotoCitations(page: Page, baseURL: string) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await page.goto(`${baseURL}/settings/citations`)
      await expect(byTestId(page, 'cite-settings-card')).toBeVisible({ timeout: 10000 })
      return
    } catch (e) {
      if (attempt === 3) throw e
      await page.waitForTimeout(1000)
    }
  }
}

test.describe('Citations library', () => {
  test.describe.configure({ retries: 2 })

  test('renders the verification-badge states with the right colors', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = {
      entries: [
        entry({ title: 'A verified paper', verification_status: 'verified' }),
        entry({ title: 'A mismatched paper', verification_status: 'mismatch' }),
        entry({ title: 'A fabricated paper', verification_status: 'not_found', doi: '10.9999/fake' }),
        entry({ title: 'A book with no DOI', verification_status: 'unverified', doi: null }),
      ],
    }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    // Each verification state renders its own distinctly-keyed badge (the
    // anti-fabrication signal): verified / mismatch / not-found / unverified.
    await expect(byTestId(page, 'cite-badge-verified')).toBeVisible()
    await expect(byTestId(page, 'cite-badge-mismatch')).toBeVisible()
    await expect(byTestId(page, 'cite-badge-not-found')).toBeVisible()
    await expect(byTestId(page, 'cite-badge-unverified')).toBeVisible()
  })

  test('a fabricated DOI reports exactly "1 not found" and "0 added"', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    await byTestId(page, 'cite-settings-import-button').click()
    await byTestId(page, 'cite-import-textarea').fill('10.9999/fake-doi')
    await byTestId(page, 'cite-import-submit').click()
    // Discriminating: the summary must show the fabricated DOI was NOT added.
    const resultAlert = byTestId(page, 'cite-import-result-alert')
    await expect(resultAlert).toContainText('1 not found', { timeout: 5000 })
    await expect(resultAlert).toContainText('0 added')
  })

  test('importing a real identifier adds a verified card', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    await byTestId(page, 'cite-settings-import-button').click()
    await byTestId(page, 'cite-import-textarea').fill('10.5555/known')
    await byTestId(page, 'cite-import-submit').click()
    await expect(byTestId(page, 'cite-import-result-alert')).toContainText('1 added', { timeout: 5000 })
  })

  test('import with empty text is a no-op (no request fired)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    // Count any import POST that escapes the client-side empty-text guard.
    let importCalls = 0
    page.on('request', req => {
      if (/\/api\/citations\/import$/.test(req.url()) && req.method() === 'POST') {
        importCalls += 1
      }
    })
    await gotoCitations(page, baseURL)

    await byTestId(page, 'cite-settings-import-button').click()
    // Leave the textarea empty and click Import + verify.
    await byTestId(page, 'cite-import-submit').click()
    await page.waitForTimeout(500)

    // The empty-text guard returns before any network call, and no result row
    // is rendered.
    expect(importCalls).toBe(0)
    await expect(byTestId(page, 'cite-import-result-alert')).toHaveCount(0)
    // The modal stays open (the Import + verify button is still present).
    await expect(byTestId(page, 'cite-import-submit')).toBeVisible()
  })

  test('import surfaces an error toast when the request fails', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    // Override the import endpoint to fail.
    await page.route(/\/api\/citations\/import$/, async (route, req) => {
      if (req.method() === 'POST') {
        return route.fulfill({
          status: 500,
          contentType: 'application/json',
          body: JSON.stringify({ message: 'resolver unavailable' }),
        })
      }
      return route.continue()
    })
    await gotoCitations(page, baseURL)

    await byTestId(page, 'cite-settings-import-button').click()
    await byTestId(page, 'cite-import-textarea').fill('10.5555/known')
    await byTestId(page, 'cite-import-submit').click()

    // The catch path surfaces an error toast (sonner error toast).
    await expect(
      page.locator('[data-sonner-toast][data-type="error"]'),
    ).toBeVisible({ timeout: 5000 })
  })

  test('exports the library (download triggered)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [entry({ title: 'Exportable paper' })] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    const downloadPromise = page.waitForEvent('download')
    await byTestId(page, 'cite-settings-export-button').click()
    await byTestId(page, 'cite-settings-export-dropdown-item-ris').click()
    const download = await downloadPromise
    expect(download.suggestedFilename()).toContain('citations')
  })

  test('deletes a citation via the card Popconfirm', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const target = entry({ title: 'Doomed paper', citation_key: 'doomed2020' })
    const state: State = { entries: [target] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    // Mock the DELETE so the real network call (which the store awaits before
    // pruning local state) succeeds deterministically.
    await page.route(/\/api\/citations\/[0-9a-f-]+$/, async (route, req) => {
      if (req.method() === 'DELETE') {
        state.entries = state.entries.filter(e => e.id !== target.id)
        return route.fulfill({ status: 204, body: '' })
      }
      return route.continue()
    })
    await gotoCitations(page, baseURL)

    await expect(byTestId(page, `cite-card-${target.id}`)).toBeVisible()

    // The per-card delete is gated on citations::manage (admin holds it via *).
    await byTestId(page, `cite-card-delete-button-${target.id}`).click()
    // Confirm the Popconfirm via its dedicated confirm control.
    await byTestId(page, `cite-card-delete-confirm-${target.id}`).click()

    await expect(byTestId(page, `cite-card-${target.id}`)).toHaveCount(0)
  })

  // The Export dropdown wires 4 formats → distinct file extensions
  // (CitationsSettingsPage EXPORT_FORMATS). library.spec previously only
  // exercised RIS; cover the other three so a mis-wired ext/format regresses.
  for (const { label, key, ext } of [
    { label: 'BibTeX (.bib)', key: 'bibtex', ext: '.bib' },
    { label: 'CSL-JSON (.json)', key: 'csljson', ext: '.json' },
    { label: 'Formatted (CSL style)', key: 'text', ext: '.txt' },
  ]) {
    test(`exports the library as ${label}`, async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      const state: State = { entries: [entry({ title: 'Exportable paper' })] }
      await loginAsAdmin(page, baseURL)
      await mockApi(page, state)
      await gotoCitations(page, baseURL)

      const downloadPromise = page.waitForEvent('download')
      await byTestId(page, 'cite-settings-export-button').click()
      await byTestId(page, `cite-settings-export-dropdown-item-${key}`).click()
      const download = await downloadPromise
      expect(download.suggestedFilename()).toBe(`citations${ext}`)
    })
  }
})
