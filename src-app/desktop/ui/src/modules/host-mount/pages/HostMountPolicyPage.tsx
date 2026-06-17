/**
 * Host-mount policy admin page (DESKTOP-ONLY).
 *
 * Controls the deployment policy for host-folder mounting into the code
 * sandbox: master enable, the allowed host path prefixes (empty = any), and
 * whether read-write mounts are permitted. Gated by `host_mount::manage`.
 */

import { useEffect, useState } from 'react'
import { App, Button, Card, Select, Switch, Typography } from 'antd'

import { SettingsPageContainer } from '@/modules/settings/components/SettingsPageContainer'
import { Stores } from '@/core/stores'

export function HostMountPolicyPage() {
  const { message } = App.useApp()
  const { policy, loading, saving } = Stores.HostMountPolicy

  const [enabled, setEnabled] = useState(true)
  const [allowReadwrite, setAllowReadwrite] = useState(false)
  const [prefixes, setPrefixes] = useState<string[]>([])

  // Mirror the loaded policy into the editable form.
  useEffect(() => {
    if (policy) {
      setEnabled(policy.enabled)
      setAllowReadwrite(policy.allow_readwrite)
      setPrefixes(policy.allowed_prefixes ?? [])
    }
  }, [policy])

  const dirty =
    !!policy &&
    (enabled !== policy.enabled ||
      allowReadwrite !== policy.allow_readwrite ||
      JSON.stringify(prefixes) !== JSON.stringify(policy.allowed_prefixes ?? []))

  const save = async () => {
    try {
      await Stores.HostMountPolicy.updatePolicy({
        enabled,
        allow_readwrite: allowReadwrite,
        allowed_prefixes: prefixes,
      })
      message.success('Saved host-mount policy')
    } catch {
      message.error('Failed to save host-mount policy')
    }
  }

  return (
    <SettingsPageContainer
      title="Host Mount Policy"
      subtitle="Control whether folders from this machine can be mounted into the code sandbox, and which paths are allowed."
    >
      <Card loading={loading && !policy} className="mb-4" data-test-section="host-mount-policy">
        <div className="flex flex-col gap-4">
          <div className="flex items-center justify-between">
            <div>
              <Typography.Text strong>Allow host-folder mounting</Typography.Text>
              <Typography.Paragraph type="secondary" className="!mb-0">
                When off, no host folders are mounted into the sandbox on any
                project or conversation.
              </Typography.Paragraph>
            </div>
            <Switch checked={enabled} onChange={setEnabled} />
          </div>

          <div className="flex items-center justify-between">
            <div>
              <Typography.Text strong>Allow read-write mounts</Typography.Text>
              <Typography.Paragraph type="secondary" className="!mb-0">
                Off by default — mounts are read-only. Enabling this lets the
                sandbox modify the real files in mounted folders.
              </Typography.Paragraph>
            </div>
            <Switch checked={allowReadwrite} onChange={setAllowReadwrite} />
          </div>

          <div>
            <Typography.Text strong>Allowed path prefixes</Typography.Text>
            <Typography.Paragraph type="secondary" className="!mb-2">
              A folder is only mountable if its path starts with one of these.
              Leave empty to allow any path (typical for a single-user desktop).
            </Typography.Paragraph>
            <Select
              mode="tags"
              value={prefixes}
              onChange={setPrefixes}
              placeholder="/Users/me/data"
              style={{ width: '100%' }}
              tokenSeparators={[',']}
              aria-label="Allowed path prefixes"
            />
          </div>

          <div className="flex justify-end">
            <Button type="primary" onClick={save} loading={saving} disabled={!dirty}>
              Save
            </Button>
          </div>
        </div>
      </Card>
    </SettingsPageContainer>
  )
}
