import { useState } from 'react'
import { Form, Input, Button, Tooltip, Typography } from 'antd'
import { EditOutlined, CheckOutlined, CloseOutlined } from '@ant-design/icons'
import { IoIosArrowBack } from 'react-icons/io'
import { useNavigate } from 'react-router-dom'
import { Stores } from '@/core/stores'
import { chatExtensionRegistry } from '@/modules/chat/core/extensions'

/**
 * TitleEditor Component
 * Self-contained component that accesses conversation from store and handles its own state/events
 */
export function TitleEditor() {
  const [form] = Form.useForm()
  const [isEditing, setIsEditing] = useState(false)
  const navigate = useNavigate()

  // Get conversation from store
  const { conversation } = Stores.Chat

  const handleEditClick = () => {
    form.setFieldValue('title', conversation?.title || '')
    setIsEditing(true)
  }

  const handleSave = async () => {
    try {
      const values = await form.validateFields()
      if (conversation && values.title.trim()) {
        await Stores.Chat.updateConversation({ title: values.title.trim() })
        setIsEditing(false)
      }
    } catch (error) {
      console.error('Failed to update title:', error)
    }
  }

  const handleCancel = () => {
    form.resetFields()
    setIsEditing(false)
  }

  const handleBack = () => {
    // Per-conversation back-target resolution: chat extensions can
    // override chat's default `/chats` via the `conversationBackHref`
    // hook. First non-undefined wins.
    const backHref = conversation
      ? chatExtensionRegistry.conversationBackHref(conversation)
      : undefined
    navigate(backHref ?? '/chats')
  }

  if (isEditing) {
    return (
      <Form name="title-editor" form={form} className="flex items-center gap-1 flex-1 max-w-full">
        <Form.Item
          name="title"
          className="!mb-0 flex-1"
          rules={[
            { required: true, message: 'Please enter a title' },
            {
              max: 100,
              message: 'Title must be less than 100 characters',
            },
          ]}
        >
          <Input
            placeholder="Enter conversation title"
            autoFocus
            onPressEnter={handleSave}
            size="small"
            className="!border-none !shadow-none"
            style={{
              backgroundColor: 'transparent',
              fontSize: '16px',
              fontWeight: 600,
            }}
          />
        </Form.Item>
        <Button
          type="text"
          size="small"
          icon={<CheckOutlined />}
          onClick={handleSave}
          className="!p-1"
        />
        <Button
          type="text"
          size="small"
          icon={<CloseOutlined />}
          onClick={handleCancel}
          className="!p-1"
        />
      </Form>
    )
  }

  return (
    <div className="flex gap-1 items-center justify-start overflow-hidden">
      <Button
        type="text"
        className="!px-1"
        onClick={handleBack}
        aria-label="Back to conversation list"
        data-testid="conversation-back-button"
      >
        <IoIosArrowBack className="text-md" />
      </Button>
      <Typography.Title
        level={5}
        ellipsis
        className="!m-0 !leading-tight truncate"
      >
        {conversation?.title || 'Untitled Conversation'}
      </Typography.Title>
      <Tooltip title="Edit title">
        <Button
          type="text"
          icon={<EditOutlined />}
          onClick={handleEditClick}
          aria-label="Edit conversation title"
        />
      </Tooltip>
    </div>
  )
}
