/**
 * File Dialog Module
 *
 * Desktop-specific module for native file dialogs
 */

import { createModule, type AppModule } from '@ziee/ui-core'
import { useFileDialogStore } from './store'

const fileDialogModule: AppModule = createModule({
  metadata: {
    name: 'file-dialog',
    version: '1.0.0',
    description: 'Native file dialogs for desktop',
  },

  routes: [],

  stores: [
    {
      name: 'FileDialog',
      store: useFileDialogStore,
    },
  ],

  sidebar: undefined,

  initialize: () => {
    console.log('File dialog module initialized')
  },
})

export default fileDialogModule
