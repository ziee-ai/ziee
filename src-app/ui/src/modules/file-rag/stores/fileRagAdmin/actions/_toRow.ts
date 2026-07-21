import type { LlmModel } from '@/api-client/types'
import type { CandidateModelRow } from '../state'

/** Pure helper: cast an LlmModel API row into a CandidateModelRow for the picker. */
export default function toRow(m: LlmModel): CandidateModelRow {
  return m as CandidateModelRow
}
