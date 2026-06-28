import { useEffect } from 'react'
import { App, Button, Spin, Typography } from 'antd'
import {
  FolderAddOutlined,
  FolderOutlined,
  PlusOutlined,
} from '@ant-design/icons'
import { Stores } from '@/core/stores'
import { Can } from '@/core/permissions'
import { Permissions, type Project } from '@/api-client/types'
import { ProjectCard } from '@/modules/projects/components/ProjectCard'
import { ProjectFormDrawer } from '@/modules/projects/components/ProjectFormDrawer'
import { HeaderBarContainer } from '@/modules/layouts/app-layout/components/HeaderBarContainer'

const { Title, Text } = Typography

export function ProjectsListPage() {
  const { message } = App.useApp()
  const { projects: projectsMap, loading, error } = Stores.Projects
  const projects = Array.from(projectsMap.values())

  useEffect(() => {
    if (error) {
      message.error(error)
      Stores.Projects.clearProjectsError()
    }
  }, [error, message])

  const handleCreate = () => Stores.ProjectDrawer.openProjectDrawer(null)
  const handleEdit = (project: Project) =>
    Stores.ProjectDrawer.openProjectDrawer(project)

  const handleDuplicate = async (project: Project) => {
    try {
      const copy = await Stores.Projects.duplicateProject(project.id)
      // `undefined` = a prior duplicate is still in flight; swallow
      // silently rather than showing a misleading success toast.
      if (copy) message.success(`Duplicated as "${copy.name}"`)
    } catch (_err) {
      message.error('Failed to duplicate project')
    }
  }

  const handleDelete = async (project: Project) => {
    try {
      await Stores.Projects.deleteProject(project.id)
      message.success('Project deleted')
    } catch (_err) {
      message.error('Failed to delete project')
    }
  }

  return (
    <div className="h-full flex flex-col overflow-hidden">
      <HeaderBarContainer>
        <div className="h-full flex items-center justify-between w-full">
          <Typography.Title level={4} className="!m-0 !leading-tight">
            Projects
          </Typography.Title>
          <Can permission={Permissions.ProjectsCreate}>
            <Button
              type="text"
              icon={<PlusOutlined />}
              onClick={handleCreate}
              aria-label="Create project"
            />
          </Can>
        </div>
      </HeaderBarContainer>

      <div className="flex-1 flex flex-col overflow-hidden items-center">
        {projects.length > 0 ? (
          <div className="flex flex-1 flex-col w-full overflow-hidden">
            <div className="h-full flex flex-col overflow-y-auto">
              <div className="max-w-4xl flex flex-wrap gap-3 pt-3 w-full self-center px-3">
                {projects.map(project => (
                  <div key={project.id} className="min-w-70 flex-1">
                    <ProjectCard
                      project={project}
                      onEdit={handleEdit}
                      onDuplicate={handleDuplicate}
                      onDelete={p => void handleDelete(p)}
                    />
                  </div>
                ))}
                <div className="min-w-70 flex-1" />
                <div className="min-w-70 flex-1" />
                <div className="min-w-70 flex-1" />
              </div>
            </div>
          </div>
        ) : loading ? (
          <div className="flex justify-center py-12 m-auto">
            <Spin />
          </div>
        ) : (
          <div className="text-center py-12 m-auto">
            <FolderOutlined className="text-6xl mb-4" />
              <Title level={3} type="secondary">
                No projects yet
              </Title>
              <Text type="secondary" className="block mb-4">
                Create a project to group related conversations under shared
                instructions and files.
              </Text>
              <Can permission={Permissions.ProjectsCreate}>
                <Button
                  type="primary"
                  icon={<FolderAddOutlined />}
                  onClick={handleCreate}
                >
                  Create Project
                </Button>
              </Can>
            </div>
        )}
      </div>

      <ProjectFormDrawer />
    </div>
  )
}
