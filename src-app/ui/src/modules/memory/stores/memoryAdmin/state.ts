import type { StoreSet } from '@ziee/framework/store-kit'
import type {
  FtsRebuildStatus,
  LlmModel,
  MemoryAdminSettings,
  RebuildStatus,
  UpdateMemoryAdminSettingsRequest,
} from '@/api-client/types'

// Candidate model row for the admin form's model pickers. Carries
// `capabilities` so the form can derive the extraction list client-side.
export type CandidateModelRow = Pick<
  LlmModel,
  'id' | 'name' | 'display_name' | 'provider_id' | 'capabilities'
>

// Widened patch type. The backend uses `Option<Option<T>>` for the model id +
// prompt fields — tri-state (absent = leave, null = clear, value = set). The TS
// codegen strips `null`; widen at the boundary so callers can clear.
export type MemoryAdminUpdatePatch = Omit<
  UpdateMemoryAdminSettingsRequest,
  'embedding_model_id' | 'default_extraction_model_id'
> & {
  embedding_model_id?: string | null
  default_extraction_model_id?: string | null
}

export const memoryAdminState = {
  settings: null as MemoryAdminSettings | null,
  // All models (capped page), used to derive the extraction-model list.
  availableModels: [] as CandidateModelRow[],
  // Embedding-capable models, server-filtered so the embedding picker isn't
  // truncated by unrelated chat models.
  embeddingModels: [] as CandidateModelRow[],
  rebuildStatus: null as RebuildStatus | null,
  ftsRebuildStatus: null as FtsRebuildStatus | null,
  loading: false,
  saving: false,
  loadingModels: false,
  triggeringReembed: false,
  triggeringFtsRebuild: false,
  error: null as string | null,
}

export type MemoryAdminState = typeof memoryAdminState
export type MemoryAdminSet = StoreSet<MemoryAdminState>
export type MemoryAdminGet = () => MemoryAdminState
