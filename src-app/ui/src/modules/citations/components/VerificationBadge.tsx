import { CircleCheck, CircleX, TriangleAlert } from 'lucide-react'
import { Tag, Tooltip } from '@ziee/kit'
import type { BibliographyEntry } from '@/api-client/types'

/**
 * The visual heart of "never invent a citation": colour-coded resolution state.
 * The three identifier-backed states carry a status glyph so they're
 * distinguishable at a glance (and for colour-blind users) — in particular
 * `mismatch` (amber + ⚠) reads as an actionable warning, not to be confused with
 * `unverified` (plain muted, no icon), which is a legitimate "nothing to verify".
 */
export function VerificationBadge({
  status,
}: {
  status: BibliographyEntry['verification_status']
}) {
  switch (status) {
    case 'verified':
      return <Tag variant="outline" tone="success" icon={<CircleCheck />} data-testid="cite-badge-verified">verified</Tag>
    case 'mismatch':
      return (
        <Tooltip title="The identifier resolves, but to a record whose title differs from what was supplied — review it.">
          <Tag variant="outline" tone="warning" icon={<TriangleAlert />} data-testid="cite-badge-mismatch">mismatch</Tag>
        </Tooltip>
      )
    case 'not_found':
      return (
        <Tooltip title="The supplied DOI/PMID did NOT resolve to a real record — likely fabricated.">
          <Tag variant="outline" tone="error" icon={<CircleX />} data-testid="cite-badge-not-found">not found</Tag>
        </Tooltip>
      )
    default:
      return (
        <Tooltip title="No identifier to verify against (e.g. a book / thesis / dataset). Not a problem — just unverified.">
          <Tag variant="outline" data-testid="cite-badge-unverified">unverified</Tag>
        </Tooltip>
      )
  }
}
