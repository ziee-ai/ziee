import { useState } from 'react'
import { Alert } from '@/components/ui'
import { Dialog, Paragraph, Text } from '@/components/ui'
import { message } from '@/components/ui'
import type { BatchReport, CitationInput } from '@/api-client/types'
import { Stores } from '@/core/stores'

/** Summarize a batch report into a one-line human note. */
function summary(report: BatchReport): string {
  let added = 0
  let merged = 0
  let dup = 0
  let notFound = 0
  let failed = 0
  for (const r of report.results) {
    switch (r.dedup_outcome) {
      case 'inserted':
        added++
        break
      case 'linked_existing':
        merged++
        break
      case 'possible_duplicate':
        dup++
        break
      case 'failed':
        failed++
        break
    }
    if (r.verification_status === 'not_found') notFound++
  }
  return `${added} added · ${merged} already present · ${dup} possible duplicate · ${notFound} not found · ${failed} failed`
}

export function ImportCitationsModal({
  open,
  onClose,
  projectId,
}: {
  open: boolean
  onClose: () => void
  projectId?: string | null
}) {
  const [text, setText] = useState('')
  const [busy, setBusy] = useState(false)
  const [result, setResult] = useState<BatchReport | null>(null)

  const handleImport = async () => {
    const items: CitationInput[] = text
      .split('\n')
      .map(l => l.trim())
      .filter(Boolean)
      .map(line => ({ id: line }))
    if (items.length === 0) return
    setBusy(true)
    try {
      const report = await Stores.Citations.importItems(items, projectId ?? null)
      setResult(report)
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Import failed')
    } finally {
      setBusy(false)
    }
  }

  return (
    <Dialog
      open={open}
      title="Import citations"
      onOpenChange={(v) => {
        if (!v) {
          setResult(null)
          setText('')
          onClose()
        }
      }}
      footer={
        <>
          <button type="button" onClick={() => {
            setResult(null)
            setText('')
            onClose()
          }}>Cancel</button>
          <button type="button" disabled={busy} onClick={handleImport}>Import + verify</button>
        </>
      }
    >
      <Paragraph type="secondary">
        Paste DOIs, PMIDs, arXiv IDs, or titles — one per line. Each is resolved
        to a real record and verified; fabricated identifiers are reported as
        <Text strong> not found</Text> and not stored.
      </Paragraph>
      <textarea
        rows={6}
        value={text}
        onChange={(e: React.ChangeEvent<HTMLTextAreaElement>) => setText(e.target.value)}
        placeholder={'10.1038/s41586-021-...\n34121113\n2101.12345'}
        className="w-full rounded-md border border-input bg-background px-3 py-2 text-sm ring-offset-background placeholder:text-muted-foreground focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:cursor-not-allowed disabled:opacity-50"
      />
      {result && (
        <Alert
          className="mt-3"
          tone="info"
          title="Import result"
          description={summary(result)}
        />
      )}
    </Dialog>
  )
}
