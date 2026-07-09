import { RotateCw, Star, Trash2 } from 'lucide-react'
import { Fragment, useState } from 'react'
import {
  Button,
  Card,
  Checkbox,
  Confirm,
  Descriptions,
  Empty,
  ErrorState,
  Flex,
  Separator,
  Spin,
  Text,
  Tooltip,
  message,
} from '@/components/ui'
import { Stores } from '@/core/stores'
import { usePermission } from '@/core/permissions'
import { Permissions, type RuntimeVersionResponse2 } from '@/api-client/types'

/**
 * Installed whisper runtimes. Each row shows the version metadata plus
 * set-default / delete actions. Mirrors llm-local-runtime's
 * InstalledVersionsCard / RuntimeVersionCard, single-engine.
 */
export function InstalledVersionsCard() {
  const { versions, loading, error } = Stores.VoiceRuntimeVersion

  const handleRefresh = () => {
    Stores.VoiceRuntimeVersion.loadVersions().catch(() =>
      message.error('Failed to refresh runtime versions'),
    )
  }

  return (
    <Card
      title="Installed runtimes"
      data-testid="voice-installed-versions-card"
      extra={
        <Button
          icon={<RotateCw />}
          loading={loading}
          onClick={handleRefresh}
          data-testid="voice-installed-refresh"
          aria-label="Refresh installed runtimes"
        >
          Refresh
        </Button>
      }
    >
      {loading && versions.length === 0 ? (
        <Spin label="Loading" />
      ) : error && versions.length === 0 ? (
        <ErrorState
          resource="installed runtimes"
          description="The installed whisper runtimes couldn't be loaded."
          details={error}
          onRetry={handleRefresh}
          data-testid="voice-installed-error"
        />
      ) : versions.length === 0 ? (
        <Empty
          description="No runtimes installed yet — install one below."
          data-testid="voice-installed-empty"
        />
      ) : (
        <div>
          {versions.map((v, i) => (
            <Fragment key={v.id}>
              {i > 0 && <Separator className="!my-4" />}
              <InstalledVersionRow version={v} />
            </Fragment>
          ))}
        </div>
      )}
    </Card>
  )
}

function InstalledVersionRow({ version }: { version: RuntimeVersionResponse2 }) {
  const { settingDefault, deleting } = Stores.VoiceRuntimeVersion
  const canManage = usePermission(Permissions.VoiceAdminManage)
  const isSettingDefault = settingDefault.get(version.id) || false
  const isDeleting = deleting.get(version.id) || false
  const [removeBinary, setRemoveBinary] = useState(false)
  const [ackDefault, setAckDefault] = useState(false)

  const handleSetDefault = async () => {
    try {
      await Stores.VoiceRuntimeVersion.setDefaultVersion(version.id)
    } catch {
      /* error surfaced in store */
    }
  }

  const handleDelete = async () => {
    try {
      await Stores.VoiceRuntimeVersion.deleteVersion(version.id, removeBinary)
      setAckDefault(false)
    } catch (error) {
      message.error(error instanceof Error ? error.message : 'Failed to delete version')
    }
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
        <div className="flex flex-wrap gap-1 items-center justify-end">
          {canManage && !version.is_system_default && (
            <Tooltip content="Make this the runtime new sessions use">
              <Button
                variant="ghost"
                icon={<Star />}
                loading={isSettingDefault}
                onClick={handleSetDefault}
                data-testid={`voice-version-set-default-${version.version}`}
                aria-label={`Set version ${version.version} as default`}
              >
                Set as Default
              </Button>
            </Tooltip>
          )}
          {canManage && (
            <Confirm
              data-testid={`voice-version-delete-confirm-${version.version}`}
              title="Delete Runtime Version"
              description={
                <Flex direction="column" gap="small" className="[&_*]:!m-0">
                  <Text>Are you sure you want to delete version {version.version}?</Text>
                  {version.is_system_default && (
                    <>
                      <Text type="danger">
                        Warning: This is the default runtime. New sessions will fall
                        back to another runtime after deletion.
                      </Text>
                      <Checkbox
                        checked={ackDefault}
                        onChange={(e: boolean) => setAckDefault(e)}
                        label="I understand this is the default runtime"
                        data-testid={`voice-version-delete-ackdefault-${version.version}`}
                      />
                    </>
                  )}
                  <Checkbox
                    checked={removeBinary}
                    onChange={(e: boolean) => setRemoveBinary(e)}
                    label="Also remove cached files from disk"
                    data-testid={`voice-version-delete-removebinary-${version.version}`}
                  />
                </Flex>
              }
              onConfirm={handleDelete}
              onOpenChange={open => {
                if (!open) setAckDefault(false)
              }}
              okText="Delete"
              cancelText="Cancel"
              okButtonProps={{
                danger: true,
                disabled: version.is_system_default && !ackDefault,
              }}
            >
              <Button
                variant="ghost"
                icon={<Trash2 />}
                loading={isDeleting}
                data-testid={`voice-version-delete-${version.version}`}
                aria-label={`Delete version ${version.version}`}
              >
                Delete
              </Button>
            </Confirm>
          )}
        </div>
      </div>

      <Descriptions
        size="sm"
        data-testid={`voice-version-desc-${version.version}`}
        items={[
          { key: 'platform', label: 'Platform', children: version.platform },
          { key: 'arch', label: 'Architecture', children: version.arch },
          { key: 'backend', label: 'Backend', children: version.backend.toUpperCase() },
          {
            key: 'installed',
            label: 'Installed',
            children: new Date(version.created_at).toLocaleString(),
          },
        ]}
      />
    </div>
  )
}
