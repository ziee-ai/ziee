import { Stores } from '@/core/stores'
import type { RuntimeVersionResponse } from '@/api-client/types'

export async function emitRuntimeVersionCreated(version: RuntimeVersionResponse) {
  await Stores.EventBus.emit({
    type: 'runtime_version.created',
    data: { version }
  })
}

export async function emitRuntimeVersionDeleted(versionId: string) {
  await Stores.EventBus.emit({
    type: 'runtime_version.deleted',
    data: { versionId }
  })
}

export async function emitRuntimeVersionDefaultChanged(versionId: string) {
  await Stores.EventBus.emit({
    type: 'runtime_version.default_changed',
    data: { versionId }
  })
}
