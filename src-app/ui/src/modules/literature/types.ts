import type { StoreProxy } from '@/core/stores'
import type { useLitSearchAdminStore } from './stores/LitSearchAdmin.store'

declare module '@/core/stores' {
  interface RegisteredStores {
    LitSearchAdmin: StoreProxy<ReturnType<typeof useLitSearchAdminStore.getState>>
  }
}

/** Shape of a `literature_search` result's structuredContent. Mirrors the Rust
 *  `AggregateResult` / `LitRecord` — modeled locally because it's MCP tool
 *  output, not a generated REST type. */
export interface LiteratureRecord {
  doi?: string | null
  pmid?: string | null
  title: string
  abstract_text?: string | null
  authors: string[]
  year?: number | null
  venue?: string | null
  url?: string | null
  source: string
  source_ids: string[]
  cited_by_count?: number | null
  is_preprint: boolean
  relevance: number
}

export interface LiteratureCompleteness {
  estimate: string
  method: string
  caveat: string
}

export interface LiteratureResult {
  query: string
  records: LiteratureRecord[]
  identified: Record<string, number>
  after_dedup: number
  degraded_sources: string[]
  completeness?: LiteratureCompleteness | null
}

export type ScreeningDecision = 'include' | 'exclude' | 'unscreened'

/** Serializable right-panel tab data for one screening session (persisted to
 *  localStorage via the panel snapshot, so decisions survive reload). */
export interface LiteratureScreeningData {
  /** The right-panel tab id (== the tab's `id`), so the panel can persist its
   *  own evolving decisions via `updateRightPanelTab`. */
  sessionId: string
  query: string
  records: LiteratureRecord[]
  identified: Record<string, number>
  afterDedup: number
  degradedSources: string[]
  completeness?: LiteratureCompleteness | null
  /** record key (DOI or title/year fallback) → decision */
  decisions: Record<string, ScreeningDecision>
  /** record key → optional exclusion reason */
  reasons: Record<string, string>
}

declare module '@/modules/chat/core/stores/Chat.store' {
  interface PanelRendererMap {
    literature: LiteratureScreeningData
  }
}

/** Stable per-record key for decisions: normalized DOI, else pmid, else a
 *  title/year fingerprint (matches the backend dedup intent). */
export function recordKey(r: LiteratureRecord): string {
  if (r.doi) return `doi:${r.doi.toLowerCase()}`
  if (r.pmid) return `pmid:${r.pmid}`
  const t = r.title.toLowerCase().replace(/[^a-z0-9]+/g, ' ').trim()
  return `ty:${t}|${r.year ?? ''}`
}
