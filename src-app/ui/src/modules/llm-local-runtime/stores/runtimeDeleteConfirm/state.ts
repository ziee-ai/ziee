import type { RuntimeVersionResponse } from '@/api-client/types'
import type { StoreSet } from '@ziee/framework/store-kit'

export const runtimeDeleteConfirmState = {
  version: null as RuntimeVersionResponse | null,
}

export type RuntimeDeleteConfirmState = typeof runtimeDeleteConfirmState
export type RuntimeDeleteConfirmSet = StoreSet<RuntimeDeleteConfirmState>
export type RuntimeDeleteConfirmGet = () => RuntimeDeleteConfirmState
