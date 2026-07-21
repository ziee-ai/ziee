import type { StoreSet } from '@ziee/framework/store-kit'
import type {
  KnowledgeBase,
  KnowledgeBaseDocument,
  KnowledgeBaseSearchResponse,
  KnowledgeBaseUsage,
  RetrievalInfo,
} from '@/api-client/types'

// Re-export so consumers of this module get the type without importing File.store.
export type FileUploadProgress = import('@/modules/file/stores/file').FileUploadProgress

/** Default documents-per-page. Numbered pagination (discrete pages via
 *  `ListPagination`, like the users/memories settings pages) — NOT infinite
 *  scroll — so only a small page loads at a time (a KB holds up to 2000). */
const DOC_DEFAULT_PAGE_SIZE = 10

export const knowledgeBaseDetailState = {
  kb: null as KnowledgeBase | null,
  documents: [] as KnowledgeBaseDocument[],
  loading: false,
  documentsLoading: false,
  /** 1-based current page + page size for the documents `ListPagination`. */
  documentsPage: 1,
  documentsPageSize: DOC_DEFAULT_PAGE_SIZE,
  uploading: false,
  /** Per-file upload progress, keyed by a synthetic local id — mirrors
   *  ProjectFiles so each uploading file shows its own FileCard progress row. */
  uploadingFiles: new Map<string, FileUploadProgress>(),
  /** Multi-select for bulk remove (mirrors ProjectFiles). */
  selectedFileIds: new Set<string>(),
  error: null as string | null,
  /** Deployment retrieval mode (for the detail-page mode line). */
  retrievalInfo: null as RetrievalInfo | null,
  /** Conversations + projects this KB is attached to ("Used in"). */
  usage: null as KnowledgeBaseUsage | null,
  /** Direct "test retrieval" search box state. */
  searching: false,
  searchResults: null as KnowledgeBaseSearchResponse | null,
}

export type KnowledgeBaseDetailState = typeof knowledgeBaseDetailState
export type KnowledgeBaseDetailSet = StoreSet<KnowledgeBaseDetailState>
export type KnowledgeBaseDetailGet = () => KnowledgeBaseDetailState
