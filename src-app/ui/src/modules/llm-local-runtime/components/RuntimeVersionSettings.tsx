import { Flex, Tabs } from 'antd'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { EngineVersionsCard } from './EngineVersionsCard'
import { RuntimeModelsByVersion } from './RuntimeModelsByVersion'
import { RuntimeDownloadDrawer } from './drawers/RuntimeDownloadDrawer'
import { RuntimeConfigCard } from './RuntimeConfigCard'
import type { RuntimeEngine } from '../types'

// Per-engine sections. EngineVersionsCard consolidates the prior
// GpuDetection + UpdateChecker + VersionList cards into one — host
// platform, available backends, installed versions, and upstream
// available versions (with inline per-version download) all live
// together. RuntimeModelsByVersion stays separate because it's
// model-centric (which models pin which engine version), not
// version-centric.
//
// The version catalogue + update-check + per-version model usage all
// read `llm_local_runtime::versions_read`. The page route only
// requires `llm_local_runtime::read`, so gate these sections
// explicitly — otherwise a read-only principal without versions_read
// sees 403'ing/empty cards.
function VersionSections({ engine }: { engine: RuntimeEngine }) {
  return (
    <Flex className="flex-col gap-3">
      <Can permission={Permissions.RuntimeVersionRead}>
        <EngineVersionsCard engine={engine} />
        <RuntimeModelsByVersion engine={engine} />
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
