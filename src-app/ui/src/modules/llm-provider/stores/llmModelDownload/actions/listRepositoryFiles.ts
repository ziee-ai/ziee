import { ApiClient } from '@/api-client'
import type { RepositoryFileListResponse } from '@/api-client/types'

export default (_set: () => void, _get: () => void) =>
  async (
    repositoryId: string,
    path: string,
    branch?: string,
  ): Promise<RepositoryFileListResponse> => {
    return await ApiClient.LlmModel.listRepositoryFiles({
      repository_id: repositoryId,
      path,
      branch: branch || 'main',
    })
  }
