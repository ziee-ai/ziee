import type { Page } from '@playwright/test'
import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin } from '../../common/auth-helpers'

/**
 * E2E — Citations export FORMAT variety (audit gap all-c004e702401e).
 *
 * library.spec.ts only exercises the RIS export. CitationsSettingsPage
 * defines four EXPORT_FORMATS and the Export dropdown wires each menu key
 * through `Stores.Citations.exportLibrary(format)` →
 * `GET /api/citations/export?format=<key>` → the client `download()` helper
 * which names the file `citations.<ext>` per the format's table entry.
 *
 * This asserts the three previously-untested formats (Formatted text,
 * BibTeX, CSL-JSON) each: (1) send the CORRECT `format` query param to the
 * real export endpoint, (2) flow the returned body into a real browser
 * download, and (3) name the download with the format's extension. Only the
 * backend formatter (the HTTP boundary) is mocked — and it echoes a
 * format-distinctive body keyed off the request's own `format` param, so a
 * wrong wiring (e.g. every item sending "ris") would fail the content +
 * extension assertions. The format rendering itself is covered by the
 * backend citations format tests.
 */

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
  verification_status: 'verified'
  verified_at: string | null
  source: string | null
  created_at: string
  updated_at: string
}

function entry(title: string): Entry {
  const now = new Date().toISOString()
  return {
    id: crypto.randomUUID(),
    csl_json: { type: 'article-journal', title, author: [{ family: 'Smith', given: 'J.' }] },
    doi: '10.5555/known',
    pmid: null,
    pmcid: null,
    arxiv_id: null,
    title,
    year: 2021,
    citation_key: 'smith2021',
    verification_status: 'verified',
    verified_at: now,
    source: 'doi',
    created_at: now,
    updated_at: now,
  }
}

// Format-distinctive bodies the mocked formatter returns, keyed off the
// request's own `format` param — so the body proves the right format reached
// the endpoint, not a hardcoded RIS echo.
const BODY_FOR: Record<string, string> = {
  text: 'Smith, J. (2021). A paper. Journal of Things.',
  bibtex: '@article{smith2021,\n  title = {{A paper}},\n  year = {2021}\n}\n',
  csljson: JSON.stringify([{ id: 'smith2021', type: 'article-journal', title: 'A paper' }]),
  ris: 'TY  - JOUR\nTI  - A paper\nER  -\n',
}

async function mockApi(page: Page, lastFormat: { value: string | null }) {
  await page.route(/\/api\/citations(\?.*)?$/, async (route, req) => {
    if (req.method() === 'GET') {
      return route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: JSON.stringify({ entries: [entry('Exportable paper')] }),
      })
    }
    return route.continue()
  })

  await page.route(/\/api\/citations\/export(\?.*)?$/, async (route, req) => {
    const fmt = new URL(req.url()).searchParams.get('format') ?? ''
    lastFormat.value = fmt
    return route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ format: fmt, output: BODY_FOR[fmt] ?? `UNKNOWN:${fmt}` }),
    })
  })

  await page.route(/\/api\/citations\/styles$/, async route =>
    route.fulfill({
      status: 200,
      contentType: 'application/json',
      body: JSON.stringify({ styles: ['apa', 'nature', 'vancouver'] }),
    }),
  )
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

async function readDownload(download: { createReadStream: () => Promise<NodeJS.ReadableStream | null> }): Promise<string> {
  const stream = await download.createReadStream()
  if (!stream) return ''
  const chunks: Buffer[] = []
  for await (const chunk of stream) chunks.push(Buffer.from(chunk))
  return Buffer.concat(chunks).toString('utf-8')
}

test.describe('Citations export — format variety', () => {
  test.describe.configure({ retries: 2 })

  const CASES: { menu: RegExp; format: string; ext: string; assertBody: (s: string) => void }[] = [
    {
      menu: /Formatted \(CSL style\)/,
      format: 'text',
      ext: '.txt',
      assertBody: s => expect(s).toContain('Smith, J. (2021)'),
    },
    {
      menu: /BibTeX/,
      format: 'bibtex',
      ext: '.bib',
      // BibTeX entries always begin with an @-type token.
      assertBody: s => expect(s.trimStart().startsWith('@')).toBeTruthy(),
    },
    {
      menu: /CSL-JSON/,
      format: 'csljson',
      ext: '.json',
      // CSL-JSON is a JSON array of reference objects.
      assertBody: s => expect(Array.isArray(JSON.parse(s))).toBeTruthy(),
    },
  ]

  for (const c of CASES) {
    test(`exports ${c.format} with the right query param, body, and .${c.ext} download`, async ({
      page,
      testInfra,
    }) => {
      const { baseURL } = testInfra
      const lastFormat = { value: null as string | null }
      await loginAsAdmin(page, baseURL)
      await mockApi(page, lastFormat)
      await gotoCitations(page, baseURL)

      const downloadPromise = page.waitForEvent('download')
      await page.getByRole('button', { name: 'Export' }).click()
      await page.getByText(c.menu).click()
      const download = await downloadPromise

      // (1) the UI sent the correct format to the real export endpoint
      expect(lastFormat.value).toBe(c.format)
      // (2) the download is named with the format's extension
      const fn = download.suggestedFilename()
      expect(fn).toContain('citations')
      expect(fn.endsWith(c.ext)).toBeTruthy()
      // (3) the format-distinctive body flowed through to the download
      c.assertBody(await readDownload(download))
    })
  }
})
