import { useState } from 'react'
import { DownloadOutlined } from '@ant-design/icons'
import { App, Alert, Button, Checkbox, Dropdown, Input, List, Segmented, Space, Tag, Typography } from 'antd'
import { ApiClient } from '@/api-client'
import { Stores } from '@/core/stores'
import {
  type LiteratureRecord,
  type LiteratureScreeningData,
  recordKey,
  type ScreeningDecision,
} from '../types'
import { downloadText, toBibtex, toCsv, toRis } from '../utils/citationFormats'

/**
 * The right-panel screening workbench (registered as the `literature` panel
 * renderer). Props ARE the serialized tab data; decisions are persisted back via
 * `updateRightPanelTab` so they survive reload. Screening-only — searches are
 * model-initiated (this panel is opened from the tool-result card).
 */
export function LiteratureScreeningPanel(data: LiteratureScreeningData) {
  const { records, decisions, reasons, query, completeness, identified, afterDedup, degradedSources } = data
  const { message } = App.useApp()

  // Transient UI-only state — never persisted to the tab snapshot.
  const [selected, setSelected] = useState<Set<string>>(new Set())
  const [submitting, setSubmitting] = useState(false)
  // Exclusion-reason DRAFTS: typed locally per keystroke (cheap), flushed to the
  // persisted snapshot on blur — so we don't re-serialize the whole records array
  // to localStorage on every character.
  const [reasonDrafts, setReasonDrafts] = useState<Record<string, string>>({})

  // Apply an update computed from the FRESHEST tab data in the store (not the
  // closed-over props) — so rapid successive edits (quick include/exclude
  // clicks, or a blur landing between renders) don't clobber each other with a
  // stale snapshot. The updater receives the current data and returns a patch.
  const persist = (update: (cur: LiteratureScreeningData) => Partial<LiteratureScreeningData>) => {
    const current =
      (Stores.Chat.__state.rightPanel.tabs.find(t => t.id === data.sessionId)?.data as
        | LiteratureScreeningData
        | undefined) ?? data
    Stores.Chat.__state.updateRightPanelTab<'literature'>(data.sessionId, {
      ...current,
      ...update(current),
    })
  }

  const setDecision = (key: string, decision: ScreeningDecision) => {
    persist(cur => ({ decisions: { ...cur.decisions, [key]: decision } }))
  }

  // Per-row exclusion reason (ASReview-style); kept and exported in the CSV.
  // Flush the draft into the persisted snapshot (called on blur).
  const flushReason = (key: string) => {
    const draft = reasonDrafts[key]
    if (draft !== undefined) {
      // Read the fresh snapshot to decide; only persist on an ACTUAL change so a
      // no-op blur doesn't rewrite the whole tab snapshot to localStorage.
      const cur =
        (Stores.Chat.__state.rightPanel.tabs.find(t => t.id === data.sessionId)?.data as
          | LiteratureScreeningData
          | undefined) ?? data
      if (draft !== (cur.reasons[key] ?? '')) {
        persist(c => ({ reasons: { ...c.reasons, [key]: draft } }))
      }
    }
    // Drop the committed draft so the input falls back to the persisted value
    // (avoids a growing stale-draft map across edits).
    setReasonDrafts(d => {
      const next = { ...d }
      delete next[key]
      return next
    })
  }

  const toggleSelect = (key: string, checked: boolean) => {
    setSelected(prev => {
      const next = new Set(prev)
      if (checked) next.add(key)
      else next.delete(key)
      return next
    })
  }

  // Compare against the UNIQUE-key population (recordKey can collide on
  // duplicate records) so "select all" isn't stuck on indeterminate.
  const uniqueKeys = new Set(records.map(recordKey))
  const allSelected = selected.size > 0 && selected.size === uniqueKeys.size
  const someSelected = selected.size > 0 && !allSelected

  const toggleSelectAll = (checked: boolean) =>
    setSelected(checked ? new Set(uniqueKeys) : new Set())

  // Bulk-apply a decision to every selected row in one persist + clear selection.
  const bulkDecide = (decision: ScreeningDecision) => {
    if (selected.size === 0) return
    persist(cur => {
      const next = { ...cur.decisions }
      for (const key of selected) next[key] = decision
      return { decisions: next }
    })
    setSelected(new Set())
  }

  const counts = records.reduce(
    (acc, r) => {
      const d = decisions[recordKey(r)] ?? 'unscreened'
      acc[d] += 1
      return acc
    },
    { include: 0, exclude: 0, unscreened: 0 } as Record<ScreeningDecision, number>,
  )
  const screened = counts.include + counts.exclude
  const identifiedTotal = Object.values(identified).reduce((a, b) => a + b, 0)

  const included = (): LiteratureRecord[] =>
    records.filter(r => (decisions[recordKey(r)] ?? 'unscreened') === 'include')

  const doExport = (fmt: 'ris' | 'bibtex' | 'csv') => {
    const inc = included()
    const set = inc.length > 0 ? inc : records // included-only, else all
    // Merge any in-progress reason drafts (typed but not yet blurred) so the CSV
    // never drops a reason the user just typed before clicking Export.
    const mergedReasons = { ...reasons, ...reasonDrafts }
    if (fmt === 'ris') downloadText('screening.ris', 'application/x-research-info-systems', toRis(set))
    else if (fmt === 'bibtex') downloadText('screening.bib', 'application/x-bibtex', toBibtex(set))
    else downloadText('screening.csv', 'text/csv', toCsv(set, decisions, mergedReasons))
  }

  // When opened from a SUSPENDED `sr-review` screening gate, resume the run:
  // derive `included_ids` from the Include decisions and submit the gate's
  // elicitation. Reads the FRESHEST tab data (decisions persist asynchronously),
  // and submits the API directly so we get success/error feedback here (the run
  // view stays in sync via the SSE `elicitationResolved` event).
  const submitScreening = async () => {
    if (!data.runId || !data.elicitationId) return
    setSubmitting(true)
    try {
      const cur =
        (Stores.Chat.__state.rightPanel.tabs.find(t => t.id === data.sessionId)?.data as
          | LiteratureScreeningData
          | undefined) ?? data
      const mergedReasons = { ...cur.reasons, ...reasonDrafts }
      const includedIds: string[] = []
      // The gate's `decisions` items accept {id, decision, reason}; omit
      // `confidence` (the schema bounds it to [0,1] and a record's relevance score
      // may exceed 1). `decisions` is an optional audit record — only
      // `included_ids` drives the downstream full-text fetch.
      const decisionsOut: Array<{ id: string; decision: string; reason: string }> = []
      for (const r of cur.records) {
        // Only records with a resolvable identifier can be fetched downstream.
        const id = r.doi || (r.pmid != null ? String(r.pmid) : '')
        if (!id) continue
        const decision = (cur.decisions[recordKey(r)] ?? 'unscreened') === 'include' ? 'include' : 'exclude'
        if (decision === 'include') includedIds.push(id)
        decisionsOut.push({ id, decision, reason: mergedReasons[recordKey(r)] ?? '' })
      }
      if (includedIds.length === 0) {
        message.warning('Mark at least one study as Include before continuing the review.')
        return
      }
      await ApiClient.Workflow.submitElicit({
        run_id: data.runId,
        elicitation_id: data.elicitationId,
        response: { included_ids: includedIds, decisions: decisionsOut, approved: true },
      })
      message.success(
        `Screening submitted — the review is continuing with ${includedIds.length} included stud${includedIds.length === 1 ? 'y' : 'ies'}.`,
      )
      // The gate is resolved; clear the handle so this becomes a read-only record
      // (hides the button + prevents a resubmit).
      persist(() => ({ runId: undefined, elicitationId: undefined }))
    } catch (e) {
      message.error(`Could not submit screening: ${e instanceof Error ? e.message : String(e)}`)
    } finally {
      setSubmitting(false)
    }
  }

  return (
    <div className="p-3 space-y-3 overflow-y-auto">
      <Typography.Title level={5} className="!mb-0">
        Screening
      </Typography.Title>
      <Typography.Text type="secondary" className="text-xs">
        “{query}”
      </Typography.Text>

      {/* PRISMA-style counts */}
      <Space wrap size="small">
        <Tag>Identified: {identifiedTotal}</Tag>
        <Tag>After dedup: {afterDedup}</Tag>
        <Tag color="processing">Screened: {screened}</Tag>
        <Tag color="success">Included: {counts.include}</Tag>
        <Tag color="error">Excluded: {counts.exclude}</Tag>
      </Space>

      {degradedSources.length > 0 && (
        <Typography.Text type="warning" className="text-xs block">
          Degraded/skipped sources: {degradedSources.join(', ')}
        </Typography.Text>
      )}

      {completeness && (
        <Alert
          type="info"
          showIcon
          title={`Saturation estimate: ${completeness.estimate.toUpperCase()}`}
          description={completeness.caveat}
        />
      )}

      {/* Resume affordance: shown only when opened from a SUSPENDED sr-review
          screening gate. The run stays paused until submitted (survives reload),
          so the human can screen across sessions, then continue. */}
      {data.runId && data.elicitationId && (
        <Alert
          type="warning"
          showIcon
          title="This review is paused for your screening"
          description={
            <div className="flex flex-col items-start gap-2">
              <Typography.Text className="text-xs">
                Mark studies Include / Exclude below, then submit to resume the review on the
                included set. You can return to this later — the run stays paused.
              </Typography.Text>
              <Button
                type="primary"
                size="small"
                loading={submitting}
                disabled={records.length === 0}
                onClick={() => void submitScreening()}
              >
                Submit screening &amp; continue ({counts.include} included)
              </Button>
            </div>
          }
        />
      )}

      {/* Bulk-action bar (select rows → apply one decision) + export. */}
      <Space wrap size="small">
        <Checkbox
          aria-label="Select all records"
          checked={allSelected}
          indeterminate={someSelected}
          onChange={e => toggleSelectAll(e.target.checked)}
        >
          {selected.size > 0 ? `${selected.size} selected` : 'Select all'}
        </Checkbox>
        <Button size="small" disabled={selected.size === 0} onClick={() => bulkDecide('include')}>
          Include
        </Button>
        <Button size="small" disabled={selected.size === 0} onClick={() => bulkDecide('exclude')}>
          Exclude
        </Button>
        <Button size="small" disabled={selected.size === 0} onClick={() => bulkDecide('unscreened')}>
          Unscreen
        </Button>
        <Dropdown
          menu={{
            items: [
              { key: 'ris', label: 'Export RIS' },
              { key: 'bibtex', label: 'Export BibTeX' },
              { key: 'csv', label: 'Export CSV' },
            ],
            onClick: ({ key }) => doExport(key as 'ris' | 'bibtex' | 'csv'),
          }}
        >
          <Button icon={<DownloadOutlined />} size="small" disabled={records.length === 0}>
            Export {counts.include > 0 ? 'included' : 'all'}
          </Button>
        </Dropdown>
      </Space>

      <List
        size="small"
        dataSource={records}
        locale={{
          emptyText:
            degradedSources.length > 0
              ? `No records — all sources errored or were skipped (${degradedSources.join(', ')}). Try again or ask the model to re-search.`
              : 'No records returned for this query.',
        }}
        renderItem={(r, i) => {
          const key = recordKey(r)
          const decision = decisions[key] ?? 'unscreened'
          return (
            // React key uses the index (recordKey can collide on duplicate
            // records); decisions/reasons/selection still key on recordKey.
            <List.Item key={`${i}-${key}`}>
              <div className="w-full flex gap-2">
                <Checkbox
                  className="mt-1"
                  aria-label={`Select "${r.title}"`}
                  checked={selected.has(key)}
                  onChange={e => toggleSelect(key, e.target.checked)}
                />
                <div className="flex-1 min-w-0">
                <Typography.Text strong className="text-sm">
                  {i + 1}. {r.title}
                </Typography.Text>
                {r.is_preprint && <Tag className="ml-1">preprint</Tag>}
                <Typography.Paragraph type="secondary" className="text-xs !mb-0">
                  {r.authors.slice(0, 3).join(', ')}
                  {r.authors.length > 3 ? ' et al.' : ''}
                  {r.year ? ` · ${r.year}` : ''}
                  {r.venue ? ` · ${r.venue}` : ''}
                  {` · ${r.source}`}
                </Typography.Paragraph>
                {(r.doi || r.pmid) && (
                  <Typography.Text type="secondary" className="text-xs block">
                    {r.doi ? `doi:${r.doi}` : ''} {r.pmid ? `pmid:${r.pmid}` : ''}
                  </Typography.Text>
                )}
                {r.abstract_text && (
                  <Typography.Paragraph
                    type="secondary"
                    className="text-xs !mb-1"
                    ellipsis={{ rows: 3, expandable: true, symbol: 'more' }}
                  >
                    {r.abstract_text}
                  </Typography.Paragraph>
                )}
                <Segmented
                  size="small"
                  value={decision}
                  onChange={val => setDecision(key, val as ScreeningDecision)}
                  options={[
                    { label: 'Unscreened', value: 'unscreened' },
                    { label: 'Include', value: 'include' },
                    { label: 'Exclude', value: 'exclude' },
                  ]}
                />
                {decision === 'exclude' && (
                  <Input
                    size="small"
                    className="mt-1"
                    placeholder="Exclusion reason (optional)"
                    value={reasonDrafts[key] ?? reasons[key] ?? ''}
                    onChange={e =>
                      setReasonDrafts(d => ({ ...d, [key]: e.target.value }))
                    }
                    onBlur={() => flushReason(key)}
                  />
                )}
                </div>
              </div>
            </List.Item>
          )
        }}
      />

      <Typography.Text type="secondary" className="text-xs block">
        An adjunct to — not a replacement for — systematic searching. Verify every
        record; cite by DOI/PMID.
      </Typography.Text>
    </div>
  )
}
