import { useState } from 'react'
import { Form, Input, Button, Typography } from 'antd'
import { EditOutlined, CheckOutlined, CloseOutlined } from '@ant-design/icons'
import { IoIosArrowBack } from 'react-icons/io'
import type { Conversation } from '@/api-client/types'

interface TitleEditorProps {
  conversation: Conversation | null
  onSave: (title: string) => Promise<void>
  onBack: () => void
  canEdit?: boolean
}

export function TitleEditor({
  conversation,
  onSave,
  onBack,
  canEdit = true,
}: TitleEditorProps) {
  const [form] = Form.useForm()
  const [isEditing, setIsEditing] = useState(false)

  const handleEditClick = () => {
    form.setFieldValue('title', conversation?.title || '')
    setIsEditing(true)
  }

  const handleSave = async () => {
    try {
      const values = await form.validateFields()
      if (conversation && values.title.trim()) {
        await onSave(values.title.trim())
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

  if (isEditing) {
    return (
      <Form form={form} className="flex items-center gap-1 flex-1 max-w-full">
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
      <Button type="text" className="!px-1" onClick={onBack}>
        <IoIosArrowBack className="text-md" />
      </Button>
      <Typography.Title
        level={5}
        ellipsis
        className="!m-0 !leading-tight truncate"
      >
        {conversation?.title || 'Untitled Conversation'}
      </Typography.Title>
      {canEdit && (
        <Button
          type="text"
          icon={<EditOutlined />}
          onClick={handleEditClick}
        />
      )}
    </div>
  )
}
