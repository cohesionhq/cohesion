//! The host objects scripts reach as globals (`cohesion`, `intelligence`,
//! `log`, `network`, `timer`, `services`, `notifications`, in extensions
//! `accounts` and `usage`, in agent sandboxes `agent`,
//! `connectors` and `images`, and in both `actors` and `storage`), each
//! installed on a sandbox when its [`Mount`] is asked for. The agent globals
//! record [`ForegroundEffect`]s for the caller rather than acting
//! themselves.

mod accounts_global;
mod actor_options;
mod actor_session_message;
mod actors_global;
mod agent_effect;
mod agent_global;
mod agent_options;
mod article_fetch_mount;
mod cohesion_global;
mod connectors_global;
mod global;
mod images_global;
mod intelligence_endpoint_options;
mod intelligence_global;
mod logger_global;
mod mount;
mod network_global;
mod network_response;
mod notifications_global;
mod services_global;
mod sleep_mount;
mod storage_global;
mod timer_global;
mod timer_registration;
mod timer_unit;
mod usage_global;

pub use accounts_global::AccountsGlobal;
pub use actor_options::ActorDispatchOptions;
pub use actor_session_message::ActorSessionMessage;
pub use actors_global::ActorsGlobal;
pub(crate) use agent_effect::{AgentEffects, AgentImages};
pub use agent_effect::{Effects, ForegroundEffect};
pub use agent_global::AgentGlobal;
pub use agent_options::AgentCueOptions;
pub use article_fetch_mount::ArticleFetchMount;
pub use cohesion_global::{
    CalendarCreateOptions, CohesionGlobal, CorpusSearchOptions, WorkspaceListOptions,
};
pub use connectors_global::ConnectorsGlobal;
pub use global::Global;
pub use images_global::{ImageOptions, ImagesGlobal};
pub use intelligence_endpoint_options::{
    AccountEndpointOptions, CustomEndpointOptions, EndpointModelOptions, EndpointOptions,
    ModelPriceOptions, ProviderEndpointOptions,
};
pub use intelligence_global::IntelligenceGlobal;
pub use logger_global::LoggerGlobal;
pub use mount::Mount;
pub use network_global::{NetworkBodyEncoding, NetworkFetchOptions, NetworkGlobal, NetworkMethod};
pub use network_response::NetworkResponse;
pub use notifications_global::NotificationsGlobal;
pub use services_global::{ServiceMap, ServicesGlobal};
pub use sleep_mount::SleepMount;
pub(crate) use sleep_mount::{ScopeClosing, sleep};
pub(crate) use storage_global::CurrentStorage;
pub use storage_global::StorageGlobal;
pub use timer_global::{TimerGlobal, TimerOptions};
pub use timer_registration::{TimerRegistration, TimerSchedule};
pub use timer_unit::TimerUnit;
pub use usage_global::{UsageGlobal, UsageOptions};
