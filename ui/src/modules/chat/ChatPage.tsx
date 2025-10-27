import { Card, Input, Button } from 'antd'
import { SendOutlined, LogoutOutlined } from '@ant-design/icons'
import { logoutUser } from '../auth/store'
import { Stores } from '@/core/stores'

const { TextArea } = Input

export default function ChatPage() {
  const { user } = Stores.Auth

  const handleLogout = async () => {
    await logoutUser()
  }

  return (
    <div className="h-screen flex flex-col">
      {/* Chat Header */}
      <div className="h-16 bg-white border-b border-gray-200 px-6 flex items-center justify-between">
        <h2 className="text-lg font-semibold text-gray-900">New Conversation</h2>
        <div className="flex items-center gap-4">
          <span className="text-sm text-gray-600">
            Welcome, <strong>{user?.username}</strong>
          </span>
          <Button
            icon={<LogoutOutlined />}
            onClick={handleLogout}
          >
            Logout
          </Button>
        </div>
      </div>

      {/* Messages Area */}
      <div className="flex-1 overflow-y-auto p-6">
        <div className="max-w-4xl mx-auto">
          {/* Welcome Message */}
          <Card className="mb-4 bg-blue-50 border-blue-200">
            <div className="text-center py-8">
              <h3 className="text-2xl font-bold text-gray-900 mb-2">
                Welcome to Ziee Chat
              </h3>
              <p className="text-gray-600">
                Modular Architecture - Ready for Week 1 Implementation
              </p>
              <p className="text-sm text-gray-500 mt-4">
                Start a conversation to begin testing the new module system
              </p>
            </div>
          </Card>
        </div>
      </div>

      {/* Input Area */}
      <div className="border-t border-gray-200 bg-white p-4">
        <div className="max-w-4xl mx-auto">
          <div className="flex gap-2">
            <TextArea
              placeholder="Type your message here..."
              autoSize={{ minRows: 1, maxRows: 6 }}
              className="flex-1"
            />
            <Button
              type="primary"
              icon={<SendOutlined />}
              size="large"
            >
              Send
            </Button>
          </div>
        </div>
      </div>
    </div>
  )
}
