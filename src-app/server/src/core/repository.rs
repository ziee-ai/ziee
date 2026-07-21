// Repository Factory - Global repository access pattern
//
// This module uses a declarative macro to eliminate boilerplate while
// preserving type safety and the ergonomic `Repos.module.method()` syntax.
//
// To add a new repository:
// 1. Add one line to the `declare_repositories!` invocation below
// 2. That's it!

// The `declare_repositories!` macro + its factory machinery moved to
// `ziee-framework` in Chunk B4 (SDK extraction). ziee keeps only the concrete
// repo LIST below; the macro's expansion still generates the `Repos` global,
// `init_repositories`, `is_repos_initialized`, and every accessor IN THIS CRATE,
// so all ~171 `Repos.xxx` call sites are unchanged.
use ziee_framework::declare_repositories;

// ============================================
// REPOSITORY DECLARATIONS
// ============================================
// Add new repositories here as a single line:
//   field_name: RepositoryType => crate::modules::module_name,

declare_repositories! {
    user: UserRepository => crate::modules::user,
    group: GroupRepository => crate::modules::user,
    llm_provider: LlmProviderRepository => crate::modules::llm_provider,
    user_group_llm_provider: UserGroupLlmProviderRepository => crate::modules::llm_provider::user_extension,
    user_key: UserKeyRepository => crate::modules::llm_provider,
    llm_model: LlmModelRepository => crate::modules::llm_model,
    download_instance: DownloadInstanceRepository => crate::modules::llm_model,
    llm_repository: LlmRepositoryRepository => crate::modules::llm_repository,
    assistant: AssistantRepository => crate::modules::assistant,
    hub: HubRepository => crate::modules::hub,
    mcp: McpRepository => crate::modules::mcp,
    mcp_settings: McpSettingsRepository => crate::modules::mcp::settings,
    app: AppRepository => crate::modules::app,
    auth: AuthRepository => crate::modules::auth,
    session_settings: SessionSettingsRepository => crate::modules::auth,
    file: FileRepository => crate::modules::file,
    file_rag: FileRagRepository => crate::modules::file_rag,
    knowledge_base: KnowledgeBaseRepository => crate::modules::knowledge_base,
    project_files: ProjectFilesRepository => crate::modules::file::project_extension,
    chat: ChatRepository => crate::modules::chat::core,
    local_runtime: LocalRuntimeRepository => crate::modules::llm_local_runtime,
    code_sandbox: CodeSandboxRepository => crate::modules::code_sandbox,
    memory: MemoryRepository => crate::modules::memory,
    summarization: SummarizationRepository => crate::modules::summarization,
    agent: AgentRepository => crate::modules::agent,
    assistant_core_memory: AssistantCoreMemoryRepository => crate::modules::assistant_core_memory,
    project: ProjectRepository => crate::modules::project,
    onboarding: OnboardingRepository => crate::modules::onboarding,
    skill: SkillRepository => crate::modules::skill,
    workflow: WorkflowRepository => crate::modules::workflow,
    file_workflow_runs: FileWorkflowRunsRepository => crate::modules::workflow::file_runs,
    web_search: WebSearchRepository => crate::modules::web_search,
    lit_search: LitSearchRepository => crate::modules::lit_search,
    js_tool: JsToolRepository => crate::modules::js_tool,
    voice: VoiceRepository => crate::modules::voice,
    voice_model: VoiceModelRepository => crate::modules::voice,
}
