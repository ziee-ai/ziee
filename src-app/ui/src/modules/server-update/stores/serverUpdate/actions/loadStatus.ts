import { ApiClient } from '@/api-client'
import type { ServerUpdateGet, ServerUpdateSet } from '../state'

const DISMISSED_VERSION_KEY = 'ziee:server-update:dismissed-version'

export default (set: ServerUpdateSet, _get: ServerUpdateGet) =>
  async () => {
    set(st => {
      st.loading = true
      st.error = null
    })
    try {
      const s = await ApiClient.ServerUpdate.getStatus(undefined, undefined)
      set(st => {
        st.currentVersion = s.current_version
        st.latestVersion = s.latest_version ?? null
        st.updateAvailable = s.update_available
        st.releaseUrl = s.release_url ?? null
        st.notes = s.notes ?? null
        st.enabled = s.enabled
        st.checkedAt = s.checked_at ?? null
        const dismissedVersion = localStorage.getItem(DISMISSED_VERSION_KEY)
        st.dismissed =
          s.update_available && !!dismissedVersion && dismissedVersion === (s.latest_version ?? null)
        st.loading = false
      })
    } catch (e) {
      set(st => {
        st.loading = false
        st.error = e instanceof Error ? e.message : 'Failed to load update status'
      })
    }
  }
