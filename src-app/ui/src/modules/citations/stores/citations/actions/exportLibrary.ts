import { ApiClient } from '@/api-client'
import type { CitationsGet, CitationsSet } from '../state'

export default (_set: CitationsSet, get: CitationsGet) => {
  return async (format: string, style?: string, projectId?: string | null) => {
    const pid = projectId !== undefined ? projectId : get().projectId
    const resp = await ApiClient.Citations.export({
      format,
      ...(style ? { style } : {}),
      ...(pid ? { project_id: pid } : {}),
    })
    return resp.output
  }
}
