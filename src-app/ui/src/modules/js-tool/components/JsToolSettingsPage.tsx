import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { JsToolSettingsSection } from './JsToolSettingsSection'

/**
 * Programmatic Tools (run_js / js_tool) admin settings page — the runtime
 * resource caps applied to every run_js invocation. A single settings card
 * (mirrors the Code Sandbox "Resource limits" page, minus the rootfs tab).
 */
export function JsToolSettingsPage() {
  return (
    <SettingsPageContainer
      title="Programmatic Tools"
      subtitle="Runtime resource caps for the built-in run_js tool — memory, stack, timeouts, and concurrency."
    >
      <JsToolSettingsSection />
    </SettingsPageContainer>
  )
}
