use cohesion_core::model::CacheLength;

use crate::tool_definition::Tool;
use crate::transcript_item::TranscriptItem;

#[derive(Debug, Default)]
pub struct Transcript {
    pub system_prompt: Option<String>,
    pub remote: Option<RemoteTranscriptState>,
    pub items: Vec<TranscriptItem>,
    pub tools: Vec<Tool>,
    pub cache: CachePlan,
}
impl Transcript {
    pub fn new(system_prompt: String) -> Self {
        Self {
            system_prompt: Some(system_prompt),
            ..Default::default()
        }
    }

    pub fn add(&mut self, item: TranscriptItem) {
        self.items.push(item);
    }
}

/// Which of its endpoint's prompt caches each part of a transcript asks
/// for. The tools and system prompt, which every thread of an agent's role
/// shares, ask for the long one; so do the first `shared_items` items,
/// which other threads copy and read back long after this one ends; the
/// rest asks for `tail`.
#[derive(Debug, Clone, Copy)]
pub struct CachePlan {
    /// What the tools and system prompt ask for: the long cache, except a
    /// background run's, which nothing reads back for long.
    pub prefix: CacheLength,
    pub shared_items: usize,
    pub tail: CacheLength,
}
impl Default for CachePlan {
    /// A thread nothing copies, all of it kept for the long cache, so it
    /// stays warm between turns far apart.
    fn default() -> Self {
        Self {
            prefix: CacheLength::Long,
            shared_items: 0,
            tail: CacheLength::Long,
        }
    }
}

#[derive(Debug)]
pub struct RemoteTranscriptState {
    pub conversation_id: String,
    pub cursor: usize,
}
