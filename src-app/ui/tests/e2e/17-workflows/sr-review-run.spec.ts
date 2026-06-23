import { test, expect } from '../../fixtures/test-context'
import { loginAsAdmin, getAdminToken } from '../../common/auth-helpers'
import {
  createProviderViaAPI,
  createModelViaAPI,
  assignProviderToAdministratorsGroup,
} from '../../common/provider-helpers'
import {
  goToWorkflowsSettingsPage,
  openWorkflowCard,
  seedDevWorkflow,
} from './helpers/workflow-helpers'

/**
 * SR-review SETTINGS run: drive the REAL workflow INPUT FORM (the run dialog
 * renders one labeled field per `inputs` entry), start the run, and reach the
 * durable screening gate.
 *
 * Determinism: the seeded workflow bakes `mock:` onto every llm/tool step (the
 * runner honors `step.mock` for dev-imported workflows — no LLM, no lit_search
 * network), while the `screen_review` elicit gate is left REAL so the run
 * SUSPENDS on it. So this spends no tokens; it's real-LLM-tier only because the
 * run snapshots a model at start (never invoked).
 *
 * Scope (per the "test what works" decision): exercises the input form + the run
 * reaching `screen_review` + the "Screen in panel" affordance. It does NOT submit
 * the gate via the generic elicit form — `included_ids` (array-of-strings) isn't
 * fillable there; screening submits through the literature panel (covered by the
 * chat spec).
 */

const ANTHROPIC_KEY = process.env.ANTHROPIC_API_KEY ?? ''
const HAS_ANTHROPIC = ANTHROPIC_KEY.length > 0

// sr-review's real `inputs` shape + a tiny mock-baked chain ending in the real
// `screen_review` durable gate. `search`/`screen` are the bridge's candidate +
// decision steps, so "Screen in panel" surfaces on the suspended run.
const SR_REVIEW_MOCKED_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
max_runtime_secs: 0
inputs:
  - name: query
    description: "The review question / search query."
    required: true
  - name: year_from
    description: "Inclusive lower bound on publication year."
    required: false
    default: 1900
  - name: year_to
    description: "Inclusive upper bound on publication year."
    required: false
    default: 2100
  - name: max_results
    description: "Max deduped candidate records."
    required: false
    default: 100
  - name: max_papers
    description: "Max papers to fetch full text for."
    required: false
    default: 50
steps:
  - id: search
    kind: tool
    server: lit_search
    tool: literature_search
    arguments:
      query: "{{ inputs.query }}"
    mock:
      query: "{{ inputs.query }}"
      records:
        - { doi: "10.1/a", pmid: null, title: "Base editing reduces off-target effects", abstract_text: "A study.", authors: ["A B"], year: 2021, venue: "Nature", url: null, source: "europepmc", source_ids: ["europepmc:1"], cited_by_count: 3, is_preprint: false, relevance: 0.9 }
        - { doi: "10.2/b", pmid: "999", title: "Off-target detection", abstract_text: "Another.", authors: ["C D"], year: 2022, venue: null, url: null, source: "crossref", source_ids: ["crossref:1"], cited_by_count: null, is_preprint: false, relevance: 0.7 }
      identified: { europepmc: 1, crossref: 1 }
      after_dedup: 2
      degraded_sources: []
      completeness: null
  - id: screen
    kind: llm_map
    for_each: "{{ search.output.records }}"
    item_var: paper
    output_format: json
    prompt: "screen {{ paper.title }}"
    on_error: skip
    depends_on: [search]
    mock:
      - { id: "10.1/a", decision: "include", reason: "on-topic", confidence: 0.9 }
      - { id: "10.2/b", decision: "exclude", reason: "off-topic", confidence: 0.6 }
  - id: screen_review
    kind: elicit
    message: "Screen the candidates, then submit the included set to continue."
    timeout_ms: 0
    schema:
      type: object
      properties:
        included_ids:
          type: array
          items: { type: string }
        approved:
          type: boolean
          const: true
      required: [included_ids, approved]
    depends_on: [screen]
outputs:
  - name: candidates
    from: "{{ search.output }}"
    expose: full
  - name: ai_screening
    from: "{{ screen.output }}"
    expose: full
`

// sr-review's GATE 2 (extraction review): a mock `extract` seeds the editable PICO
// table; `review` is the real durable elicit gate; `synthesize` is mocked so the
// run resumes to completion after the human approves.
const SR_REVIEW_REVIEW_GATE_YAML = `$schema: "/schemas/2026-06-12/workflow-definition.schema.json"
max_runtime_secs: 0
inputs:
  - name: question
    description: "The review question the synthesis must answer."
    required: true
steps:
  - id: extract
    kind: llm
    output_format: json
    prompt: "extract for {{ inputs.question }}"
    mock:
      - { id: "10.1/a", population: "200 patients", intervention: "base editing", comparator: "standard CRISPR", outcome: "off-target mutations", effect: "47% reduction", risk_of_bias: "low", confidence: 0.8, quote: "reduced off-target mutations by 47%" }
  - id: review
    kind: elicit
    message: "Review the AI-extracted study data — correct any value or quote, then approve to synthesize."
    data:
      extractions: "{{ extract.output }}"
    schema:
      type: object
      properties:
        extractions:
          type: array
          ui:
            widget: table
          items:
            type: ["object", "null"]
            properties:
              id: { type: string, title: "Study ID", ui: { width: 160 } }
              population: { type: string, title: "Population" }
              intervention: { type: string, title: "Intervention" }
              comparator: { type: string, title: "Comparator" }
              outcome: { type: string, title: "Outcome" }
              effect: { type: string, title: "Effect" }
              risk_of_bias: { type: string, title: "Risk of bias" }
              confidence: { type: number, title: "Confidence", minimum: 0, maximum: 1 }
              quote: { type: string, title: "Supporting quote", ui: { expand: true } }
            required: [id]
        approved:
          type: boolean
          const: true
      required: [approved]
    timeout_ms: 0
    depends_on: [extract]
  - id: synthesize
    kind: llm
    output_format: text
    prompt: "synthesize {{ review.output.extractions }}"
    mock: "## Synthesis. Base editing reduced off-target effects [10.1/a]. Limitations: AI-assisted."
    depends_on: [review]
outputs:
  - name: report
    from: "{{ synthesize.output }}"
    expose: full
    mime_type: text/markdown
`

test.describe('SR-review — settings run input form + screening gate', () => {
  test.skip(!HAS_ANTHROPIC, 'ANTHROPIC_API_KEY not set — model snapshot unavailable')

  test('fill the input form in the run dialog → run → reaches the screening gate', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)

    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )

    await seedDevWorkflow(request, apiURL, adminToken, 'e2e-sr-review', SR_REVIEW_MOCKED_YAML)

    // Open the workflow → its Run dialog (renders one labeled field per input).
    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-sr-review')
    await page.getByRole('button', { name: /Run$/ }).first().click()

    // THE INPUT FORM: every `inputs` entry renders as a field labeled by its name.
    await expect(page.getByLabel('query')).toBeVisible()
    await expect(page.getByLabel('year_from')).toBeVisible()
    await expect(page.getByLabel('max_papers')).toBeVisible()

    await page.getByLabel('query').fill('CRISPR base editing off-target effects')
    // Pick the model (required for a standalone run) + launch.
    await page.getByLabel('Model').click()
    await page.getByText('Claude Haiku 4.5').last().click()
    await page.getByRole('button', { name: 'Run', exact: true }).last().click()

    // The run-progress view (with the gate + the screening affordance) lives on
    // the dedicated "Workflow page" — navigate to it (mirrors durable-resume).
    await page.getByText('Workflow page', { exact: true }).first().click()

    // The run progresses through the mocked steps and SUSPENDS on the durable
    // screening gate (status `waiting`, the gate form renders from the snapshot).
    await expect(page.getByText(/input required/i)).toBeVisible({ timeout: 20000 })
    await expect(page.getByText('waiting', { exact: true }).first()).toBeVisible({ timeout: 10000 })

    // The screening affordance surfaces for the pending `screen_review` gate (my
    // addition) — the primary "Screen in panel" path to screen + resume.
    await expect(page.getByRole('button', { name: 'Screen in panel' })).toBeVisible({
      timeout: 10000,
    })
  })

  test('extraction-review gate: the PICO table renders, approve + submit resumes to completion', async ({
    page,
    request,
    testInfra,
  }) => {
    const { baseURL, apiURL } = testInfra
    await loginAsAdmin(page, baseURL)
    const adminToken = await getAdminToken(apiURL)
    const providerId = await createProviderViaAPI(apiURL, adminToken, 'Anthropic', 'anthropic')
    await assignProviderToAdministratorsGroup(apiURL, adminToken, providerId)
    await createModelViaAPI(
      apiURL,
      adminToken,
      providerId,
      'claude-haiku-4-5-20251001',
      'Claude Haiku 4.5',
      'anthropic',
    )
    await seedDevWorkflow(request, apiURL, adminToken, 'e2e-sr-review-extract', SR_REVIEW_REVIEW_GATE_YAML)

    await goToWorkflowsSettingsPage(page, baseURL)
    await openWorkflowCard(page, 'e2e-sr-review-extract')
    await page.getByRole('button', { name: /Run$/ }).first().click()
    await page.getByLabel('question').fill('Does base editing reduce off-target effects?')
    await page.getByLabel('Model').click()
    await page.getByText('Claude Haiku 4.5').last().click()
    await page.getByRole('button', { name: 'Run', exact: true }).last().click()

    await page.getByText('Workflow page', { exact: true }).first().click()

    // GATE 2: the extraction-review form renders the EditableArrayTable seeded with
    // the AI extraction (`data:`), so the reviewer sees the PICO row.
    await expect(page.getByText(/input required/i)).toBeVisible({ timeout: 20000 })
    await expect(page.getByText('base editing').first()).toBeVisible({ timeout: 10000 })

    // Approve (the `approved` boolean → a Switch) + submit → the run resumes past
    // the gate and the (mocked) synthesize step runs to completion.
    const approve = page.getByRole('switch').first()
    if (await approve.count()) await approve.click()
    await page.getByRole('button', { name: 'Submit', exact: true }).click()

    await expect(page.getByText('completed', { exact: true }).first()).toBeVisible({
      timeout: 30000,
    })
  })
})
