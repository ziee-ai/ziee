/**
 * Stories for components the initial gallery missed (flagged by the coverage
 * audit): Space, Layout, ScrollArea, Image, Upload, Attachment, SidebarTrigger,
 * and FormList (dynamic field array).
 */
import { File as FileIcon, X } from 'lucide-react'
import {
  Attachment,
  AttachmentAction,
  AttachmentActions,
  AttachmentContent,
  AttachmentDescription,
  AttachmentMedia,
  AttachmentTitle,
  Button,
  Form,
  FormField,
  FormList,
  Image,
  Input,
  Layout,
  ScrollArea,
  SidebarProvider,
  SidebarTrigger,
  Space,
  Tag,
  Upload,
  useForm,
} from '@/components/ui'
import type { GalleryStory } from '../story'

const noop = () => undefined

// Deterministic inline image (no network) for the Image story.
const SAMPLE_IMG =
  'data:image/svg+xml,' +
  encodeURIComponent(
    '<svg xmlns="http://www.w3.org/2000/svg" width="96" height="64"><rect width="96" height="64" fill="%233A5BA0"/><text x="48" y="38" font-size="12" fill="white" text-anchor="middle">IMG</text></svg>',
  )

const spaceStory: GalleryStory = {
  id: 'space',
  title: 'Space',
  cases: [
    {
      key: 'directions',
      label: 'Horizontal / vertical / sizes',
      render: () => (
        <div className="flex flex-col gap-4">
          <Space data-testid="g-space-h" size="md">
            <Button data-testid="g-space-b1" size="sm">
              One
            </Button>
            <Button data-testid="g-space-b2" size="sm" variant="outline">
              Two
            </Button>
            <Tag data-testid="g-space-tag" tone="info">
              Tag
            </Tag>
          </Space>
          <Space data-testid="g-space-v" direction="vertical" size="sm">
            <Tag data-testid="g-space-v1" tone="success">
              Alpha
            </Tag>
            <Tag data-testid="g-space-v2" tone="warning">
              Beta
            </Tag>
          </Space>
        </div>
      ),
    },
  ],
}

const layoutStory: GalleryStory = {
  id: 'layout',
  title: 'Layout',
  note: 'Header / Sider / Content / Footer regions',
  cases: [
    {
      key: 'hasSider',
      label: 'Header + Sider + Content + Footer',
      render: () => (
        <Layout
          data-testid="g-layout"
          className="h-48 w-80 overflow-hidden rounded-md border border-border"
        >
          <Layout.Header data-testid="g-layout-header" className="bg-muted/50 text-sm">
            Header
          </Layout.Header>
          <Layout hasSider data-testid="g-layout-body">
            <Layout.Sider
              data-testid="g-layout-sider"
              className="w-28 bg-muted/30 p-2 text-sm"
            >
              Sider
            </Layout.Sider>
            <Layout.Content
              data-testid="g-layout-content"
              className="p-2 text-sm text-muted-foreground"
            >
              Content area
            </Layout.Content>
          </Layout>
          <Layout.Footer data-testid="g-layout-footer" className="bg-muted/50 text-sm">
            Footer
          </Layout.Footer>
        </Layout>
      ),
    },
  ],
}

const scrollAreaStory: GalleryStory = {
  id: 'scroll-area',
  title: 'ScrollArea',
  cases: [
    {
      key: 'y',
      label: 'Vertical scroll',
      render: () => (
        <ScrollArea axis="y" className="h-32 w-48 rounded-md border border-border">
          <div className="flex flex-col gap-1 p-2">
            {Array.from({ length: 14 }, (_, i) => (
              <div key={i} className="rounded bg-muted px-2 py-1 text-sm">
                Row {i + 1}
              </div>
            ))}
          </div>
        </ScrollArea>
      ),
    },
  ],
}

const imageStory: GalleryStory = {
  id: 'image',
  title: 'Image',
  cases: [
    {
      key: 'basic',
      label: 'Sized / fallback',
      render: () => (
        <div className="flex items-start gap-3">
          <Image src={SAMPLE_IMG} alt="Sample" width={96} height={64} />
          <Image
            src="data:image/png;base64,invalid"
            alt="Broken with fallback"
            width={96}
            height={64}
            fallback={
              <div className="flex size-full items-center justify-center bg-muted text-xs text-muted-foreground">
                no image
              </div>
            }
          />
        </div>
      ),
    },
  ],
}

const uploadStory: GalleryStory = {
  id: 'upload',
  title: 'Upload',
  cases: [
    {
      key: 'dropzone',
      label: 'Dropzone / disabled',
      render: () => (
        <div className="flex flex-wrap gap-3">
          <Upload
            data-testid="g-upload"
            label="Upload files"
            onFiles={noop}
            className="w-48"
          >
            <div className="flex flex-col items-center gap-1 p-4 text-center text-sm text-muted-foreground">
              <FileIcon className="size-5" />
              Drag files here or click
            </div>
          </Upload>
          <Upload
            data-testid="g-upload-disabled"
            label="Upload disabled"
            onFiles={noop}
            disabled
            className="w-48"
          >
            <div className="p-4 text-center text-sm text-muted-foreground">
              Disabled
            </div>
          </Upload>
        </div>
      ),
    },
  ],
}

const attachmentStory: GalleryStory = {
  id: 'attachment',
  title: 'Attachment',
  note: 'file chip — states + orientation',
  cases: [
    {
      key: 'states',
      label: 'done / uploading / error',
      render: () => (
        <div className="flex flex-wrap gap-3">
          {(['done', 'uploading', 'error'] as const).map(state => (
            <Attachment key={state} state={state}>
              <AttachmentMedia variant="icon">
                <FileIcon />
              </AttachmentMedia>
              <AttachmentContent>
                <AttachmentTitle>report-{state}.pdf</AttachmentTitle>
                <AttachmentDescription>1.2 MB · {state}</AttachmentDescription>
              </AttachmentContent>
              <AttachmentActions>
                <AttachmentAction aria-label="Remove attachment">
                  <X />
                </AttachmentAction>
              </AttachmentActions>
            </Attachment>
          ))}
        </div>
      ),
    },
  ],
}

const sidebarTriggerStory: GalleryStory = {
  id: 'sidebar-trigger',
  title: 'SidebarTrigger',
  cases: [
    {
      key: 'default',
      label: 'Trigger',
      render: () => (
        <SidebarProvider>
          <SidebarTrigger
            data-testid="g-sidebar-trigger"
            aria-label="Toggle sidebar"
          />
        </SidebarProvider>
      ),
    },
  ],
}

interface FormListValues {
  items: { value: string }[]
}

function FormListDemo() {
  const form = useForm<FormListValues>({
    defaultValues: { items: [{ value: 'First' }, { value: 'Second' }] },
  })
  return (
    <Form
      data-testid="g-formlist-form"
      form={form}
      onSubmit={noop}
      className="w-64"
    >
      <FormList<FormListValues> name="items">
        {({ fields, append, remove }) => (
          <div className="flex flex-col gap-2">
            {fields.map((f, i) => (
              <div key={f.id} className="flex items-center gap-2">
                <FormField name={`items.${i}.value`} aria-label={`Item ${i + 1}`}>
                  <Input data-testid={`g-fl-input-${i}`} />
                </FormField>
                <Button
                  data-testid={`g-fl-remove-${i}`}
                  size="icon"
                  variant="ghost"
                  tooltip="Remove"
                  icon={<X />}
                  onClick={() => remove(i)}
                />
              </div>
            ))}
            <Button
              data-testid="g-fl-add"
              size="sm"
              variant="outline"
              onClick={() => append({ value: '' })}
            >
              Add item
            </Button>
          </div>
        )}
      </FormList>
    </Form>
  )
}

const formListStory: GalleryStory = {
  id: 'form-list',
  title: 'FormList',
  note: 'dynamic field array (add / remove rows)',
  cases: [{ key: 'dynamic', label: 'Dynamic rows', render: () => <FormListDemo /> }],
}

export const missingStories: GalleryStory[] = [
  spaceStory,
  layoutStory,
  scrollAreaStory,
  imageStory,
  uploadStory,
  attachmentStory,
  sidebarTriggerStory,
  formListStory,
]
