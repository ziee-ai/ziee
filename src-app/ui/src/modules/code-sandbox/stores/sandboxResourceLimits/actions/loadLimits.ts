import type { SandboxResourceLimitsGet, SandboxResourceLimitsSet } from '../state'
import loadLimitsFactory from './_loadLimits'

export default (set: SandboxResourceLimitsSet, get: SandboxResourceLimitsGet) => {
  const _loadLimits = loadLimitsFactory(set, get)
  return async () => _loadLimits()
}
