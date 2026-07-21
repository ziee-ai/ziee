import type { HardwareGet, HardwareSet } from '../state'
import sseConnectFactory from './_sseConnect'

export default (set: HardwareSet, get: HardwareGet) => {
  const sseConnect = sseConnectFactory(set, get)
  return async () => sseConnect()
}
