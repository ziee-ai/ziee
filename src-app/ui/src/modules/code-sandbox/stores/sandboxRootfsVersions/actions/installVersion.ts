import { ApiClient } from '@/api-client'
import type { ActionState, SandboxRootfsVersionsGet, SandboxRootfsVersionsSet } from '../state'
import { reconcileInitialTask } from '../../installTaskReconcile'

function setAction(s: { actions: Record<string, ActionState> }, key: string, patch: ActionState) {
  const cur = s.actions[key] ?? {}
  s.actions[key] = { ...cur, ...patch }
}

function clearAction(s: { actions: Record<string, ActionState> }, key: string) {
  delete s.actions[key]
}

function rowKey(version: string, arch: string, flavor: string, pkg: string): string {
  return `${version}::${arch}::${flavor}::${pkg}`
}

export default (set: SandboxRootfsVersionsSet, _get: SandboxRootfsVersionsGet) => {
  return async (version: string, arch: string, flavor: string, pkg: string) => {
    const key = rowKey(version, arch, flavor, pkg)
    set(s => {
      setAction(s, key, { installing: true })
      s.error = null
    })
    try {
      // 202 Accepted — server returns InstallTaskState immediately; live
      // progress streams through the SSE subscription.
      const initial = await ApiClient.CodeSandbox.installRootfsVersion({
        version,
        arch,
        flavor,
        package: pkg,
      })
      set(s => {
        // Race guard: the SSE `taskStarted`/`progress` events (same task_id)
        // may already have created + advanced this task while this POST was
        // in flight. Keep the SSE-tracked task if present so a late reply
        // (phase: null) can't clobber an in-flight download back to "queued".
        s.installTasks[key] = reconcileInitialTask(s.installTasks[key], initial)
      })
    } catch (e: any) {
      set(s => {
        s.error = e?.message ?? `Failed to install ${version}`
        clearAction(s, key)
      })
    }
  }
}
