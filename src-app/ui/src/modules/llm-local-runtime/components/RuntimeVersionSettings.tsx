import { Tabs } from 'antd'
import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { RuntimeVersionList } from './RuntimeVersionList'
import { RuntimeUpdateChecker } from './RuntimeUpdateChecker'
import { RuntimeDownloadDrawer } from './drawers/RuntimeDownloadDrawer'

export function RuntimeVersionSettings() {
  return (
    <>
      <SettingsPageContainer
        title="Local Runtime Versions"
        subtitle="Manage llama.cpp and mistral.rs binary versions for local model inference"
      >
        <Tabs
          defaultActiveKey="llamacpp"
          items={[
            {
              key: 'llamacpp',
              label: 'Llama.cpp',
              children: (
                <>
                  <RuntimeUpdateChecker engine="llamacpp" />
                  <RuntimeVersionList engine="llamacpp" />
                </>
              )
            },
            {
              key: 'mistralrs',
              label: 'Mistral.rs',
              children: (
                <>
                  <RuntimeUpdateChecker engine="mistralrs" />
                  <RuntimeVersionList engine="mistralrs" />
                </>
              )
            }
          ]}
        />
      </SettingsPageContainer>

      <RuntimeDownloadDrawer />
    </>
  )
}
