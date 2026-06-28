import React from 'react'
import {
  Button,
  Form,
  FormField,
  Input,
  Select,
  Space,
  message,
  useForm,
  zodResolver,
} from '@/components/ui'
import { z } from 'zod'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import type { DownloadVersionRequest } from '@/api-client/types'

const schema = z.object({
  engine: z.string(),
  version: z.string().min(1, 'Version is required'),
  platform: z.string().min(1, 'Platform is required'),
  arch: z.string().min(1, 'Architecture is required'),
  backend: z.string().min(1, 'Backend is required'),
})

export function RuntimeDownloadDrawer() {
  const { open, engine, closeDrawer } = Stores.RuntimeDownloadDrawer
  const { updateChecks, checking } = Stores.RuntimeUpdate
  // Server-host platform/arch from the GPU-detection store — always available
  // (local probe), unlike the update check which hits github.com and can fail.
  const { gpu } = Stores.RuntimeConfig
  const form = useForm<DownloadVersionRequest>({
    resolver: zodResolver(schema),
    defaultValues: {
      engine: '',
      version: 'latest',
      platform: '',
      arch: '',
      backend: 'cpu',
    },
  })
  const [submitting, setSubmitting] = React.useState(false)

  // Backend artifacts depend on the SERVER host (where the engine runs), not
  // the browser. The update check reports the published backends + the
  // GPU-version-matched recommendation; detect-gpu reports platform/arch.
  const updateCheck = engine ? updateChecks.get(engine) : undefined
  const isChecking = engine ? checking.get(engine) || false : false

  const readyVersions = (updateCheck?.versions ?? []).filter(v => v.binary_ready)
  const backendOptions = Array.from(
    new Set(readyVersions.flatMap(v => v.available_backends))
  )
  const recommended = readyVersions[0]?.recommended_backend
  const platform = updateCheck?.platform ?? gpu?.platform
  const arch = updateCheck?.arch ?? gpu?.arch

  // On open: ensure host detection is loaded + kick off the update check.
  React.useEffect(() => {
    if (!open || !engine) return
    if (!gpu) {
      Stores.RuntimeConfig.loadGpu().catch(() => {})
    }
    if (!updateCheck && !isChecking) {
      Stores.RuntimeUpdate.checkForUpdates(engine).catch(() => {
        // Surfaced via the store; the form still seeds from detect-gpu + cpu.
      })
    }
  }, [open, engine, gpu, updateCheck, isChecking])

  // Seed the form as soon as ANY host info is known, so a failed/slow update
  // check can't strand the user with empty required fields (cpu is always a
  // valid backend).
  React.useEffect(() => {
    if (open && engine && (platform || arch)) {
      form.reset({
        engine,
        version: 'latest',
        platform: platform ?? '',
        arch: arch ?? '',
        backend: recommended ?? backendOptions[0] ?? 'cpu'
      })
    }
  }, [open, engine, platform, arch, recommended, backendOptions.length, form])

  const handleSubmit = async (values: DownloadVersionRequest) => {
    setSubmitting(true)
    try {
      await Stores.RuntimeVersion.downloadVersion(values)
      message.success('Runtime version download started')
      closeDrawer()
      form.reset()
    } catch (error) {
      message.error(error instanceof Error ? error.message : 'Download failed')
    } finally {
      setSubmitting(false)
    }
  }

  const handleClose = () => {
    closeDrawer()
    form.reset()
  }

  return (
    <Drawer
      title={`Download ${engine} Runtime`}
      open={open}
      onClose={handleClose}
      size={600}
      footer={
        <Space>
          <Button variant="outline" onClick={handleClose}>Cancel</Button>
          <Button onClick={form.handleSubmit(handleSubmit)} loading={submitting}>
            Download
          </Button>
        </Space>
      }
    >
      <Form
        form={form}
        layout="vertical"
        onSubmit={handleSubmit}
      >
        <FormField
          label="Version"
          name="version"
          required
          description="Enter 'latest' for the newest version, or a specific version tag (e.g., 'b4359')"
        >
          <Input placeholder="latest" />
        </FormField>

        <FormField
          label="Platform"
          name="platform"
          required
        >
          <Select
            options={[
              { value: 'linux', label: 'Linux' },
              { value: 'macos', label: 'macOS' },
              { value: 'windows', label: 'Windows' },
            ]}
          />
        </FormField>

        <FormField
          label="Architecture"
          name="arch"
          required
        >
          <Select
            options={[
              { value: 'x86_64', label: 'x86_64' },
              { value: 'aarch64', label: 'aarch64' },
            ]}
          />
        </FormField>

        <FormField
          label="Backend"
          name="backend"
          required
          description={
            backendOptions.length > 0
              ? `Backends published for your host (${platform ?? '?'}/${arch ?? '?'}).`
              : isChecking
                ? 'Checking which backends are published for your host…'
                : 'Showing CPU as a safe default. The Available versions section on the engine card auto-detects published GPU builds for your host.'
          }
        >
          <Select
            loading={isChecking}
            options={(backendOptions.length > 0 ? backendOptions : ['cpu']).map(
              b => ({
                value: b,
                label: b === recommended ? `${b} (recommended)` : b
              })
            )}
          />
        </FormField>
      </Form>
    </Drawer>
  )
}
