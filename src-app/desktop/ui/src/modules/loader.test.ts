/**
 * Guards the desktop bundle's core-module blocklist.
 *
 * `server-update` MUST stay blocklisted: the desktop has its own auto-updater
 * (tauri-plugin-updater) and its own /settings/about page, so loading the web
 * server-update module would surface a duplicate update banner + collide on the
 * route. See modules/updater/* and backend/mod.rs (update_check force-off).
 */

import { describe, expect, it } from 'vitest'
import { CORE_MODULE_BLOCKLIST, isBlocklisted, applyBlocklist } from '@/modules/loader.desktop'

describe('desktop CORE_MODULE_BLOCKLIST', () => {
  it('blocklists the web server-update + user-profile modules', () => {
    expect(CORE_MODULE_BLOCKLIST.has('server-update')).toBe(true)
    expect(isBlocklisted('server-update')).toBe(true)
    expect(isBlocklisted('user-profile')).toBe(true)
  })

  it('applyBlocklist actually drops server-update from a module list', () => {
    const mods = [
      { metadata: { name: 'server-update' } },
      { metadata: { name: 'chat' } },
      { metadata: { name: 'user-profile' } },
      { metadata: { name: 'settings' } },
    ]
    const kept = applyBlocklist(mods).map((m) => m.metadata.name)
    expect(kept).toEqual(['chat', 'settings'])
    expect(kept).not.toContain('server-update')
  })
})
