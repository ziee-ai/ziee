import type { StoreSet } from '@ziee/framework/store-kit'
import type {
  SummarizationAdminSettings,
  UpdateSummarizationAdminSettingsRequest,
} from '@/api-client/types'

export type SummarizationModelRow = {
  id: string
  name: string
  display_name: string
  provider_id: string
}

// Widened patch type. The backend uses `Option<Option<T>>` for the model id +
// prompt fields — tri-state (absent = leave, null = clear, value = set). The TS
// codegen strips `null`; widen at the boundary so callers can clear.
export type SummarizationAdminUpdatePatch = Omit<
  UpdateSummarizationAdminSettingsRequest,
  'default_summarization_model_id' | 'full_summary_prompt' | 'incremental_summary_prompt'
> & {
  default_summarization_model_id?: string | null
  full_summary_prompt?: string | null
  incremental_summary_prompt?: string | null
}

export const summarizationAdminState = {
  settings: null as SummarizationAdminSettings | null,
  availableModels: [] as SummarizationModelRow[],
  loading: false,
  saving: false,
  loadingModels: false,
  error: null as string | null,
}

export type SummarizationAdminState = typeof summarizationAdminState
export type SummarizationAdminSet = StoreSet<SummarizationAdminState>
export type SummarizationAdminGet = () => SummarizationAdminState
