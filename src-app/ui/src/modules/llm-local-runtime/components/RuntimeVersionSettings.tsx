import { Flex, Tabs } from 'antd'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { RuntimeVersionList } from './RuntimeVersionList'
import { RuntimeUpdateChecker } from './RuntimeUpdateChecker'
import { RuntimeModelsByVersion } from './RuntimeModelsByVersion'
import { RuntimeDownloadDrawer } from './drawers/RuntimeDownloadDrawer'
import { GpuDetectionCard } from './GpuDetectionCard'
import { RuntimeConfigCard } from './RuntimeConfigCard'
import type { RuntimeEngine } from '../types'

// The version catalogue, update-checker, and per-version model usage all read
// `llm_local_runtime::versions_read` endpoints. The page route only requires
// `llm_local_runtime::read`, so gate these sections explicitly — otherwise a
// read-only principal without versions_read sees 403'ing/empty cards.
function VersionSections({ engine }: { engine: RuntimeEngine }) {
  return (
    <Flex className="flex-col gap-3">
      <GpuDetectionCard engine={engine} />
      <Can permission={Permissions.RuntimeVersionRead}>
        <RuntimeUpdateChecker engine={engine} />
        <RuntimeVersionList engine={engine} />
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
        subtitle="Hardware acceleration, engine binary versions, and runtime configuration for local model inference"
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
