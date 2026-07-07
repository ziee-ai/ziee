/**
 * Citations fixture — a populated bibliography for `/settings/citations`.
 *
 * The broad crawl recorded `Citations.list` as `{ entries: [] }` (an empty
 * library on the recording box), so the LOADED gallery state was already the
 * empty state — indistinguishable from the `empty` data-state mode. Seeding real
 * entries here makes `loaded` show the actual reference cards + verification
 * badges, so the `empty` mode (arrays deep-emptied by `toEmpty`) renders a
 * genuinely different `<Empty>` state.
 *
 * Typed against the generated response type so a shape drift fails `tsc`; the
 * ajv contract test (`gallery:check-fixtures`) validates it against openapi.json.
 */
import type { ListCitationsResponse } from '@/api-client/types'
import type { Cassette } from '../mockApi'

// A small, varied library: verified / unverified / mismatch statuses, DOIs, a
// preprint, and CSL-JSON author metadata the CitationCard renders (family/given).
const citationsList: ListCitationsResponse = {
  entries: [
    {
      id: '11111111-1111-4111-8111-111111111111',
      citation_key: 'vaswani2017attention',
      title: 'Attention Is All You Need',
      doi: '10.48550/arXiv.1706.03762',
      arxiv_id: '1706.03762',
      year: 2017,
      source: 'crossref',
      verification_status: 'verified',
      verified_at: '2026-07-04T18:22:10Z',
      created_at: '2026-07-04T18:20:00Z',
      updated_at: '2026-07-04T18:22:10Z',
      csl_json: {
        type: 'article-journal',
        title: 'Attention Is All You Need',
        author: [
          { family: 'Vaswani', given: 'Ashish' },
          { family: 'Shazeer', given: 'Noam' },
          { family: 'Parmar', given: 'Niki' },
          { family: 'Uszkoreit', given: 'Jakob' },
          { family: 'Jones', given: 'Llion' },
          { family: 'Gomez', given: 'Aidan' },
        ],
        issued: { 'date-parts': [[2017]] },
      },
    },
    {
      id: '22222222-2222-4222-8222-222222222222',
      citation_key: 'devlin2019bert',
      title:
        'BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding',
      doi: '10.18653/v1/N19-1423',
      pmid: '31178961',
      year: 2019,
      source: 'pubmed',
      verification_status: 'verified',
      verified_at: '2026-07-04T18:24:31Z',
      created_at: '2026-07-04T18:21:00Z',
      updated_at: '2026-07-04T18:24:31Z',
      csl_json: {
        type: 'paper-conference',
        title:
          'BERT: Pre-training of Deep Bidirectional Transformers for Language Understanding',
        author: [
          { family: 'Devlin', given: 'Jacob' },
          { family: 'Chang', given: 'Ming-Wei' },
          { family: 'Lee', given: 'Kenton' },
          { family: 'Toutanova', given: 'Kristina' },
        ],
        issued: { 'date-parts': [[2019]] },
      },
    },
    {
      id: '33333333-3333-4333-8333-333333333333',
      citation_key: 'lecun2015deep',
      title: 'Deep learning',
      doi: '10.1038/nature14539',
      year: 2015,
      source: 'crossref',
      verification_status: 'mismatch',
      created_at: '2026-07-04T18:25:00Z',
      updated_at: '2026-07-04T18:25:40Z',
      csl_json: {
        type: 'article-journal',
        title: 'Deep learning',
        author: [
          { family: 'LeCun', given: 'Yann' },
          { family: 'Bengio', given: 'Yoshua' },
          { family: 'Hinton', given: 'Geoffrey' },
        ],
        issued: { 'date-parts': [[2015]] },
      },
    },
    {
      id: '44444444-4444-4444-8444-444444444444',
      citation_key: 'grant2004handbook',
      title: 'Handbook of Systematic Review Methods',
      year: 2004,
      verification_status: 'unverified',
      created_at: '2026-07-04T18:26:00Z',
      updated_at: '2026-07-04T18:26:00Z',
      csl_json: {
        type: 'book',
        title: 'Handbook of Systematic Review Methods',
        author: [{ family: 'Grant', given: 'Maria' }],
        issued: { 'date-parts': [[2004]] },
      },
    },
  ],
}

export const citationsCassette: Cassette = {
  // The library view (`/settings/citations`) calls this WITHOUT a project_id and
  // wants the populated list. The project-scoped bibliography surfaces
  // (`ProjectBibliographyInlinePreview` / `ManagePanel`) call it WITH a
  // `project_id` and are the EMPTY-state gallery entries — so a project-scoped
  // query resolves to zero entries (otherwise the "empty" surface shows the 4
  // library rows as "4 reference(s)").
  'Citations.list': ({ query }) =>
    query.project_id ? { entries: [] } : citationsList,
}
