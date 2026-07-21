import type { StoreSet } from '@ziee/framework/store-kit'
import type { File as FileEntity } from '@/api-client/types'

export const filePreviewDrawerState = {
  isOpen: false,
  file: null as FileEntity | null,
}

export type FilePreviewDrawerState = typeof filePreviewDrawerState
export type FilePreviewDrawerSet = StoreSet<FilePreviewDrawerState>
export type FilePreviewDrawerGet = () => FilePreviewDrawerState
