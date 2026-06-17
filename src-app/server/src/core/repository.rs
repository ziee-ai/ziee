// Repository Factory - Global repository access pattern
//
// This module uses a declarative macro to eliminate boilerplate while
// preserving type safety and the ergonomic `Repos.module.method()` syntax.
//
// To add a new repository:
// 1. Add one line to the `declare_repositories!` invocation below
// 2. That's it!

use once_cell::sync::OnceCell;
use sqlx::PgPool;
use std::sync::Arc;

/// Declarative macro for repository registration
///
/// This macro generates all the boilerplate needed for repository access:
/// - Imports
/// - RepositoryFactory struct with fields
/// - Getter methods
/// - Wrapper structs with Deref implementations
/// - ReposAccessor struct
/// - Repos constant
/// - Initialization functions
///
/// Reduces ~23 lines of boilerplate per repository to just 1 line.
macro_rules! declare_repositories {
    ($(
        $field:ident: $type:ident => $module_path:path
    ),* $(,)?) => {
        // ============================================
        // SECTION 1: IMPORTS
        // ============================================
        $(
            use $module_path::{ $type };
        )*

        // ============================================
        // SECTION 2: REPOSITORY FACTORY
        // ============================================
        pub struct RepositoryFactory {
            pool: PgPool,
            $(
                $field: OnceCell<Arc<$type>>,
            )*
        }

        impl RepositoryFactory {
            fn new(pool: PgPool) -> Self {
                Self {
                    pool,
                    $(
                        $field: OnceCell::new(),
                    )*
                }
            }

            pub fn pool(&self) -> &PgPool {
                &self.pool
            }

            $(
                pub fn $field(&self) -> Arc<$type> {
                    self.$field
                        .get_or_init(|| Arc::new($type::new(self.pool.clone())))
                        .clone()
                }
            )*
        }

        // ============================================
        // SECTION 2.5: FACTORY INITIALIZATION
        // ============================================
        static FACTORY: OnceCell<RepositoryFactory> = OnceCell::new();

        /// Initialize the global repository factory.
        ///
        /// SECURITY: warns loudly via tracing on double-initialization.
        /// The previous implementation used `set(...).ok()` which silently
        /// swallowed the error — leaving the original pool in place
        /// while the caller assumed the new one had been swapped in.
        /// If the second init carried a different connection string,
        /// the assumed-new behavior would silently revert to the old
        /// pool's behavior, including its credentials. Closes 14-core
        /// F-10 (Medium).
        ///
        /// Test note: the integration test binary calls this once per
        /// TestServer::start (potentially hundreds of times per process),
        /// so we cannot panic on re-init. The first call wins; subsequent
        /// calls log a warning that surfaces the issue in observability
        /// without breaking test isolation.
        pub fn init_repositories(pool: PgPool) {
            if FACTORY.set(RepositoryFactory::new(pool)).is_err() {
                tracing::warn!(
                    "init_repositories called more than once in this process; \
                     subsequent call ignored. In production this signals a \
                     second bootstrap path — investigate."
                );
            }
        }

        fn get_factory() -> &'static RepositoryFactory {
            FACTORY.get().expect("RepositoryFactory not initialized. Call init_repositories() first.")
        }

        /// True if `init_repositories()` has been called and the
        /// global factory is set. Useful for code paths that fire
        /// before the server's startup sequence has finished (e.g.
        /// the desktop tauri crate's auto_start tunnel hook, which
        /// races against the embedded-PostgreSQL boot).
        pub fn is_repos_initialized() -> bool {
            FACTORY.get().is_some()
        }

        // ============================================
        // SECTION 3: WRAPPER STRUCTS (Deref pattern)
        // ============================================
        paste::paste! {
            $(
                pub struct [<$type Repos>];
                impl std::ops::Deref for [<$type Repos>] {
                    type Target = Arc<$type>;
                    fn deref(&self) -> &Self::Target {
                        static INSTANCE: OnceCell<Arc<$type>> = OnceCell::new();
                        INSTANCE.get_or_init(|| get_factory().$field())
                    }
                }
            )*
        }

        // ============================================
        // SECTION 4: REPOS ACCESSOR
        // ============================================
        paste::paste! {
            pub struct ReposAccessor {
                $(
                    pub $field: [<$type Repos>],
                )*
            }
        }

        impl ReposAccessor {
            /// Get the underlying database pool
            ///
            /// For use by modules that haven't been migrated to repository pattern yet.
            /// Prefer using specific repository methods when available.
            pub fn pool(&self) -> &PgPool {
                get_factory().pool()
            }
        }

        // ============================================
        // SECTION 5: GLOBAL CONSTANT
        // ============================================
        paste::paste! {
            /// Global repository accessor
            ///
            /// Provides direct field access to all repositories. All repositories are
            /// lazily initialized and cached for the lifetime of the application.
            ///
            /// # Example
            /// ```
            /// let users = Repos.user.find_all().await?;
            /// let user = Repos.user.find_by_id(user_id).await?;
            /// let groups = Repos.group.find_by_user_id(user_id).await?;
            /// let pool = Repos.pool(); // For modules without repository structs
            /// ```
            #[allow(non_upper_case_globals)]
            pub const Repos: ReposAccessor = ReposAccessor {
                $(
                    $field: [<$type Repos>],
                )*
            };
        }
    };
}

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
    file: FileRepository => crate::modules::file,
    file_rag: FileRagRepository => crate::modules::file_rag,
    project_files: ProjectFilesRepository => crate::modules::file::project_extension,
    chat: ChatRepository => crate::modules::chat::core,
    local_runtime: LocalRuntimeRepository => crate::modules::llm_local_runtime,
    code_sandbox: CodeSandboxRepository => crate::modules::code_sandbox,
    memory: MemoryRepository => crate::modules::memory,
    summarization: SummarizationRepository => crate::modules::summarization,
    assistant_core_memory: AssistantCoreMemoryRepository => crate::modules::assistant_core_memory,
    project: ProjectRepository => crate::modules::project,
    onboarding: OnboardingRepository => crate::modules::onboarding,
    skill: SkillRepository => crate::modules::skill,
    workflow: WorkflowRepository => crate::modules::workflow,
}
