import type { Assistant } from '@/api-client/types'
import type { TemplateAssistantsGet } from '../state'

export default (_set: never, get: TemplateAssistantsGet) => {
  (): Assistant | undefined =>
    get().assistants.find(a => a.is_default)
}
