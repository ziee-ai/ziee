import type { UserAssistantsGet, UserAssistantsSet } from '../state'
import doLoadFactory from './_doLoad'

export default (set: UserAssistantsSet, get: UserAssistantsGet) => {
  const doLoad = doLoadFactory(set, get)
  return async (page?: number, pageSize?: number) => {
    const currentState = get()
    const targetPage = page ?? currentState.currentPage
    const targetPageSize = pageSize ?? currentState.pageSize
    await doLoad(targetPage, targetPageSize)
  }
}
