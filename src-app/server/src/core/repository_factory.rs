// Repository Factory - Global repository access pattern
//
// Provides a global Repos accessor for clean, consistent repository access across the codebase.
// Similar to the frontend's Stores pattern for architectural symmetry.

use once_cell::sync::OnceCell;
use sqlx::PgPool;
use std::sync::Arc;

// Import all repository types
use crate::modules::user::{GroupRepository, UserRepository};
use crate::modules::llm_provider::LlmProviderRepository;
use crate::modules::llm_model::{DownloadInstanceRepository, LlmModelRepository};
use crate::modules::llm_repository::LlmRepositoryRepository;
use crate::modules::assistant::AssistantRepository;
use crate::modules::hub::HubRepository;
use crate::modules::mcp::McpRepository;
use crate::modules::app::AppRepository;
use crate::modules::auth::AuthRepository;

static FACTORY: OnceCell<RepositoryFactory> = OnceCell::new();

/// Central repository factory with lazy initialization
pub struct RepositoryFactory {
    pool: PgPool,
    user: OnceCell<Arc<UserRepository>>,
    group: OnceCell<Arc<GroupRepository>>,
    llm_provider: OnceCell<Arc<LlmProviderRepository>>,
    llm_model: OnceCell<Arc<LlmModelRepository>>,
    download_instance: OnceCell<Arc<DownloadInstanceRepository>>,
    llm_repository: OnceCell<Arc<LlmRepositoryRepository>>,
    assistant: OnceCell<Arc<AssistantRepository>>,
    hub: OnceCell<Arc<HubRepository>>,
    mcp: OnceCell<Arc<McpRepository>>,
    app: OnceCell<Arc<AppRepository>>,
    auth: OnceCell<Arc<AuthRepository>>,
}

impl RepositoryFactory {
    fn new(pool: PgPool) -> Self {
        Self {
            pool,
            user: OnceCell::new(),
            group: OnceCell::new(),
            llm_provider: OnceCell::new(),
            llm_model: OnceCell::new(),
            download_instance: OnceCell::new(),
            llm_repository: OnceCell::new(),
            assistant: OnceCell::new(),
            hub: OnceCell::new(),
            mcp: OnceCell::new(),
            app: OnceCell::new(),
            auth: OnceCell::new(),
        }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub fn user(&self) -> Arc<UserRepository> {
        self.user
            .get_or_init(|| Arc::new(UserRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn group(&self) -> Arc<GroupRepository> {
        self.group
            .get_or_init(|| Arc::new(GroupRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn llm_provider(&self) -> Arc<LlmProviderRepository> {
        self.llm_provider
            .get_or_init(|| Arc::new(LlmProviderRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn llm_model(&self) -> Arc<LlmModelRepository> {
        self.llm_model
            .get_or_init(|| Arc::new(LlmModelRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn download_instance(&self) -> Arc<DownloadInstanceRepository> {
        self.download_instance
            .get_or_init(|| Arc::new(DownloadInstanceRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn llm_repository(&self) -> Arc<LlmRepositoryRepository> {
        self.llm_repository
            .get_or_init(|| Arc::new(LlmRepositoryRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn assistant(&self) -> Arc<AssistantRepository> {
        self.assistant
            .get_or_init(|| Arc::new(AssistantRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn hub(&self) -> Arc<HubRepository> {
        self.hub
            .get_or_init(|| Arc::new(HubRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn mcp(&self) -> Arc<McpRepository> {
        self.mcp
            .get_or_init(|| Arc::new(McpRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn app(&self) -> Arc<AppRepository> {
        self.app
            .get_or_init(|| Arc::new(AppRepository::new(self.pool.clone())))
            .clone()
    }

    pub fn auth(&self) -> Arc<AuthRepository> {
        self.auth
            .get_or_init(|| Arc::new(AuthRepository::new(self.pool.clone())))
            .clone()
    }
}

/// Initialize the global repository factory
pub fn init_repositories(pool: PgPool) {
    FACTORY.set(RepositoryFactory::new(pool)).ok();
}

fn get_factory() -> &'static RepositoryFactory {
    FACTORY.get().expect("RepositoryFactory not initialized. Call init_repositories() first.")
}

// Wrapper structs for Deref pattern (enables clean Repos.user.method() syntax)

pub struct UserRepos;
impl std::ops::Deref for UserRepos {
    type Target = Arc<UserRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<UserRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().user())
    }
}

pub struct GroupRepos;
impl std::ops::Deref for GroupRepos {
    type Target = Arc<GroupRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<GroupRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().group())
    }
}

pub struct LlmProviderRepos;
impl std::ops::Deref for LlmProviderRepos {
    type Target = Arc<LlmProviderRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<LlmProviderRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().llm_provider())
    }
}

pub struct LlmModelRepos;
impl std::ops::Deref for LlmModelRepos {
    type Target = Arc<LlmModelRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<LlmModelRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().llm_model())
    }
}

pub struct DownloadInstanceRepos;
impl std::ops::Deref for DownloadInstanceRepos {
    type Target = Arc<DownloadInstanceRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<DownloadInstanceRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().download_instance())
    }
}

pub struct LlmRepositoryRepos;
impl std::ops::Deref for LlmRepositoryRepos {
    type Target = Arc<LlmRepositoryRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<LlmRepositoryRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().llm_repository())
    }
}

pub struct AssistantRepos;
impl std::ops::Deref for AssistantRepos {
    type Target = Arc<AssistantRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<AssistantRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().assistant())
    }
}

pub struct HubRepos;
impl std::ops::Deref for HubRepos {
    type Target = Arc<HubRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<HubRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().hub())
    }
}

pub struct McpRepos;
impl std::ops::Deref for McpRepos {
    type Target = Arc<McpRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<McpRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().mcp())
    }
}

pub struct AppRepos;
impl std::ops::Deref for AppRepos {
    type Target = Arc<AppRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<AppRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().app())
    }
}

pub struct AuthRepos;
impl std::ops::Deref for AuthRepos {
    type Target = Arc<AuthRepository>;
    fn deref(&self) -> &Self::Target {
        static INSTANCE: OnceCell<Arc<AuthRepository>> = OnceCell::new();
        INSTANCE.get_or_init(|| get_factory().auth())
    }
}

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
#[allow(dead_code)]
pub struct ReposAccessor {
    pub user: UserRepos,
    pub group: GroupRepos,
    pub llm_provider: LlmProviderRepos,
    pub llm_model: LlmModelRepos,
    pub download_instance: DownloadInstanceRepos,
    pub llm_repository: LlmRepositoryRepos,
    pub assistant: AssistantRepos,
    pub hub: HubRepos,
    pub mcp: McpRepos,
    pub app: AppRepos,
    pub auth: AuthRepos,
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

/// Global constant instance for repository access
#[allow(non_upper_case_globals)]
pub const Repos: ReposAccessor = ReposAccessor {
    user: UserRepos,
    group: GroupRepos,
    llm_provider: LlmProviderRepos,
    llm_model: LlmModelRepos,
    download_instance: DownloadInstanceRepos,
    llm_repository: LlmRepositoryRepos,
    assistant: AssistantRepos,
    hub: HubRepos,
    mcp: McpRepos,
    app: AppRepos,
    auth: AuthRepos,
};
