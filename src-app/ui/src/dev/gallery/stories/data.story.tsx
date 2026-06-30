/**
 * Stories for data-display + collection components.
 */
import { useState } from 'react'
import {
  Descriptions,
  List,
  Menu,
  Pagination,
  Table,
  Tag,
  Tree,
} from '@/components/ui'
import type { TableColumn } from '@/components/ui'
import type { GalleryStory } from '../story'

interface Row {
  id: string
  name: string
  role: string
  status: 'active' | 'invited'
}

const rows: Row[] = [
  { id: '1', name: 'Ada Lovelace', role: 'Admin', status: 'active' },
  { id: '2', name: 'Alan Turing', role: 'Engineer', status: 'active' },
  { id: '3', name: 'Grace Hopper', role: 'Engineer', status: 'invited' },
]

const columns: TableColumn<Row>[] = [
  { key: 'name', title: 'Name', dataIndex: 'name' },
  { key: 'role', title: 'Role', dataIndex: 'role' },
  {
    key: 'status',
    title: 'Status',
    render: r => (
      <Tag
        data-testid={`g-table-status-${r.id}`}
        tone={r.status === 'active' ? 'success' : 'warning'}
      >
        {r.status}
      </Tag>
    ),
  },
]

const tableStory: GalleryStory = {
  id: 'table',
  title: 'Table',
  cases: [
    {
      key: 'basic',
      label: 'Basic',
      render: () => (
        <div className="w-[28rem]">
          <Table
            data-testid="g-table"
            columns={columns}
            dataSource={rows}
            rowKey="id"
          />
        </div>
      ),
    },
  ],
}

const listStory: GalleryStory = {
  id: 'list',
  title: 'List',
  cases: [
    {
      key: 'basic',
      label: 'With header',
      render: () => (
        <div className="w-72 rounded-md border border-border">
          <List
            data-testid="g-list"
            dataSource={rows}
            rowKey="id"
            header="Team"
            renderItem={item => (
              <div className="flex justify-between">
                <span>{item.name}</span>
                <span className="text-muted-foreground">{item.role}</span>
              </div>
            )}
          />
        </div>
      ),
    },
  ],
}

const descriptionsStory: GalleryStory = {
  id: 'descriptions',
  title: 'Descriptions',
  cases: [
    {
      key: 'bordered',
      label: 'Bordered',
      render: () => (
        <div className="w-96">
          <Descriptions
            data-testid="g-desc"
            title="Account"
            bordered
            column={1}
            items={[
              { key: 'name', label: 'Name', children: 'Ada Lovelace' },
              { key: 'email', label: 'Email', children: 'ada@example.com' },
              { key: 'role', label: 'Role', children: 'Admin' },
            ]}
          />
        </div>
      ),
    },
  ],
}

const treeStory: GalleryStory = {
  id: 'tree',
  title: 'Tree',
  cases: [
    {
      key: 'basic',
      label: 'Basic',
      render: () => (
        <div className="w-64">
          <Tree
            data-testid="g-tree"
            aria-label="File tree"
            defaultExpandedKeys={['src']}
            defaultSelectedKey="app"
            treeData={[
              {
                key: 'src',
                title: 'src',
                children: [
                  { key: 'app', title: 'App.tsx', isLeaf: true },
                  { key: 'main', title: 'main.tsx', isLeaf: true },
                ],
              },
              { key: 'readme', title: 'README.md', isLeaf: true },
            ]}
          />
        </div>
      ),
    },
    {
      key: 'checkable',
      label: 'Checkable',
      render: () => (
        <div className="w-64">
          <Tree
            data-testid="g-tree-checkable"
            aria-label="Checkable tree"
            checkable
            defaultExpandedKeys={['docs']}
            defaultCheckedKeys={['intro']}
            treeData={[
              {
                key: 'docs',
                title: 'docs',
                children: [
                  { key: 'intro', title: 'intro.md', isLeaf: true },
                  { key: 'guide', title: 'guide.md', isLeaf: true },
                ],
              },
            ]}
          />
        </div>
      ),
    },
  ],
}

const menuStory: GalleryStory = {
  id: 'menu',
  title: 'Menu',
  cases: [
    {
      key: 'vertical',
      label: 'Vertical',
      render: () => (
        <div className="w-56 rounded-md border border-border">
          <Menu
            data-testid="g-menu"
            aria-label="Navigation"
            selectedKey="home"
            items={[
              { key: 'home', label: 'Home' },
              { key: 'projects', label: 'Projects' },
              { type: 'divider' },
              { key: 'settings', label: 'Settings' },
            ]}
          />
        </div>
      ),
    },
  ],
}

function PaginationDemo() {
  const [page, setPage] = useState(2)
  return (
    <Pagination
      data-testid="g-pagination"
      aria-label="Pagination"
      current={page}
      total={240}
      pageSize={20}
      onChange={setPage}
      previousLabel="Previous"
      nextLabel="Next"
      pageLabel={p => `Page ${p}`}
    />
  )
}

const paginationStory: GalleryStory = {
  id: 'pagination',
  title: 'Pagination',
  cases: [
    { key: 'basic', label: 'Basic', render: () => <PaginationDemo /> },
  ],
}

export const dataStories: GalleryStory[] = [
  tableStory,
  listStory,
  descriptionsStory,
  treeStory,
  menuStory,
  paginationStory,
]
