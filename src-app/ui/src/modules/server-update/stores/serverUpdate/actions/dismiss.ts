import type { ServerUpdateGet, ServerUpdateSet } from '../state'

export default (set: ServerUpdateSet, get: ServerUpdateGet) =>
  async () => {
    const v = get().latestVersion
    if (v) localStorage.setItem('ziee:server-update:dismissed-version', v)
    set(st => {
      st.dismissed = true
    })
  }
