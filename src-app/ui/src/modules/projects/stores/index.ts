export { useProjectsStore } from './Projects.store'
export { useProjectDetailStore } from './projectDetail'
export { useProjectDrawerStore } from './ProjectDrawer.store'

// Re-export the Stores proxy for convenient `Stores.Projects.method()` access
// within this module (matches the assistants module pattern).
export { Stores } from '@ziee/framework/stores'
