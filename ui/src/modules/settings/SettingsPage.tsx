import { Card, Tabs, Form, Input, Button, Switch } from 'antd'
import { Stores } from '@/core/stores'

export default function SettingsPage() {
  const { user } = Stores.Auth

  const items = [
    {
      key: 'profile',
      label: 'Profile',
      children: (
        <Card>
          <Form layout="vertical">
            <Form.Item label="Username">
              <Input defaultValue={user?.username} disabled />
            </Form.Item>
            <Form.Item label="Email">
              <Input defaultValue={user?.emails[0]?.address} disabled />
            </Form.Item>
            <Form.Item label="Member Since">
              <Input
                defaultValue={new Date(user?.created_at || '').toLocaleDateString()}
                disabled
              />
            </Form.Item>
          </Form>
        </Card>
      ),
    },
    {
      key: 'general',
      label: 'General',
      children: (
        <Card>
          <Form layout="vertical">
            <Form.Item label="Application Name">
              <Input defaultValue="Ziee Chat" />
            </Form.Item>
            <Form.Item label="Theme">
              <div className="flex items-center gap-2">
                <span>Light</span>
                <Switch />
                <span>Dark</span>
              </div>
            </Form.Item>
            <Form.Item>
              <Button type="primary">Save Settings</Button>
            </Form.Item>
          </Form>
        </Card>
      ),
    },
    {
      key: 'modules',
      label: 'Modules',
      children: (
        <Card>
          <div className="space-y-4">
            <div className="border-l-4 border-blue-500 pl-4 py-2">
              <h4 className="font-semibold text-gray-900">Module System</h4>
              <p className="text-sm text-gray-600">
                Modules will be displayed here once implemented
              </p>
              <p className="text-xs text-gray-500 mt-2">
                Week 2+: Projects, Assistants, Hub, etc.
              </p>
            </div>
          </div>
        </Card>
      ),
    },
  ]

  return (
    <div className="h-screen overflow-y-auto">
      <div className="p-6">
        <div className="mb-6">
          <h1 className="text-2xl font-bold text-gray-900">Settings</h1>
          <p className="text-gray-600">Configure your Ziee Chat application</p>
        </div>

        <Tabs items={items} />
      </div>
    </div>
  )
}
