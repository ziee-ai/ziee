import { Flex, Tabs } from 'antd'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { RuntimeVersionList } from './RuntimeVersionList'
import { RuntimeUpdateChecker } from './RuntimeUpdateChecker'
import { RuntimeModelsByVersion } from './RuntimeModelsByVersion'
import { RuntimeDownloadDrawer } from './drawers/RuntimeDownloadDrawer'
import { GpuDetectionCard } from './GpuDetectionCard'
import { RuntimeConfigCard } from './RuntimeConfigCard'

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
                children: (
                  <Flex className="flex-col gap-3">
                    <GpuDetectionCard engine="llamacpp" />
                    <RuntimeUpdateChecker engine="llamacpp" />
                    <RuntimeVersionList engine="llamacpp" />
                    <RuntimeModelsByVersion engine="llamacpp" />
                  </Flex>
                )
              },
              {
                key: 'mistralrs',
                label: 'Mistral.rs',
                children: (
                  <Flex className="flex-col gap-3">
                    <GpuDetectionCard engine="mistralrs" />
                    <RuntimeUpdateChecker engine="mistralrs" />
                    <RuntimeVersionList engine="mistralrs" />
                    <RuntimeModelsByVersion engine="mistralrs" />
                  </Flex>
                )
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
