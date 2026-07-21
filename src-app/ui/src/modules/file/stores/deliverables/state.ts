import type { StoreSet } from '@ziee/framework/store-kit'
import type { File as FileEntity } from '@/api-client/types'

export const deliverablesState = {
  byConversation: new Map<string, FileEntity[]>(),
  loadingSet: new Set<string>(),
}

export type DeliverablesState = typeof deliverablesState
export type DeliverablesSet = StoreSet<DeliverablesState>
export type DeliverablesGet = () => DeliverablesState
