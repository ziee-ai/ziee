/**
 * Composite scenes — real-ish compositions where layout bugs actually surface
 * (a form, a card with header/footer/actions, a table panel, a populated
 * sidebar). Isolated components can pass every per-component check and still
 * break when laid out together; these scenes catch that.
 */
import {
  Avatar,
  Button,
  Card,
  Checkbox,
  Flex,
  Form,
  FormField,
  Input,
  Menu,
  Select,
  Switch,
  Table,
  Tag,
  Text,
  Textarea,
  Title,
  useForm,
} from '@/components/ui'
import type { TableColumn } from '@/components/ui'
import type { GalleryStory } from '../story'

const noop = () => undefined

interface ProfileValues {
  name: string
  email: string
  role: string
  bio: string
  notify: boolean
  agree: boolean
}

function ProfileFormScene() {
  const form = useForm<ProfileValues>({
    defaultValues: {
      name: 'Ada Lovelace',
      email: 'ada@example.com',
      role: 'admin',
      bio: '',
      notify: true,
      agree: false,
    },
  })
  return (
    <Card data-testid="g-scene-form-card" title="Profile" className="w-96">
      <Form
        data-testid="g-scene-form"
        form={form}
        onSubmit={noop}
        layout="vertical"
      >
        <FormField name="name" label="Name">
          <Input data-testid="g-scene-name" />
        </FormField>
        <FormField name="email" label="Email">
          <Input data-testid="g-scene-email" />
        </FormField>
        <FormField name="role" label="Role">
          <Select
            data-testid="g-scene-role"
            aria-label="Role"
            options={[
              { value: 'admin', label: 'Admin' },
              { value: 'editor', label: 'Editor' },
              { value: 'viewer', label: 'Viewer' },
            ]}
          />
        </FormField>
        <FormField name="bio" label="Bio">
          <Textarea data-testid="g-scene-bio" placeholder="A short bio…" />
        </FormField>
        <FormField name="notify" label="Email notifications" valuePropName="checked">
          <Switch data-testid="g-scene-notify" aria-label="Email notifications" />
        </FormField>
        <FormField name="agree" aria-label="Agree to terms" valuePropName="checked">
          <Checkbox data-testid="g-scene-agree" label="I agree to the terms" />
        </FormField>
        <Flex justify="end" gap="sm">
          <Button data-testid="g-scene-cancel" variant="ghost">
            Cancel
          </Button>
          <Button data-testid="g-scene-save">Save changes</Button>
        </Flex>
      </Form>
    </Card>
  )
}

const formScene: GalleryStory = {
  id: 'scene-form',
  title: 'Scene — form',
  note: 'labeled fields + actions inside a card',
  cases: [{ key: 'profile', label: 'Profile form', render: () => <ProfileFormScene /> }],
}

interface Invoice {
  id: string
  number: string
  amount: string
  status: 'paid' | 'due' | 'overdue'
}

const invoices: Invoice[] = [
  { id: '1', number: 'INV-001', amount: '$1,200.00', status: 'paid' },
  { id: '2', number: 'INV-002', amount: '$840.00', status: 'due' },
  { id: '3', number: 'INV-003', amount: '$2,310.00', status: 'overdue' },
]

const invoiceCols: TableColumn<Invoice>[] = [
  { key: 'number', title: 'Invoice', dataIndex: 'number' },
  { key: 'amount', title: 'Amount', dataIndex: 'amount', align: 'right' },
  {
    key: 'status',
    title: 'Status',
    render: r => (
      <Tag
        data-testid={`g-scene-invoice-status-${r.id}`}
        tone={
          r.status === 'paid'
            ? 'success'
            : r.status === 'due'
              ? 'warning'
              : 'error'
        }
      >
        {r.status}
      </Tag>
    ),
  },
]

function TablePanelScene() {
  return (
    <Card
      data-testid="g-scene-table-card"
      title="Invoices"
      extra={
        <Button data-testid="g-scene-table-new" size="sm">
          New invoice
        </Button>
      }
      className="w-[32rem]"
    >
      <Table
        data-testid="g-scene-table"
        columns={invoiceCols}
        dataSource={invoices}
        rowKey="id"
      />
    </Card>
  )
}

const tableScene: GalleryStory = {
  id: 'scene-table',
  title: 'Scene — table panel',
  note: 'card header + actions + data table',
  cases: [{ key: 'invoices', label: 'Invoices', render: () => <TablePanelScene /> }],
}

function SidebarScene() {
  return (
    <div
      data-testid="g-scene-sidebar"
      className="flex h-72 w-[40rem] overflow-hidden rounded-lg border border-border"
    >
      <aside className="flex w-56 flex-col gap-3 border-r border-border bg-muted/40 p-3">
        <Flex align="center" gap="sm">
          <Avatar fallback="Z" />
          <div className="flex flex-col">
            <Text strong>ziee</Text>
            <Text tone="muted" className="text-xs">
              Workspace
            </Text>
          </div>
        </Flex>
        <Menu
          data-testid="g-scene-nav"
          aria-label="Sidebar navigation"
          selectedKey="chat"
          items={[
            { key: 'chat', label: 'Chat' },
            { key: 'projects', label: 'Projects' },
            { key: 'library', label: 'Library' },
            { type: 'divider' },
            { key: 'settings', label: 'Settings' },
          ]}
        />
      </aside>
      <main className="flex flex-1 flex-col gap-3 p-4">
        <Flex justify="between" align="center">
          <Title level={3}>Chat</Title>
          <Button data-testid="g-scene-sidebar-new" size="sm">
            New
          </Button>
        </Flex>
        <Card data-testid="g-scene-sidebar-card" className="flex-1">
          <Text tone="muted">Main content area.</Text>
        </Card>
      </main>
    </div>
  )
}

const sidebarScene: GalleryStory = {
  id: 'scene-sidebar',
  title: 'Scene — sidebar layout',
  note: 'populated nav rail + content region',
  cases: [{ key: 'app', label: 'App shell', render: () => <SidebarScene /> }],
}

export const compositeStories: GalleryStory[] = [
  formScene,
  tableScene,
  sidebarScene,
]
