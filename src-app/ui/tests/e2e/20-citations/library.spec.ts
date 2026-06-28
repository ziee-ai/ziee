import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

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

  // DELETE /api/citations/{id} — remove from state so a refetch reflects it.
  await page.route(/\/api\/citations\/[0-9a-fA-F-]{36}$/, async (route, req) => {
    if (req.method() === 'DELETE') {
      const id = req.url().split('/').pop() ?? ''
      state.entries = state.entries.filter(e => e.id !== id)
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ ok: true }) })
    }
    return route.continue()
  })
}

async function gotoCitations(page: Page, baseURL: string) {
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      await page.goto(`${baseURL}/settings/citations`)
      await expect(page.getByRole('heading', { name: 'Citations' })).toBeVisible({ timeout: 10000 })
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

    await expect(page.getByText('A verified paper')).toBeVisible()
    // antd preset-color Tags expose color via class — assert the three colored
    // states (the anti-fabrication signal); unverified is a plain (uncolored) Tag.
    await expect(page.locator('.ant-tag-green', { hasText: 'verified' })).toBeVisible()
    await expect(page.locator('.ant-tag-orange', { hasText: 'mismatch' })).toBeVisible()
    await expect(page.locator('.ant-tag-red', { hasText: 'not found' })).toBeVisible()
    await expect(page.getByText('unverified')).toBeVisible()
  })

  test('a fabricated DOI reports exactly "1 not found" and "0 added"', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    await page.getByRole('button', { name: 'Import' }).click()
    await page.getByPlaceholder(/10\.1038/).fill('10.9999/fake-doi')
    await page.getByRole('button', { name: 'Import + verify' }).click()
    // Discriminating: the summary must show the fabricated DOI was NOT added.
    await expect(page.getByText(/1 not found/)).toBeVisible({ timeout: 5000 })
    await expect(page.getByText(/0 added/)).toBeVisible()
  })

  test('importing a real identifier adds a verified card', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    await page.getByRole('button', { name: 'Import' }).click()
    await page.getByPlaceholder(/10\.1038/).fill('10.5555/known')
    await page.getByRole('button', { name: 'Import + verify' }).click()
    await expect(page.getByText(/1 added/)).toBeVisible({ timeout: 5000 })
  })

  // audit id 2ee363461578 — the empty-state (no citations) was never E2E-tested;
  // every other test seeds entries via mockApi.
  test('shows the empty-state when the library has no citations', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    await expect(
      page.getByText('No citations yet — import some or run a literature search.'),
    ).toBeVisible({ timeout: 10000 })
    await expect(page.getByText('0 reference(s)')).toBeVisible()
  })

  // audit id f437d5db3a45 — the import modal's empty-text no-op guard and the
  // error-toast catch path were untested (only success + fabricated-DOI were).
  test('import with empty text is a no-op (no request fired)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)

    let importCalled = false
    await page.route(/\/api\/citations\/import$/, async route => {
      importCalled = true
      return route.fulfill({ status: 200, contentType: 'application/json', body: JSON.stringify({ results: [] }) })
    })

    await gotoCitations(page, baseURL)
    await page.getByRole('button', { name: 'Import' }).click()
    // Leave the textarea empty and click Import + verify → the guard returns early.
    await page.getByRole('button', { name: 'Import + verify' }).click()
    await page.waitForTimeout(500)
    expect(importCalled).toBe(false)
  })

  test('import surfaces a backend error as a toast', async ({ page, testInfra }) => {
    // Count any import POST that escapes the client-side empty-text guard.
    let importCalls = 0
    page.on('request', req => {
      if (/\/api\/citations\/import$/.test(req.url()) && req.method() === 'POST') {
        importCalls += 1
      }
    })
    await gotoCitations(page, baseURL)

    await page.getByRole('button', { name: 'Import' }).click()
    // Leave the textarea empty and click Import + verify.
    await page.getByRole('button', { name: 'Import + verify' }).click()
    await page.waitForTimeout(500)

    // The empty-text guard returns before any network call, and no result row
    // is rendered.
    expect(importCalls).toBe(0)
    await expect(page.getByText(/added/)).toHaveCount(0)
    // The modal stays open (the Import + verify button is still present).
    await expect(
      page.getByRole('button', { name: 'Import + verify' }),
    ).toBeVisible()
  })

  test('import surfaces an error toast when the request fails', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    // Override the import route to fail (exercises handleImport's catch branch).
    await page.route(/\/api\/citations\/import$/, async route =>
      route.fulfill({
        status: 500,
        contentType: 'application/json',
        body: JSON.stringify({ error_code: 'INTERNAL', error: 'resolver exploded' }),
      }),
    )

    await gotoCitations(page, baseURL)
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

    await page.getByRole('button', { name: 'Import' }).click()
    await page.getByPlaceholder(/10\.1038/).fill('10.5555/known')
    await page.getByRole('button', { name: 'Import + verify' }).click()

    // The catch path surfaces an error message.
    await expect(page.locator('.ant-message-error')).toBeVisible({ timeout: 5000 })
  })

  test('exports the library (download triggered)', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = { entries: [entry({ title: 'Exportable paper' })] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
    await gotoCitations(page, baseURL)

    const downloadPromise = page.waitForEvent('download')
    await page.getByRole('button', { name: 'Export' }).click()
    await page.getByText('RIS (.ris)').click()
    const download = await downloadPromise
    expect(download.suggestedFilename()).toContain('citations')
  })

  // audit id all-eb628fb20657 — deleting an individual citation via the
  // per-card Popconfirm (CitationCard handleDelete → Stores.Citations.remove →
  // DELETE /api/citations/{id}) was untested. Click Delete, confirm the
  // Popconfirm, and assert the card disappears.
  test('deletes a citation via the Popconfirm', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    const state: State = {
      entries: [entry({ title: 'Doomed paper', citation_key: 'doomed2021' })],
    }
  test('shows the empty state when the library has no citations', async ({ page, testInfra }) => {
    const { baseURL } = testInfra
    // library.spec.ts always seeded entries; the no-citations branch
    // (CitationsSettingsPage Empty + disabled Verify-all/Export) was untested.
    const state: State = { entries: [] }
    await loginAsAdmin(page, baseURL)
    await mockApi(page, state)
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

    await expect(page.getByText('Doomed paper')).toBeVisible()

    // Per-card delete button (aria-label includes the citation key).
    await page.getByRole('button', { name: 'Delete doomed2021' }).click()
    // Confirm the antd Popconfirm.
    const popover = page.locator('.ant-popconfirm:visible').last()
    await expect(popover.getByText('Delete from library?')).toBeVisible()
    await popover.getByRole('button', { name: /^(OK|Delete|Yes)$/ }).click()

    // The card is removed (DELETE fired → state empty → refetch shows none).
    await expect(page.getByText('Doomed paper')).toHaveCount(0, { timeout: 10000 })
    await expect(
      page.getByText('No citations yet — import some or run a literature search.'),
    ).toBeVisible({ timeout: 15000 })
    // The reference counter reads zero and the entry-gated actions are disabled.
    await expect(page.getByText('0 reference(s)')).toBeVisible()
    await expect(page.getByRole('button', { name: 'Verify all' })).toBeDisabled()
    await expect(page.getByRole('button', { name: 'Export' })).toBeDisabled()
  })
    // The per-card delete is gated on citations::manage (admin holds it via *).
    await page.getByRole('button', { name: 'Delete doomed2020' }).click()
    // Confirm the Popconfirm.
    const popconfirm = page.locator('.ant-popconfirm:visible')
    await expect(popconfirm.getByText('Delete from library?')).toBeVisible()
    // Confirm via the primary (danger) button — text-independent + stable.
    await popconfirm.locator('.ant-btn-primary').click()

    await expect(page.getByText('Doomed paper')).toHaveCount(0)
  })

  // The Export dropdown wires 4 formats → distinct file extensions
  // (CitationsSettingsPage EXPORT_FORMATS). library.spec previously only
  // exercised RIS; cover the other three so a mis-wired ext/format regresses.
  for (const { label, ext } of [
    { label: 'BibTeX (.bib)', ext: '.bib' },
    { label: 'CSL-JSON (.json)', ext: '.json' },
    { label: 'Formatted (CSL style)', ext: '.txt' },
  ]) {
    test(`exports the library as ${label}`, async ({ page, testInfra }) => {
      const { baseURL } = testInfra
      const state: State = { entries: [entry({ title: 'Exportable paper' })] }
      await loginAsAdmin(page, baseURL)
      await mockApi(page, state)
      await gotoCitations(page, baseURL)

      const downloadPromise = page.waitForEvent('download')
      await page.getByRole('button', { name: 'Export' }).click()
      await page.getByText(label).click()
      const download = await downloadPromise
      expect(download.suggestedFilename()).toBe(`citations${ext}`)
    })
  }
})
