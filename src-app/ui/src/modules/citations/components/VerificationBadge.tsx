import { Tag, Tooltip } from 'antd'
import type { BibliographyEntry } from '@/api-client/types'

/** The visual heart of "never invent a citation": colour-coded resolution state. */
export function VerificationBadge({
  status,
}: {
  status: BibliographyEntry['verification_status']
}) {
  switch (status) {
    case 'verified':
      return <Tag color="green">verified</Tag>
    case 'mismatch':
      return (
        <Tooltip title="The identifier resolves, but to a record whose title differs from what was supplied — review it.">
          <Tag color="orange">mismatch</Tag>
        </Tooltip>
      )
    case 'not_found':
      return (
        <Tooltip title="The supplied DOI/PMID did NOT resolve to a real record — likely fabricated.">
          <Tag color="red">not found</Tag>
        </Tooltip>
      )
    default:
      return (
        <Tooltip title="No identifier to verify against (e.g. a book / thesis / dataset). Not a problem — just unverified.">
          <Tag>unverified</Tag>
        </Tooltip>
      )
  }
}
