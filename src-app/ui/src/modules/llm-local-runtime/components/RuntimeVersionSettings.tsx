import { Flex, Tabs } from 'antd'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { InstalledVersionsCard } from './InstalledVersionsCard'
import { AvailableVersionsCard } from './AvailableVersionsCard'
import { RuntimeDownloadDrawer } from './drawers/RuntimeDownloadDrawer'
import { RuntimeConfigCard } from './RuntimeConfigCard'
import type { RuntimeEngine } from '../types'

// Per-engine sections. Each engine tab stacks two cards:
//   1. Installed versions  — list of registered binaries; each row
//                            shows the models that resolve to that
//                            version inlined directly underneath
//                            (start/stop/restart/swap + Logs), so
//                            an operator sees "v0.0.1 — 3 models
//                            pin it, 1 is running" in one place.
//                            The unresolved-models warning appears
//                            as a footer block when applicable.
//   2. Available versions  — upstream catalog (with Check-for-updates
//                            in the card's `extra` slot, mirroring
//                            UsersSettings's `+` create-button
//                            convention), plus the host platform /
//                            available-backends context strip.
//
// Both read `llm_local_runtime::versions_read`. The page route only
// requires `llm_local_runtime::read`, so they're wrapped in <Can>
// here to avoid empty/403 cards for read-only principals without
// versions_read.
function VersionSections({ engine }: { engine: RuntimeEngine }) {
  return (
    <Flex className="flex-col gap-3">
      <Can permission={Permissions.RuntimeVersionRead}>
        <InstalledVersionsCard engine={engine} />
        <AvailableVersionsCard engine={engine} />
      </Can>
    </Flex>
  )
}

export function RuntimeVersionSettings() {
  return (
    <>
      <SettingsPageContainer
        title="Local Runtimes"
        subtitle="Engine binary versions, available downloads, and runtime configuration for local model inference"
      >
        <Flex className="flex-col gap-3">
          <Tabs
            defaultActiveKey="llamacpp"
            items={[
              {
                key: 'llamacpp',
                label: 'Llama.cpp',
                children: <VersionSections engine="llamacpp" />
              },
              {
                key: 'mistralrs',
                label: 'Mistral.rs',
                children: <VersionSections engine="mistralrs" />
              }
            ]}
          />

          {/* Singleton runtime config — applies across both engines. */}
          <RuntimeConfigCard />
        </Flex>
      </SettingsPageContainer>

      <RuntimeDownloadDrawer />
    </>
  )
}
