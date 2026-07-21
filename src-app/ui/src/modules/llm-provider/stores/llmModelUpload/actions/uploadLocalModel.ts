import type { ModelCapabilities, ModelEngineSettings } from '@/api-client/types'
import type { LlmModelGet, LlmModelSet } from '../state'
import uploadLocalModelFactory from './_uploadLocalModel'

export default (set: LlmModelSet, get: LlmModelGet) => {
  const uploadLocalModel = uploadLocalModelFactory(set, get)
  return async (
    providerId: string,
    name: string,
    displayName: string,
    description: string | undefined,
    mainFilename: string,
    fileFormat: string,
    capabilities: ModelCapabilities | undefined,
    engineType: string | undefined,
    engineSettings: ModelEngineSettings | undefined,
    files: File[],
  ) => uploadLocalModel(providerId, name, displayName, description, mainFilename, fileFormat, capabilities, engineType, engineSettings, files)
}
