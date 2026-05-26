import { useEffect } from 'react'
import { useNavigate, useParams } from 'react-router-dom'
import { App, Button, Card, Spin, Tabs, Typography } from 'antd'
import {
  ArrowLeftOutlined,
  CopyOutlined,
  EditOutlined,
  PlusOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions } from '@/api-client/types'
import { ProjectFormDrawer } from '@/modules/projects/components/ProjectFormDrawer'
import { ProjectFilesPanel } from '@/modules/projects/components/ProjectFilesPanel'
import { ProjectConversationsList } from '@/modules/projects/components/ProjectConversationsList'
import { ProjectMcpSettingsPanel } from '@/modules/projects/components/ProjectMcpSettingsPanel'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'

const { Title, Text, Paragraph } = Typography

export function ProjectDetailPage() {
  const { projectId } = useParams<{ projectId: string }>()
  const navigate = useNavigate()
  const { message } = App.useApp()

  const { project, loading, error } = Stores.ProjectDetail

  useEffect(() => {
    if (projectId) {
      void Stores.ProjectDetail.loadProject(projectId)
    }
  }, [projectId])

  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.ProjectDetail.clearProjectDetailError()
    }
  }, [error, message])

  if (!projectId) {
    return null
  }

  if (loading || !project) {
    return (
      <div className="h-full flex items-center justify-center">
        <Spin />
      </div>
    )
  }

  const handleEdit = () => Stores.ProjectDrawer.openProjectDrawer(project)

  const handleDuplicate = async () => {
    try {
      const copy = await Stores.Projects.duplicateProject(project.id)
      // `undefined` = a prior duplicate is still in flight; bail
      // silently so the user doesn't get a misleading toast or get
      // navigated to a phantom project page.
      if (!copy) return
      message.success(`Duplicated as "${copy.name}"`)
      navigate(`/projects/${copy.id}`)
    } catch (_err) {
      message.error('Failed to duplicate project')
    }
  }

  // Forward to the chat module; the chat composer reads the project
  // via the conversation's `project_id` after creation, so the server
  // wires up the rest. For v1 the FE just navigates to /chat?project_id=…
  // and the chat module's create flow honors it.
  const handleNewChat = () => {
    navigate(`/chat?project_id=${project.id}`)
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full gap-2">
          <div className="flex items-center min-w-0 gap-2">
            <Button
              type="text"
              icon={<ArrowLeftOutlined />}
              onClick={() => navigate('/projects')}
              aria-label="Back to projects"
            />
            <Title level={4} className="!m-0 !leading-tight truncate">
              {project.name}
            </Title>
          </div>
          <div className="flex items-center gap-1">
            <Can permission={Permissions.ProjectsEdit}>
              <Button type="text" icon={<EditOutlined />} onClick={handleEdit}>
                Edit
              </Button>
            </Can>
            {/* Duplicate is gated by BOTH create + read on the
                backend (handlers.rs: RequirePermissions<(ProjectsCreate,
                ProjectsRead)>). Match that on the FE so the button
                only renders when the user can actually succeed
                (audit Q12). */}
            <Can
              permission={{
                allOf: [
                  Permissions.ProjectsCreate,
                  Permissions.ProjectsRead,
                ],
              }}
            >
              <Button
                type="text"
                icon={<CopyOutlined />}
                onClick={handleDuplicate}
              >
                Duplicate
              </Button>
            </Can>
            <Button
              type="primary"
              icon={<PlusOutlined />}
              onClick={handleNewChat}
            >
              New chat
            </Button>
          </div>
        </div>
      </HeaderBarContainer>

      <div className="flex-1 overflow-y-auto">
        <div className="max-w-4xl mx-auto p-4 space-y-4">
          {project.description && (
            <Card title="About">
              <Paragraph>{project.description}</Paragraph>
            </Card>
          )}

          <Card title="Instructions">
            {project.instructions ? (
              <Paragraph className="whitespace-pre-wrap">
                {project.instructions}
              </Paragraph>
            ) : (
              <Text type="secondary" className="italic">
                No instructions yet — click Edit to add some.
              </Text>
            )}
          </Card>

          <Card>
            <Tabs
              items={[
                {
                  key: 'knowledge',
                  label: 'Knowledge',
                  children: <ProjectFilesPanel projectId={project.id} />,
                },
                {
                  key: 'conversations',
                  label: 'Conversations',
                  children: (
                    <ProjectConversationsList projectId={project.id} />
                  ),
                },
                {
                  key: 'mcp',
                  label: 'MCP Settings',
                  children: <ProjectMcpSettingsPanel project={project} />,
                },
              ]}
            />
          </Card>
        </div>
      </div>

      <ProjectFormDrawer />
    </div>
  )
}
