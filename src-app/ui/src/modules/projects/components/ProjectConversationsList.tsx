import { useNavigate } from 'react-router-dom'
import { Empty, List, Typography } from 'antd'
import { MessageOutlined } from '@ant-design/icons'
import { Stores } from '@/core/stores'

interface ProjectConversationsListProps {
  projectId: string
}

export function ProjectConversationsList({
  projectId,
}: ProjectConversationsListProps) {
  const navigate = useNavigate()
  const { conversations, conversationsLoading } = Stores.ProjectDetail

  if (!conversationsLoading && conversations.length === 0) {
    return (
      <Empty
        image={Empty.PRESENTED_IMAGE_SIMPLE}
        description="No conversations in this project yet"
      >
        <Typography.Text type="secondary">
          Start a new chat here and it will inherit this project's
          instructions + knowledge.
        </Typography.Text>
      </Empty>
    )
  }

  return (
    <List
      loading={conversationsLoading}
      dataSource={conversations}
      renderItem={item => (
        <List.Item
          className="cursor-pointer hover:bg-black/5 transition-colors"
          onClick={() => navigate(`/chat/${item.id}`)}
          data-project-id={projectId}
        >
          <List.Item.Meta
            avatar={<MessageOutlined />}
            title={
              <Typography.Text>
                {item.title || 'Untitled conversation'}
              </Typography.Text>
            }
            description={`${item.message_count} message${item.message_count === 1 ? '' : 's'}`}
          />
        </List.Item>
      )}
    />
  )
}
