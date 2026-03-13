import type { RuntimePlatform, RuntimeArch, RuntimeBackend } from '../types'

export function detectPlatform(): RuntimePlatform {
  const platform = window.navigator.platform.toLowerCase()
  if (platform.includes('mac')) return 'macos'
  if (platform.includes('win')) return 'windows'
  return 'linux'
}

export function detectArch(): RuntimeArch {
  // Modern browsers expose this via userAgentData
  // Fallback to x86_64 for compatibility
  return 'x86_64'
}

export function getDefaultBackend(platform: RuntimePlatform): RuntimeBackend {
  if (platform === 'macos') return 'metal'
  // Could detect CUDA availability, but default to CPU for safety
  return 'cpu'
}
