import type { StoreSet } from '@ziee/framework/store-kit'
import type {
  FileRagAdminSettings,
  LlmModel,
  UpdateFileRagAdminSettingsRequest,
} from '@/api-client/types'

/** Candidate embedding-model row for the picker. */
export type CandidateModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id' | 'capabilities'
>

// Tri-state `embedding_model_id` (`Option<Option<Uuid>>`) — codegen drops the
// `null` arm; widen at the store boundary so callers can clear.
export type FileRagAdminUpdatePatch = Omit<
  UpdateFileRagAdminSettingsRequest,
  'embedding_model_id' | 'reranker_model_id'
> & {
  embedding_model_id?: string | null
  reranker_model_id?: string | null
}

export const fileRagAdminState = {
  settings: null as FileRagAdminSettings | null,
  embeddingModels: [] as CandidateModelRow[],
  rerankerModels: [] as CandidateModelRow[],
  loading: false,
  saving: false,
  loadingModels: false,
  triggeringReembed: false,
  triggeringBackfill: false,
  error: null as string | null,
}

export type FileRagAdminState = typeof fileRagAdminState
export type FileRagAdminSet = StoreSet<FileRagAdminState>
export type FileRagAdminGet = () => FileRagAdminState
