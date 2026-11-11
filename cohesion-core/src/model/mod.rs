//! The domain models: every table-backed struct, plus the plain structs and
//! enums they carry.

mod activity;
mod actor;
mod actor_message_columns;
mod agent_agenda_item;
mod agent_message_columns;
mod agent_skill;
mod agent_thread_mark;
mod article;
mod attachment;
mod backlink;
mod catalog_model;
mod client;
mod cohesion_ui;
mod connector;
mod connector_account;
mod contact;
mod day_filter;
mod document;
mod document_folder;
mod document_table;
mod embed;
mod endpoint_model;
mod extension;
mod feed;
mod fetch_columns;
mod hook_kind;
mod image_bytes;
mod intelligence_agent;
mod intelligence_agent_message;
mod intelligence_agent_run;
mod intelligence_cost_event;
mod intelligence_default;
mod intelligence_endpoint;
mod intelligence_provider;
mod intelligence_spend_limit;
mod intelligence_thread;
mod intelligence_usage;
mod item_link;
mod memory;
mod metadata;
mod photo;
mod profile;
mod resource;
mod rewind;
mod saved_task_view;
mod search_result;
mod security;
mod storage_entry;
mod storage_settings;
mod tag;
mod task;
mod task_folder;
mod text_section;
mod workspace;

pub use activity::{Activity, ActivityKind, ActivityOutcome, ActivityStanding, ActivityStatus};
pub use actor::*;
pub use actor_message_columns::{ActorMessageKind, ActorMessageSender};
pub use agent_agenda_item::{AGENDA_ITEM_CHARS, AGENDA_LIMIT, AgentAgendaItem};
pub use agent_message_columns::{
    AgentCodeStatus, AgentMessageKind, AgentReplyStatus, AgentWakeKind,
};
pub use agent_skill::{AgentSkill, AgentSkillScope};
pub use agent_thread_mark::AgentThreadMark;
pub use article::{
    Article, ArticleCapture, ArticleFetchFailure, ArticleFilter, ArticleSnapshot, ArticleSummary,
    Published, PublishedPrecision, fallback_article_title,
};
pub use attachment::{Attachment, AttachmentMessage};
pub use backlink::{Backlink, LinkSource, OutgoingLink, item_links, outgoing_links};
pub use catalog_model::CatalogModel;
pub use client::{Client, ClientKeyKind};
pub use cohesion_ui::{
    Block, CheckboxBlock, Component, HeadingBlock, HeadingLevel, LinkBlock, OverviewBlock,
    TextBlock,
};
pub use connector::{
    AccountDescription, Connector, ConnectorDefinition, ConnectorResource,
    ConnectorResourceFilters, ConnectorResourcePreview, Credential, OrganizationUnit,
    PkceChallenge, ResourceKind, ResourceKindDescription, ServiceDescription,
};
pub use connector_account::{AccountHost, ConnectorAccount};
pub use contact::{Contact, ContactMethod, ContactMethodKind, NewContactMethod};
pub use day_filter::{DayFilter, DayRange};
pub use document::{Document, DocumentType};
pub use document_folder::DocumentFolder;
pub use document_table::{
    DocumentCell, DocumentFolderProperty, DocumentValue, FormulaUpdateMode, PropertyDataType,
    PropertyFormula, PropertyOption, PropertyType, PropertyValue, TableRow,
};
pub use embed::{Embed, EmbedItem, EmbedKind, TABLE_PREVIEW_ROWS, excerpt};
pub use endpoint_model::EndpointModel;
pub use extension::{
    Extension, ExtensionHook, ExtensionManifest, ExtensionPermission, ExtensionPermissionKind,
    ExtensionService, ExtensionServiceFunction, InstalledService, InstalledServiceFunction,
    RegistryResult, ServiceFunctionManifest, ServiceManifest,
};
pub use feed::{
    Feed, FeedCapture, FeedCorpusHit, FeedFullPagePolicy, FeedItemCapture, FeedItemUrl, FeedStatus,
    FeedSummary,
};
pub use hook_kind::HookKind;
pub use image_bytes::ImageBytes;
pub use intelligence_agent::*;
pub use intelligence_agent_message::*;
pub use intelligence_agent_run::*;
pub use intelligence_cost_event::*;
pub use intelligence_default::*;
pub use intelligence_endpoint::*;
pub use intelligence_provider::*;
pub use intelligence_spend_limit::{IntelligenceSpend, IntelligenceSpendLimit, SpendPeriod};
pub use intelligence_thread::*;
pub use intelligence_usage::{IntelligenceUsage, ModelUsage};
pub use item_link::{ItemLink, LinkKind, LinkTarget, NotAnItemLink};
pub use memory::{Memory, MemoryScope};
pub use metadata::{HasMetadata, Metadata, MetadataOwner};
pub use photo::Photo;
pub use profile::Profile;
pub use resource::{
    MetadataKey, Resource, ResourceContentKind, ResourceSort, WorkspaceResourceKind,
};
pub use rewind::{
    DeletedItem, RestoredItem, RewindWriter, Version, VersionAuthor, VersionChange, VersionSummary,
    VersionTarget, VersionedItem,
};
pub use saved_task_view::{
    SavedTaskView, SavedTaskViewPriority, SavedTaskViewSize, SavedTaskViewTag, TagMatch,
    TaskPriorityList, TaskSizeList, TaskViewList, TaskViewScope,
};
pub use search_result::{SearchEntityKind, SearchResult};
pub use security::{PairingOpener, SecurityEvent, SecurityEventKind, SecuritySettings};
pub use storage_entry::{StorageEntry, StorageOwner};
pub use storage_settings::StorageSettings;
pub use tag::{Tag, TagFilter, TagIds, TaskTag};
pub use task::{
    Task, TaskAssignee, TaskFolderFilter, TaskPriority, TaskSize, TaskSort, TaskStatus,
};
pub use task_folder::TaskFolder;
pub use text_section::{TextPiece, TextSection, text_pieces};
pub use workspace::Workspace;
