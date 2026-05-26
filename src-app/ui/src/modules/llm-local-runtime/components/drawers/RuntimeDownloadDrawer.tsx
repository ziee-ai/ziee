import React from 'react'
import { Button, Form, Input, message, Select, Space } from 'antd'
import { Drawer } from '@/modules/layouts/app-layout/components/Drawer'
import { Stores } from '@/core/stores'
import { detectPlatform, detectArch, getDefaultBackend } from '../../utils/platform'
import type { DownloadVersionRequest } from '@/api-client/types'

export function RuntimeDownloadDrawer() {
  const { open, engine, closeDrawer } = Stores.RuntimeDownloadDrawer
  const [form] = Form.useForm<DownloadVersionRequest>()
  const [submitting, setSubmitting] = React.useState(false)

  const platform = Form.useWatch('platform', form)

  React.useEffect(() => {
    if (open && engine) {
      form.setFieldsValue({
        engine,
        version: 'latest',
        platform: detectPlatform(),
        arch: detectArch(),
        backend: getDefaultBackend(detectPlatform())
      })
    }
  }, [open, engine, form])

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
        >
          <Select>
            <Select.Option value="cpu">CPU</Select.Option>
            <Select.Option value="cuda" disabled={platform !== 'linux'}>
              CUDA (Linux only)
            </Select.Option>
            <Select.Option value="metal" disabled={platform !== 'macos'}>
              Metal (macOS only)
            </Select.Option>
          </Select>
        </Form.Item>

        <Form.Item name="engine" hidden>
          <Input />
        </Form.Item>
      </Form>
    </Drawer>
  )
}
