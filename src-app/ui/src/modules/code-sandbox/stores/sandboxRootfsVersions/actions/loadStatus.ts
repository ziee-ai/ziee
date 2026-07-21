import { ApiClient } from '@/api-client'
import type { SandboxRootfsVersionsGet, SandboxRootfsVersionsSet } from '../state'

function rowKey(version: string, arch: string, flavor: string, pkg: string): string {
  return `${version}::${arch}::${flavor}::${pkg}`
}

export default (set: SandboxRootfsVersionsSet, _get: SandboxRootfsVersionsGet) => {
  return async (opts?: { pruneFailed?: boolean }) => {
    set(s => {
      s.loading = true
      s.error = null
    })
    try {
      const res = await ApiClient.CodeSandbox.listRootfsVersions(undefined)
      set(s => {
        s.pinnedVersion = res.pinned_version ?? null
        s.installed = res.installed
        s.available = res.available
        s.draining = res.draining
        s.conversationCount = res.conversation_count
        s.mcpServerWorkspaceCount = res.mcp_server_workspace_count
        s.hostArch = res.host_arch ?? null
        s.hostPackage = res.host_package ?? null
        s.availability = res.availability
        // Prune any install task whose artifact has landed — keeps installTasks
        // bounded while letting the SSE `complete` handler hold a completed
        // task until this reload arrives (aggregate bar stays monotonic).
        for (const a of res.installed) {
          delete s.installTasks[rowKey(a.version, a.arch, a.flavor, a.package)]
        }
        // Clear terminal-failed tasks ONLY on explicit Refresh/mount — NOT on
        // the SSE `complete` auto-reload, or a sibling flavor's success would
        // erase a still-failed flavor's red bar.
        if (opts?.pruneFailed) {
          for (const key of Object.keys(s.installTasks)) {
            if (s.installTasks[key].status === 'failed') delete s.installTasks[key]
          }
        }
        s.loading = false
      })
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? 'Failed to load rootfs versions'
        s.loading = false
      })
    }
  }
}
