/**
 * Stories for data-display + collection components.
 */
import { Home, Search, Settings } from 'lucide-react'
import { useState } from 'react'
import {
  Button,
  Descriptions,
  List,
  Menu,
  Pagination,
  Table,
  Tag,
  Tree,
} from '@/components/ui'
import type { TableColumn } from '@/components/ui'
import { DelimitedTable } from '@/modules/file/viewers/tabular/DelimitedTable'
import { XlsxSheet } from '@/modules/file/viewers/tabular/XlsxBody'
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

// ── kit Table actions (sort / filter / resize / column-chooser / numeric /
//    ellipsis / selection) — the F1 capability surface for e2e. ───────────────
interface ARow {
  id: string
  name: string
  qty: string
  note: string
}
const actionRows: ARow[] = [
  { id: '1', name: 'Banana', qty: '10', note: 'A short note' },
  { id: '2', name: 'apple', qty: '2', note: 'This is a deliberately long cell value that must be clipped and expandable via a popover in the tabular viewer' },
  { id: '3', name: 'Cherry', qty: '30', note: 'Another note' },
]
const actionColumns: TableColumn<ARow>[] = [
  { key: 'name', title: 'Name', dataIndex: 'name', hideable: true },
  { key: 'qty', title: 'Qty', dataIndex: 'qty', hideable: true },
  { key: 'note', title: 'Note', dataIndex: 'note', ellipsis: true, hideable: true },
]
const tableActionsStory: GalleryStory = {
  id: 'table-actions',
  title: 'Table — actions',
  cases: [
    {
      key: 'actions',
      label: 'Sort / filter / resize / columns / numeric',
      render: () => (
        <div className="w-[34rem]">
          <Table
            data-testid="g-table-actions"
            columns={actionColumns}
            dataSource={actionRows}
            rowKey="id"
            sortable
            filterable
            resizable
            columnChooser
            detectNumericColumns
            selectionMode="cell"
            filterPlaceholder="Filter rows…"
          />
        </div>
      ),
    },
  ],
}

// Virtualized scroll-to-index case (jump-to-row mechanic).
function ScrollDemo() {
  const [scrollTo, setScrollTo] = useState<number | null>(null)
  const rows = Array.from({ length: 500 }, (_, i) => ({
    id: String(i),
    name: `Row ${i}`,
    value: String(i * 3),
  }))
  const cols: TableColumn<{ id: string; name: string; value: string }>[] = [
    { key: 'name', title: 'Name', dataIndex: 'name' },
    { key: 'value', title: 'Value', dataIndex: 'value', numeric: true },
  ]
  return (
    <div className="w-96 flex flex-col gap-2">
      <Button data-testid="g-table-scroll-btn" onClick={() => setScrollTo(400)}>
        Scroll to row 400
      </Button>
      <div className="h-64">
        <Table
          data-testid="g-table-scroll"
          columns={cols}
          dataSource={rows}
          rowKey="id"
          virtualized
          scrollToIndex={scrollTo}
        />
      </div>
    </div>
  )
}
const tableScrollStory: GalleryStory = {
  id: 'table-scroll',
  title: 'Table — scroll-to-index',
  cases: [{ key: 'scroll', label: 'Virtualized jump', render: () => <ScrollDemo /> }],
}

// The real tabular file viewer, prop-driven (no store/binary needed).
const CSV_TEXT = [
  'Name,Qty,Note',
  'Banana,10,A short note',
  'apple,2,This is a deliberately long cell value that must be clipped and expandable via a popover in the tabular viewer',
  'Cherry,30,Another note',
  'Date,7,Yet another',
].join('\n')
const delimitedStory: GalleryStory = {
  id: 'delimited-viewer',
  title: 'Tabular viewer — CSV',
  cases: [
    {
      key: 'csv',
      label: 'CSV with toolbar',
      render: () => (
        <div className="w-[36rem]">
          <DelimitedTable text={CSV_TEXT} delimiter="," fileName="data.csv" />
        </div>
      ),
    },
  ],
}

// The real XLSX per-sheet grid, prop-driven.
const xlsxSheetStory: GalleryStory = {
  id: 'xlsx-viewer',
  title: 'Tabular viewer — XLSX sheet',
  cases: [
    {
      key: 'sheet',
      label: 'Sheet with toolbar',
      render: () => (
        <div className="w-[36rem]">
          <XlsxSheet
            fileName="book.xlsx"
            sheet={{
              name: 'Sheet1',
              headers: ['Name', 'Qty', 'Note'],
              rows: [
                ['Banana', '10', 'A short note'],
                ['apple', '2', 'Another'],
                ['Cherry', '30', 'Third'],
              ],
              truncated: false,
            }}
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
    {
      key: 'horizontal',
      label: 'Horizontal',
      render: () => (
        <Menu
          data-testid="g-menu-h"
          aria-label="Top navigation"
          mode="horizontal"
          selectedKey="overview"
          items={[
            { key: 'overview', label: 'Overview' },
            { key: 'activity', label: 'Activity' },
            { key: 'reports', label: 'Reports' },
          ]}
        />
      ),
    },
    {
      key: 'collapsed',
      label: 'Collapsed (icon rail)',
      render: () => (
        <div className="w-14 rounded-md border border-border">
          <Menu
            data-testid="g-menu-collapsed"
            aria-label="Collapsed navigation"
            collapsed
            selectedKey="home"
            items={[
              { key: 'home', label: 'Home', icon: <Home /> },
              { key: 'search', label: 'Search', icon: <Search /> },
              { key: 'settings', label: 'Settings', icon: <Settings /> },
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
  tableActionsStory,
  tableScrollStory,
  delimitedStory,
  xlsxSheetStory,
  listStory,
  descriptionsStory,
  treeStory,
  menuStory,
  paginationStory,
]
