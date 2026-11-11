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
impl CachePlan {
    /// Where a transcript of `items` items puts the breakpoint of its
    /// shared items, and what the rest asks for. A read lands only where an
    /// earlier request wrote, and the server writes only at the end of the
    /// latest message, which a thread copying the shared items never
    /// sends: it follows them with its own. So the last shared item carries
    /// a breakpoint of its own, whose index is the first answer. Shared
    /// items that end the transcript are cached as its tail is, the
    /// request's own breakpoint landing on the same block: there is no
    /// index, and the tail asks for what the prefix does.
    pub fn shared_breakpoint(&self, items: usize) -> (Option<usize>, CacheLength) {
        if self.shared_items > 0 && self.shared_items >= items {
            (None, self.prefix)
        } else {
            (self.shared_items.checked_sub(1), self.tail)
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(prefix: CacheLength, shared_items: usize, tail: CacheLength) -> CachePlan {
        CachePlan {
            prefix,
            shared_items,
            tail,
        }
    }

    #[test]
    fn no_shared_items_have_no_breakpoint() {
        let plan = plan(CacheLength::Long, 0, CacheLength::Short);
        assert_eq!(plan.shared_breakpoint(0), (None, CacheLength::Short));
        assert_eq!(plan.shared_breakpoint(3), (None, CacheLength::Short));
    }

    #[test]
    fn the_last_shared_item_carries_the_breakpoint() {
        let plan = plan(CacheLength::Long, 2, CacheLength::Short);
        assert_eq!(plan.shared_breakpoint(3), (Some(1), CacheLength::Short));
    }

    #[test]
    fn shared_items_ending_the_transcript_are_cached_as_the_prefix_is() {
        let plan = plan(CacheLength::Long, 2, CacheLength::Short);
        assert_eq!(plan.shared_breakpoint(2), (None, CacheLength::Long));
        assert_eq!(plan.shared_breakpoint(1), (None, CacheLength::Long));
    }
}
