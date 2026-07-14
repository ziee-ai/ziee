import { Trash2 } from 'lucide-react'
import { Button, Card, Confirm, Space, Text, Paragraph, message } from '@ziee/kit'
import type { BibliographyEntry } from '@/api-client/types'
import { Stores } from '@ziee/framework/stores'
import { VerificationBadge } from './VerificationBadge'

/** Pull a compact author list out of the entry's CSL-JSON. */
function authorLine(csl: unknown): string {
  if (!csl || typeof csl !== 'object') return ''
  const authors = (csl as Record<string, unknown>).author
  if (!Array.isArray(authors)) return ''
  const names = authors
    .map(a => {
      if (!a || typeof a !== 'object') return ''
      const o = a as Record<string, unknown>
      const family = typeof o.family === 'string' ? o.family : ''
      const given = typeof o.given === 'string' ? o.given : ''
      const literal = typeof o.literal === 'string' ? o.literal : ''
      return family ? `${family}${given ? ` ${given[0]}` : ''}` : literal
    })
    .filter(Boolean)
  if (names.length === 0) return ''
  return names.length > 4 ? `${names.slice(0, 4).join(', ')}, et al.` : names.join(', ')
}

export function CitationCard({
  entry,
  canManage,
}: {
  entry: BibliographyEntry
  // Required (no default) so a caller can't accidentally fail-OPEN and render
  // an ungated Delete; both call sites pass the resolved permission.
  canManage: boolean
}) {
  const handleDelete = async () => {
    try {
      await Stores.Citations.remove(entry.id)
    } catch (e) {
      message.error(e instanceof Error ? e.message : 'Delete failed')
    }
  }
  const authors = authorLine(entry.csl_json)
  const meta = [authors, entry.year ? String(entry.year) : '']
    .filter(Boolean)
    .join(' · ')

  return (
    <Card size="sm" className="mb-2" data-testid={`cite-card-${entry.id}`}>
      <Space direction="vertical" size={2} className="w-full">
        {/* Row stays on ONE line (no wrap) so the Delete button never orphans
            onto its own line under a long key on a narrow (mobile) viewport —
            the key group shrinks/wraps internally, Delete stays anchored right. */}
        <div className="flex items-center justify-between gap-2 w-full">
          {/* Key-then-badge reading order (Spec C): the citation key is the
              identity the user scans for, the verification badge qualifies it —
              e.g. "vaswani2017attention (verified)". Badge tones stay in the
              success / danger / muted family (see VerificationBadge). */}
          <Space size={8} className="min-w-0">
            <Text
              code
              ellipsis
              copyable={{
                text: entry.citation_key,
                label: 'Copy citation key',
              }}
            >
              {entry.citation_key}
            </Text>
            <VerificationBadge status={entry.verification_status} />
          </Space>
          {canManage && (
            <div className="shrink-0">
              <Confirm
                title="Delete from library?"
                description="Removes it from the library and every project."
                okButtonProps={{ danger: true }}
                onConfirm={handleDelete}
                okText="OK"
                cancelText="Cancel"
                data-testid={`cite-card-delete-confirm-${entry.id}`}
              >
                <Button
                  size="default"
                  variant="outline"
                  type="button"
                  aria-label={`Delete ${entry.citation_key}`}
                  icon={<Trash2 />}
                  data-testid={`cite-card-delete-button-${entry.id}`}
                />
              </Confirm>
            </div>
          )}
        </div>
        <Text strong className="[overflow-wrap:anywhere]">{entry.title || '(untitled)'}</Text>
        {meta && <Text type="secondary">{meta}</Text>}
        {entry.doi && (
          <Paragraph className="m-0">
            {/* break-all so a long DOI wraps inside the card instead of
                overflowing its right edge on a narrow (mobile) viewport —
                DOIs are unbroken `/`- and `.`-joined strings with no natural
                wrap opportunities. */}
            <a
              href={`https://doi.org/${entry.doi}`}
              target="_blank"
              rel="noreferrer"
              className="break-all"
            >
              doi:{entry.doi}
            </a>
          </Paragraph>
        )}
      </Space>
    </Card>
  )
}
