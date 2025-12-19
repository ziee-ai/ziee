//! Desktop Repository System
//!
//! Provides the `DesktopRepos` global accessor for desktop-specific repositories.
//! Reuses the server's PgPool but maintains separate repository instances.
//!
//! To add a new repository:
//! 1. Create the repository struct in `repositories/` module
//! 2. Add one line to the `declare_desktop_repositories!` invocation below

use once_cell::sync::OnceCell;
use sqlx::PgPool;
use std::sync::Arc;

/// Declarative macro for desktop repository registration
///
/// This macro generates all the boilerplate needed for repository access:
/// - DesktopRepositoryFactory struct with fields
/// - Getter methods
/// - Wrapper structs with Deref implementations
/// - DesktopReposAccessor struct
/// - DesktopRepos constant
/// - Initialization function
macro_rules! declare_desktop_repositories {
    ($( $field:ident: $type:ident $(=> $module_path:path)? ),* $(,)?) => {
        // ============================================
        // SECTION 1: IMPORTS (if module paths provided)
        // ============================================
        $(
            $(
                use $module_path::{ $type };
            )?
        )*

        // ============================================
        // SECTION 2: REPOSITORY FACTORY
        // ============================================
        pub struct DesktopRepositoryFactory {
            pool: PgPool,
            $(
                $field: OnceCell<Arc<$type>>,
            )*
        }

        impl DesktopRepositoryFactory {
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
                #[allow(dead_code)]
                pub fn $field(&self) -> Arc<$type> {
                    self.$field
                        .get_or_init(|| Arc::new($type::new(self.pool.clone())))
                        .clone()
                }
            )*
        }

        // ============================================
        // SECTION 3: FACTORY INITIALIZATION
        // ============================================
        static DESKTOP_FACTORY: OnceCell<DesktopRepositoryFactory> = OnceCell::new();

        /// Initialize the desktop repository factory with the server's pool
        pub fn init_desktop_repositories(pool: PgPool) {
            if DESKTOP_FACTORY.set(DesktopRepositoryFactory::new(pool)).is_err() {
                tracing::warn!("DesktopRepositoryFactory already initialized");
            } else {
                tracing::info!("DesktopRepositoryFactory initialized");
            }
        }

        fn get_desktop_factory() -> &'static DesktopRepositoryFactory {
            DESKTOP_FACTORY.get().expect("DesktopRepositoryFactory not initialized. Call init_desktop_repositories() first.")
        }

        /// Check if desktop repositories are initialized
        pub fn is_desktop_repos_initialized() -> bool {
            DESKTOP_FACTORY.get().is_some()
        }

        // ============================================
        // SECTION 4: WRAPPER STRUCTS (Deref pattern)
        // ============================================
        paste::paste! {
            $(
                pub struct [<$type Repos>];
                impl std::ops::Deref for [<$type Repos>] {
                    type Target = Arc<$type>;
                    fn deref(&self) -> &Self::Target {
                        static INSTANCE: OnceCell<Arc<$type>> = OnceCell::new();
                        INSTANCE.get_or_init(|| get_desktop_factory().$field())
                    }
                }
            )*
        }

        // ============================================
        // SECTION 5: REPOS ACCESSOR
        // ============================================
        paste::paste! {
            pub struct DesktopReposAccessor {
                $(
                    pub $field: [<$type Repos>],
                )*
            }
        }

        impl DesktopReposAccessor {
            /// Get the underlying database pool
            pub fn pool(&self) -> &PgPool {
                get_desktop_factory().pool()
            }
        }

        // ============================================
        // SECTION 6: GLOBAL CONSTANT
        // ============================================
        paste::paste! {
            /// Global desktop repository accessor
            ///
            /// Provides direct field access to all desktop repositories.
            /// All repositories are lazily initialized and cached.
            ///
            /// # Example
            /// ```
            /// let setting = DesktopRepos.settings.get("theme").await?;
            /// let pool = DesktopRepos.pool(); // Direct pool access
            /// ```
            #[allow(non_upper_case_globals)]
            pub const DesktopRepos: DesktopReposAccessor = DesktopReposAccessor {
                $(
                    $field: [<$type Repos>],
                )*
            };
        }
    };
}

// ============================================
// DESKTOP REPOSITORY DECLARATIONS
// ============================================
// Add new repositories here as a single line:
//   field_name: RepositoryType => crate::core::repositories::module_name,

declare_desktop_repositories! {
    settings: SettingsRepository => crate::core::repositories::settings,
}
