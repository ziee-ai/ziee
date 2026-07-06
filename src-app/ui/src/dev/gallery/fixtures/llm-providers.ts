/**
 * LLM-providers fixture — recorded `GET /api/llm-providers` +
 * `GET /api/llm-models?providerId=` for the settings page.
 *
 * Recorded from a real server by `scripts/record-gallery-fixtures.mjs` (which
 * enables a few providers + creates models through the real endpoints), then
 * typed against the generated response types so any drift fails `tsc`.
 */
import type {
  Group,
  GroupListResponse,
  LlmModelListResponse,
  LlmProviderListResponse,
} from '@/api-client/types'
import type { Cassette } from '../mockApi'
import recorded from './recorded/llm-providers.json'

interface LlmProvidersFixture {
  providers: LlmProviderListResponse
  modelsByProvider: Record<string, LlmModelListResponse>
  groups: GroupListResponse
  groupsByProvider: Record<string, Group[]>
}

// Typed against the generated response types (layer-1 correctness).
const fixture: LlmProvidersFixture = recorded as LlmProvidersFixture

export const llmProvidersList = fixture.providers
export const llmModelsByProvider = fixture.modelsByProvider

/** First enabled remote (non-local) provider — the populated view to show. */
export const firstEnabledRemoteProviderId: string | undefined =
  llmProvidersList.providers.find(
    p => p.enabled && p.provider_type !== 'local',
  )?.id

export const llmGroupsList = fixture.groups
export const llmGroupsByProvider = fixture.groupsByProvider

const emptyModels: LlmModelListResponse = {
  models: [],
  page: 1,
  per_page: 100,
  total: 0,
}

export const llmProvidersCassette: Cassette = {
  'LlmProvider.list': llmProvidersList,
  // `LlmModel.list` is keyed by the `?providerId=` query param.
  'LlmModel.list': ({ query }) =>
    llmModelsByProvider[query.providerId] ?? emptyModels,
  // The provider settings page also renders a group-assignment card.
  'UserGroup.list': llmGroupsList,
  'LlmProvider.getGroups': ({ params }) =>
    llmGroupsByProvider[params.provider_id] ?? [],
}
