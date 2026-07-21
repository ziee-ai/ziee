import type { RuntimeVersionResponse } from '@/api-client/types'
import { EventBus } from '@ziee/framework/stores'

export async function emitRuntimeVersionCreated(version: RuntimeVersionResponse) {
  await EventBus.emit({
    type: 'runtime_version.created',
    data: { version }
  })
}

export async function emitRuntimeVersionDeleted(versionId: string) {
  await EventBus.emit({
    type: 'runtime_version.deleted',
    data: { versionId }
  })
}

export async function emitRuntimeVersionDefaultChanged(versionId: string) {
  await EventBus.emit({
    type: 'runtime_version.default_changed',
    data: { versionId }
  })
}

export async function emitRuntimeModelUsageChanged(modelId: string) {
  await EventBus.emit({
    type: 'runtime_version.usage_changed',
    data: { modelId }
  })
}
