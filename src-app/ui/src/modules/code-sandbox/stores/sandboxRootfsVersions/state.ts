import type { StoreSet } from '@ziee/framework/store-kit'
import type { DrainEntry, InstallTaskState, RootfsArtifact, RootfsRelease, SandboxAvailability, SwapOutcome } from '@/api-client/types'

/** Per-(version, arch, flavor, package) action state — drives the install /
 *  set-pin / delete buttons' loading flags. */
export interface ActionState {
  installing?: boolean
  pinning?: boolean
  deleting?: boolean
}

export const sandboxRootfsVersionsState = {
  pinnedVersion: null as string | null,
  installed: [] as RootfsArtifact[],
  /** Releases on GitHub (catalog). Empty if GitHub was unreachable. */
  available: [] as RootfsRelease[],
  /** Live mounts the server registered — keyed by artifact_id. */
  draining: [] as DrainEntry[],
  /** Count of per-conversation workspace dirs. */
  conversationCount: 0,
  /** Count of per-MCP-server workspace dirs. */
  mcpServerWorkspaceCount: 0,
  /** Server-authoritative host CPU arch + rootfs package format. */
  hostArch: null as string | null,
  hostPackage: null as string | null,
  /** Whether code_sandbox is initialized, else the machine-readable reason.
   * When not `'ready'` the LIST endpoint still returns 200 with the GitHub
   * catalog (installed/pinned empty) — the section renders a graceful notice
   * instead of a destructive error. Defaults to `'ready'` so the working UI is
   * unchanged until a degraded response arrives. */
  availability: 'ready' as SandboxAvailability,
  /** Outcome of the last set-pin call. */
  lastSwap: null as SwapOutcome | null,
  loading: false,
  /** Data-load failure (the rootfs-status GET). Rendered as a destructive
   * ErrorState. NEVER holds SSE/transport state — that lives in `sseError`. */
  error: null as string | null,
  /** Live-progress SSE transport state (disconnect/reconnect/permanent
   * failure). Kept OUT of `error` so a background reconnect blip never
   * surfaces as a raw destructive "SSE disconnected…" string in user copy. */
  sseError: null as string | null,
  actions: {} as Record<string, ActionState>,
  /** Live install task state, keyed by `<version>::<arch>::<flavor>::<package>`. */
  installTasks: {} as Record<string, InstallTaskState>,
  /** True once the SSE subscription emitted its `connected` event. */
  sseConnected: false,
}

export type SandboxRootfsVersionsState = typeof sandboxRootfsVersionsState
export type SandboxRootfsVersionsSet = StoreSet<SandboxRootfsVersionsState>
export type SandboxRootfsVersionsGet = () => SandboxRootfsVersionsState
