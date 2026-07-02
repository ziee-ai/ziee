//! workflow chat extension (auto-discovered at modules/*/chat-extension/).
//!
//! Registers a `tool_result` content renderer that adds a "Save to my
//! workflows" + "Download .tar.gz" affordance to a `run_from_workspace` result
//! (the LLM-authored-in-sandbox workflow the user just ran). It claims ONLY its
//! own blocks via the renderer's static `contentMatch`, so every other
//! `tool_result` falls through to the file / literature renderers unchanged.

import { createExtension, type ChatExtension } from '@/modules/chat/core/extensions'
import { WorkflowWorkspaceRunCard } from './components/WorkflowWorkspaceRunCard'

const workflowExtension: ChatExtension = createExtension({
  name: 'workflow-workspace',
  description: 'Save/Download affordance for run_from_workspace tool results',
  // Below file (80) + literature (75) so it's tried first, but its
  // `contentMatch` claims only `run_from_workspace` — all other tool_result
  // blocks fall through to the next renderer.
  priority: 74,
  contentTypes: {
    tool_result: WorkflowWorkspaceRunCard,
  },
})

export default workflowExtension
