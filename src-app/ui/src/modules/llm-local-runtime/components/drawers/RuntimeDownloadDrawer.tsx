import React from 'react'
import { Button, Form, Input, message, Select, Space } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { ApiClient } from '@/api-client'
import type { DownloadVersionRequest } from '@/api-client/types'

export function RuntimeDownloadDrawer() {
  const { open, engine, closeDrawer } = Stores.RuntimeDownloadDrawer
  const { updateChecks, checking } = Stores.RuntimeUpdate
  const [form] = Form.useForm<DownloadVersionRequest>()
  const [submitting, setSubmitting] = React.useState(false)
  // Server-host platform/arch from /detect-gpu — always available (local
  // probe), unlike the update check which hits github.com and can fail.
  const [host, setHost] = React.useState<{ platform: string; arch: string } | null>(null)

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
  const platform = updateCheck?.platform ?? host?.platform
  const arch = updateCheck?.arch ?? host?.arch

  // On open: probe the host (platform/arch) and kick off the update check.
  React.useEffect(() => {
    if (!open || !engine) return
    ApiClient.LocalRuntime.detectGpu()
      .then(g => setHost({ platform: g.platform, arch: g.arch }))
      .catch(() => {})
    if (!updateCheck && !isChecking) {
      Stores.RuntimeUpdate.checkForUpdates(engine).catch(() => {
        // Surfaced via the store; the form still seeds from detect-gpu + cpu.
      })
    }
  }, [open, engine, updateCheck, isChecking])

  // Seed the form as soon as ANY host info is known, so a failed/slow update
  // check can't strand the user with empty required fields (cpu is always a
  // valid backend).
  React.useEffect(() => {
    if (open && engine && (platform || arch)) {
      form.setFieldsValue({
        engine,
        version: 'latest',
        platform,
        arch,
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
      form.resetFields()
    } catch (error) {
      message.error(error instanceof Error ? error.message : 'Download failed')
    } finally {
      setSubmitting(false)
    }
  }

  const handleClose = () => {
    closeDrawer()
    form.resetFields()
  }

  return (
    <Drawer
      title={`Download ${engine} Runtime`}
      open={open}
      onClose={handleClose}
      size={600}
      footer={
        <Space>
          <Button onClick={handleClose}>Cancel</Button>
          <Button type="primary" onClick={() => form.submit()} loading={submitting}>
            Download
          </Button>
        </Space>
      }
    >
      <Form
        form={form}
        layout="vertical"
        onFinish={handleSubmit}
      >
        <Form.Item
          label="Version"
          name="version"
          rules={[{ required: true, message: 'Version is required' }]}
          help="Enter 'latest' for the newest version, or a specific version tag (e.g., 'b4359')"
        >
          <Input placeholder="latest" />
        </Form.Item>

        <Form.Item
          label="Platform"
          name="platform"
          rules={[{ required: true, message: 'Platform is required' }]}
        >
          <Select>
            <Select.Option value="linux">Linux</Select.Option>
            <Select.Option value="macos">macOS</Select.Option>
            <Select.Option value="windows">Windows</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item
          label="Architecture"
          name="arch"
          rules={[{ required: true, message: 'Architecture is required' }]}
        >
          <Select>
            <Select.Option value="x86_64">x86_64</Select.Option>
            <Select.Option value="aarch64">aarch64</Select.Option>
          </Select>
        </Form.Item>

        <Form.Item
          label="Backend"
          name="backend"
          rules={[{ required: true, message: 'Backend is required' }]}
          help={
            backendOptions.length > 0
              ? `Backends published for your host (${platform ?? '?'}/${arch ?? '?'})`
              : isChecking
                ? 'Checking which backends are published for your host…'
                : 'Only cpu is confirmed; run "Check for Updates" to see GPU builds.'
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
        </Form.Item>

        <Form.Item name="engine" hidden>
          <Input />
        </Form.Item>
      </Form>
    </Drawer>
  )
}
