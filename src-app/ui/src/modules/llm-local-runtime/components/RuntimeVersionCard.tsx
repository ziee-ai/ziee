import { useState } from 'react'
import {
  App,
  Button,
  Checkbox,
  Descriptions,
  Flex,
  Popconfirm,
  Tooltip,
  Typography,
} from 'antd'
import {
  DeleteOutlined,
  StarOutlined,
} from '@ant-design/icons'

const { Text } = Typography
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type RuntimeVersionResponse } from '@/api-client/types'

interface Props {
  version: RuntimeVersionResponse
}

/**
 * One installed-version row, laid out to match the peer-module
 * settings pattern (UsersSettings, LlmRepositorySettings):
 *
 *  - Title row: `Version v…` on the left + a `(Default)` secondary
 *    tag when applicable; action buttons (`type="text"`) on the
 *    top-right. No leading status dot — the Badge that previously
 *    rendered a colored dot before the version label has been
 *    dropped; the operator can tell a version is the system default
 *    from the `Default` text tag + the `Set as Default` button
 *    being absent.
 *  - Metadata is rendered via a compact `Descriptions` with
 *    `colon={false}` and 12px gray labels — same look as
 *    `UsersSettings`'s Email / Last Login / Created strip.
 */
export function RuntimeVersionCard({ version }: Props) {
  const { settingDefault, deleting } = Stores.RuntimeVersion

  const isSettingDefault = settingDefault.get(version.id) || false
  const isDeleting = deleting.get(version.id) || false

  const canUpdate = usePermission(Permissions.RuntimeVersionUpdate)
  const canDelete = usePermission(Permissions.RuntimeVersionDelete)

  const [removeBinary, setRemoveBinary] = useState(false)
  // Extra acknowledgment gate when deleting the system-default version — a
  // mis-click here drops the version new sessions fall back to, so require an
  // explicit confirmation beyond the standard Popconfirm.
  const [ackDefault, setAckDefault] = useState(false)
  const { message } = App.useApp()

  const handleSetDefault = async () => {
    try {
      await Stores.RuntimeVersion.setDefaultVersion(version.id)
    } catch {
      // Error already handled in store
    }
  }

  const handleDelete = async () => {
    try {
      await Stores.RuntimeVersion.deleteVersion(version.id, removeBinary)
    } catch (error) {
      // Surface the in-use guard (409) reason, e.g. which models/providers
      // still depend on this version.
      message.error(
        error instanceof Error ? error.message : 'Failed to delete version'
      )
    }
  }

  const actions: React.ReactNode[] = []
  if (canUpdate && !version.is_system_default) {
    actions.push(
      <Tooltip
        key="set-default"
        title="Make this version the default for new sessions"
      >
        <Button
          type="text"
          icon={<StarOutlined />}
          loading={isSettingDefault}
          onClick={handleSetDefault}
          aria-label={`Set version ${version.version} as default`}
        >
          Set as Default
        </Button>
      </Tooltip>
    )
  }
  if (canDelete) {
    actions.push(
      <Popconfirm
        key="delete"
        title="Delete Runtime Version"
        description={
          <Flex vertical gap="small">
            <Text>
              Are you sure you want to delete version {version.version}?
            </Text>
            {version.is_system_default && (
              <>
                <Text type="danger">
                  Warning: This is the default version. New sessions will fall
                  back to another version after deletion.
                </Text>
                <Checkbox
                  checked={ackDefault}
                  onChange={e => setAckDefault(e.target.checked)}
                >
                  I understand this is the default version
                </Checkbox>
              </>
            )}
            <Checkbox
              checked={removeBinary}
              onChange={e => setRemoveBinary(e.target.checked)}
            >
              Also remove cached files from disk
            </Checkbox>
          </Flex>
        }
        onConfirm={handleDelete}
        onOpenChange={open => {
          if (!open) setAckDefault(false)
        }}
        okText="Delete"
        okButtonProps={{
          danger: true,
          disabled: version.is_system_default && !ackDefault,
        }}
      >
        <Button
          type="text"
          danger
          icon={<DeleteOutlined />}
          loading={isDeleting}
          aria-label={`Delete version ${version.version}`}
        >
          Delete
        </Button>
      </Popconfirm>
    )
  }

  return (
    <div>
      <div className="flex items-center gap-2 mb-2 flex-wrap">
        <div className="flex-1 min-w-48">
          <Flex align="center" gap="small">
            <Text className="font-medium">Version {version.version}</Text>
            {version.is_system_default && (
              <Text type="secondary" className="text-xs">
                (Default)
              </Text>
            )}
          </Flex>
        </div>
        <div className="flex gap-1 items-center justify-end">{actions}</div>
      </div>

      <Descriptions
        size="small"
        column={{ xs: 1, sm: 2, md: 4 }}
        colon={false}
        styles={{
          label: { fontSize: '12px', color: '#8c8c8c' },
          content: { fontSize: '12px' },
        }}
      >
        <Descriptions.Item label="Platform">
          {version.platform}
        </Descriptions.Item>
        <Descriptions.Item label="Architecture">
          {version.arch}
        </Descriptions.Item>
        <Descriptions.Item label="Backend">
          {version.backend.toUpperCase()}
        </Descriptions.Item>
        <Descriptions.Item label="Installed">
          {new Date(version.created_at).toLocaleString()}
        </Descriptions.Item>
      </Descriptions>
    </div>
  )
}
