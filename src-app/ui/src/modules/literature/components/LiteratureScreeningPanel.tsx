import { useState } from 'react'
import { Download } from 'lucide-react'
import { Button, Checkbox, Dropdown, Input, List, Segmented, Space, Tag, Text, Title, Paragraph } from '@/components/ui'
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

  // Transient UI-only state — never persisted to the tab snapshot.
  const [selected, setSelected] = useState<Set<string>>(new Set())
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

  return (
    <div className="p-3 space-y-3 overflow-y-auto" data-testid="lit-screening-panel">
      <Title level={5} className="!mb-0">
        Screening
      </Title>
      <Text type="secondary" className="text-xs">
        “{query}”
      </Text>

      {/* PRISMA-style counts */}
      <Space wrap size="small">
        <Tag variant="outline" data-testid="lit-screening-tag-identified">Identified: {identifiedTotal}</Tag>
        <Tag variant="outline" data-testid="lit-screening-tag-after-dedup">After dedup: {afterDedup}</Tag>
        <Tag variant="outline" tone="info" data-testid="lit-screening-tag-screened">Screened: {screened}</Tag>
        <Tag variant="outline" tone="success" data-testid="lit-screening-tag-included">Included: {counts.include}</Tag>
        <Tag variant="outline" tone="error" data-testid="lit-screening-tag-excluded">Excluded: {counts.exclude}</Tag>
      </Space>

      {degradedSources.length > 0 && (
        <Text type="warning" className="text-xs block">
          Degraded/skipped sources: {degradedSources.join(', ')}
        </Text>
      )}

      {completeness && (
        <div className="rounded-md bg-accent p-3 border border-border" data-testid="lit-screening-completeness">
          <Text className="text-sm font-medium text-foreground">{completeness.estimate.toUpperCase()}</Text>
          <Paragraph className="text-xs text-muted-foreground !mb-0">{completeness.caveat}</Paragraph>
        </div>
      )}

      {/* Bulk-action bar (select rows → apply one decision) + export. */}
      <Space wrap size="small">
        <Checkbox
          aria-label="Select all records"
          checked={allSelected}
          indeterminate={someSelected}
          onChange={(checked: boolean) => toggleSelectAll(checked)}
          label={selected.size > 0 ? `${selected.size} selected` : 'Select all'}
          data-testid="lit-screening-select-all-checkbox"
        />
        <Button size="default" disabled={selected.size === 0} onClick={() => bulkDecide('include')} data-testid="lit-screening-bulk-include-button">
          Include
        </Button>
        <Button size="default" disabled={selected.size === 0} onClick={() => bulkDecide('exclude')} data-testid="lit-screening-bulk-exclude-button">
          Exclude
        </Button>
        <Button size="default" disabled={selected.size === 0} onClick={() => bulkDecide('unscreened')} data-testid="lit-screening-bulk-unscreen-button">
          Unscreen
        </Button>
        <Dropdown
          items={[
            { key: 'ris', label: 'Export RIS' },
            { key: 'bibtex', label: 'Export BibTeX' },
            { key: 'csv', label: 'Export CSV' },
          ]}
          onSelect={(key: string) => doExport(key as 'ris' | 'bibtex' | 'csv')}
          data-testid="lit-screening-export-dropdown"
        >
          <Button icon={<Download />} size="default" disabled={records.length === 0} data-testid="lit-screening-export-button">
            Export {counts.include > 0 ? 'included' : 'all'}
          </Button>
        </Dropdown>
      </Space>

      <List
        size="sm"
        dataSource={records}
        data-testid="lit-screening-records-list"
        rowKey={(_, i) => String(i)}
        empty={
          degradedSources.length > 0
            ? `No records — all sources errored or were skipped (${degradedSources.join(', ')}). Try again or ask the model to re-search.`
            : 'No records returned for this query.'
        }
        renderItem={(r: unknown, i: number) => {
          const record = r as LiteratureRecord
          const key = recordKey(record)
          const decision = decisions[key] ?? 'unscreened'
          return (
            // React key uses the index (recordKey can collide on duplicate
            // records); decisions/reasons/selection still key on recordKey.
            <div key={`${i}-${key}`} className="w-full flex gap-2">
              <div className="flex items-start mt-1">
                <Checkbox
                  aria-label={`Select "${record.title ?? ''}"`}
                  checked={selected.has(key)}
                  onChange={(checked: boolean) => toggleSelect(key, checked)}
                  data-testid={`lit-screening-record-checkbox-${key}`}
                />
              </div>
              <div className="flex-1 min-w-0">
                <Text strong className="text-sm">
                  {i + 1}. {record.title}
                </Text>
                {record.is_preprint && <Tag variant="outline" data-testid={`lit-screening-preprint-${i}`} className="ml-1">preprint</Tag>}
                <Paragraph type="secondary" className="text-xs !mb-0">
                  {record.authors?.slice(0, 3).join(', ')}
                  {record.authors?.length > 3 ? ' et al.' : ''}
                  {record.year ? ` · ${record.year}` : ''}
                  {record.venue ? ` · ${record.venue}` : ''}
                  {` · ${record.source}`}
                </Paragraph>
                {(record.doi || record.pmid) && (
                  <Text type="secondary" className="text-xs block">
                    {record.doi ? `doi:${record.doi}` : ''} {record.pmid ? `pmid:${record.pmid}` : ''}
                  </Text>
                )}
                {record.abstract_text && (
                  <Paragraph
                    type="secondary"
                    className="text-xs !mb-1"
                    ellipsis={true}
                  >
                    {record.abstract_text}
                  </Paragraph>
                )}
                <Segmented
                  size="sm"
                  aria-label="Screening decision"
                  value={decision}
                  onChange={val => setDecision(key, val as ScreeningDecision)}
                  options={[
                    { label: 'Unscreened', value: 'unscreened' },
                    { label: 'Include', value: 'include' },
                    { label: 'Exclude', value: 'exclude' },
                  ]}
                  data-testid={`lit-screening-record-decision-${key}`}
                />
                {decision === 'exclude' && (
                  <Input
                    size="sm"
                    className="mt-1"
                    aria-label="Exclusion reason"
                    placeholder="Exclusion reason (optional)"
                    value={reasonDrafts[key] ?? reasons[key] ?? ''}
                    onChange={e =>
                      setReasonDrafts(d => ({ ...d, [key]: e.target.value }))
                    }
                    onBlur={() => flushReason(key)}
                    data-testid={`lit-screening-record-reason-${key}`}
                  />
                )}
              </div>
            </div>
          )
        }}
      />

      <Text type="secondary" className="text-xs block">
        An adjunct to — not a replacement for — systematic searching. Verify every
        record; cite by DOI/PMID.
      </Text>
    </div>
  )
}
