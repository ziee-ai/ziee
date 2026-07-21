import type { LitSearchAdminGet, LitSearchAdminSet } from '../state'
import doSettingsFactory from './doSettings'
import doConnectorsFactory from './doConnectors'

export default (set: LitSearchAdminSet, get: LitSearchAdminGet) => {
  const doSettings = doSettingsFactory(set, get)
  const doConnectors = doConnectorsFactory(set, get)
  return async () => {
    await Promise.all([doSettings(), doConnectors()])
  }
}
