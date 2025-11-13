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

        /// Initialize the global repository factory
        pub fn init_repositories(pool: PgPool) {
            FACTORY.set(RepositoryFactory::new(pool)).ok();
        }

        fn get_factory() -> &'static RepositoryFactory {
            FACTORY.get().expect("RepositoryFactory not initialized. Call init_repositories() first.")
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
            #[allow(dead_code)]
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
    llm_model: LlmModelRepository => crate::modules::llm_model,
    download_instance: DownloadInstanceRepository => crate::modules::llm_model,
    llm_repository: LlmRepositoryRepository => crate::modules::llm_repository,
    assistant: AssistantRepository => crate::modules::assistant,
    hub: HubRepository => crate::modules::hub,
    mcp: McpRepository => crate::modules::mcp,
    app: AppRepository => crate::modules::app,
    auth: AuthRepository => crate::modules::auth,
}
