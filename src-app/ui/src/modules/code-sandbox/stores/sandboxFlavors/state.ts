import type { StoreSet } from '@ziee/framework/store-kit'
import type { EnvironmentInfo } from '@/api-client/types'

/**
 * Canonical fallback labels used when the load call fails or the user lacks
 * `code_sandbox::environments::read` permission — the saved value is validated
 * server-side against KNOWN_FLAVORS anyway.
 */
export const FALLBACK_OPTIONS = [
  { label: 'full', value: 'full' },
  { label: 'minimal', value: 'minimal' },
]

export const FALLBACK_HOST_COMMANDS = ['npx', 'uvx', 'python', 'python3', 'node']

export function toOptions(flavors: EnvironmentInfo[]): { label: string; value: string }[] {
  return flavors.map(e => ({ label: `${e.flavor} — ${e.description}`, value: e.flavor }))
}

export const sandboxFlavorsState = {
  flavors: [] as EnvironmentInfo[],
  selectOptions: [] as { label: string; value: string }[],
  hostCommands: [] as string[],
  loading: false,
  error: null as string | null,
  isInitialized: false,
}

export type SandboxFlavorsState = typeof sandboxFlavorsState
export type SandboxFlavorsSet = StoreSet<SandboxFlavorsState>
export type SandboxFlavorsGet = () => SandboxFlavorsState
