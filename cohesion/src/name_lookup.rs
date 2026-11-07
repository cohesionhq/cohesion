use cohesion_core::model::{
    ArticleSummary, EndpointModel, FeedSummary, IntelligenceAgent, IntelligenceAgentCue,
    IntelligenceAgentTeam, Tag,
};
use cohesion_core::protocol::RedactedIntelligenceEndpoint;
use cohesion_core::{TaskFolder, Workspace};

use crate::cli_error::CliError;

/// Something commands name by id or by a name, like a workspace's title.
pub trait Named {
    /// What it's called in errors, like "workspace".
    const KIND: &'static str;
    fn id(&self) -> i64;
    fn name(&self) -> &str;
}

impl Named for Workspace {
    const KIND: &'static str = "workspace";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.title
    }
}

impl Named for FeedSummary {
    const KIND: &'static str = "feed";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.title
    }
}

impl Named for ArticleSummary {
    const KIND: &'static str = "article";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.title
    }
}

impl Named for Tag {
    const KIND: &'static str = "tag";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl Named for TaskFolder {
    const KIND: &'static str = "task folder";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.title
    }
}

impl Named for RedactedIntelligenceEndpoint {
    const KIND: &'static str = "endpoint";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

impl Named for EndpointModel {
    const KIND: &'static str = "model";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.model_identifier
    }
}

impl Named for IntelligenceAgent {
    const KIND: &'static str = "agent";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.agent_name
    }
}

impl Named for IntelligenceAgentTeam {
    const KIND: &'static str = "team";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.title
    }
}

impl Named for IntelligenceAgentCue {
    const KIND: &'static str = "cue";
    fn id(&self) -> i64 {
        self.id
    }
    fn name(&self) -> &str {
        &self.name
    }
}

/// Resolves `query` as an id first, then as a name (matched case-insensitively).
pub fn find<'a, T: Named>(items: &'a [T], query: &str) -> Result<&'a T, CliError> {
    let id = query.parse::<i64>().ok();
    if let Some(item) = items.iter().find(|item| Some(item.id()) == id) {
        return Ok(item);
    }
    let matches: Vec<_> = items
        .iter()
        .filter(|item| item.name().eq_ignore_ascii_case(query))
        .collect();
    match matches[..] {
        [item] => Ok(item),
        [] => Err(CliError::UnknownName(T::KIND, query.to_owned())),
        _ => {
            let ids = matches.iter().map(|item| item.id()).collect();
            Err(CliError::AmbiguousName(T::KIND, query.to_owned(), ids))
        }
    }
}
