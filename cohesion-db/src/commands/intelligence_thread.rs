use cohesion_core::{
    FromRequest,
    model::{
        ImageBytes, IntelligenceThread, IntelligenceThreadImage, IntelligenceThreadMessage,
        RemoteChain, ThreadItem, ThreadOrigin, ThreadPrompt,
    },
    protocol::{GetIntelligenceThreadRequest, IntelligenceThreadDetail},
    value::IntoValue,
};

use crate::changed::{Changed, Invalidates};
use crate::retention::days_in_secs;
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, Write,
};

/// A thread message with the images the model saw with it.
#[derive(Debug, Clone)]
pub struct ThreadEntry {
    pub message: IntelligenceThreadMessage,
    pub images: Vec<IntelligenceThreadImage>,
}

/// Creates a thread for an agent's conversation or one of its background
/// runs, sharing the stored prompt of any thread that opens the same way.
pub struct CreateIntelligenceThreadCommand {
    pub system_prompt: String,
}
impl DatabaseCommand for CreateIntelligenceThreadCommand {
    type Response = IntelligenceThread;

    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let stored: Option<ThreadPrompt> = ctx
            .db
            .select()
            .filter_eq(ThreadPrompt::PROMPT, self.system_prompt.clone())
            .one()?;
        let prompt = match stored {
            Some(prompt) => prompt,
            None => ctx
                .db
                .insert()
                .set(ThreadPrompt::PROMPT, self.system_prompt)
                .returning_one()
                .found()?,
        };
        let thread: IntelligenceThread = ctx
            .db
            .insert()
            .set(IntelligenceThread::PROMPT_ID, prompt.id)
            .returning_one()
            .found()?;
        Ok(thread.invalidating_nothing())
    }
}

/// Every message of `thread`, oldest first: what it inherits, through each
/// thread it carries on, then its own.
fn messages_of<'c>(
    db: impl ReadableDatabase<'c>,
    thread: &IntelligenceThread,
) -> DatabaseCommandResult<Vec<IntelligenceThreadMessage>> {
    let mut chain = vec![(thread.id, None)];
    let mut origin = thread.origin;
    while let ThreadOrigin::Fork {
        parent_thread_id,
        inherited_through_id,
        ..
    } = origin
    {
        chain.push((parent_thread_id, Some(inherited_through_id)));
        let parent: IntelligenceThread = db
            .select()
            .filter_eq(IntelligenceThread::ID, parent_thread_id)
            .one()
            .found()?;
        origin = parent.origin;
    }
    let mut messages = Vec::new();
    for (thread_id, through) in chain.into_iter().rev() {
        let select = db.select().filter("thread_id = ?").bind(thread_id);
        let select = match through {
            Some(through) => select.filter("id <= ?").bind(through),
            None => select,
        };
        messages.extend(select.order("id ASC").all()?);
    }
    Ok(messages)
}

/// `messages` with the images the model saw with them.
fn with_images<'c>(
    db: impl ReadableDatabase<'c>,
    messages: Vec<IntelligenceThreadMessage>,
) -> DatabaseCommandResult<Vec<ThreadEntry>> {
    let ids = messages
        .iter()
        .map(|message| message.id)
        .collect::<Vec<_>>();
    let mut images: Vec<IntelligenceThreadImage> = db
        .select()
        .filter_in_values("message_id", ids)
        .order("id ASC")
        .all()?;
    Ok(messages
        .into_iter()
        .map(|message| {
            let (own, rest) = images
                .drain(..)
                .partition(|image: &IntelligenceThreadImage| image.message_id == message.id);
            images = rest;
            ThreadEntry {
                message,
                images: own,
            }
        })
        .collect())
}

/// Starts a new thread carrying on thread `thread_id`'s first `items`
/// messages, with its system prompt, and returns it with them, in order. The
/// new thread reads them from the thread that holds them rather than copying
/// them. It carries on the source's stored server-side chain when that chain
/// ends within those messages, so a provider that keeps the conversation
/// branches from it; otherwise its first turn sends them whole.
pub struct ForkIntelligenceThreadCommand {
    pub thread_id: i64,
    pub items: usize,
}
impl DatabaseCommand for ForkIntelligenceThreadCommand {
    type Response = (IntelligenceThread, Vec<ThreadEntry>);
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let source: IntelligenceThread = ctx
            .db
            .select()
            .filter_eq(IntelligenceThread::ID, self.thread_id)
            .one()
            .found()?;
        let mut messages = messages_of(ctx.db.read(), &source)?;
        messages.truncate(self.items);
        let remote = source
            .remote
            .filter(|chain| chain.remote_cursor as usize <= messages.len());
        // The last message's own thread holds everything before it, so a fork
        // of a fork inherits as directly as it can.
        let origin = match messages.last() {
            Some(last) => ThreadOrigin::Fork {
                parent_thread_id: last.thread_id,
                inherited_through_id: last.id,
                inherited_items: messages.len() as i64,
            },
            None => ThreadOrigin::Root,
        };
        let thread: IntelligenceThread = ctx
            .db
            .insert()
            .set(IntelligenceThread::PROMPT_ID, source.prompt_id)
            .set_group(origin)
            .set_group(remote)
            .returning_one()
            .found()?;
        let entries = with_images(ctx.db.read(), messages)?;
        Ok((thread, entries).invalidating_nothing())
    }
}

/// Every message of a thread, inherited ones first, with their images.
pub struct GetIntelligenceThreadMessagesCommand {
    pub thread_id: i64,
}
impl DatabaseCommand for GetIntelligenceThreadMessagesCommand {
    type Response = Vec<ThreadEntry>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let thread = GetIntelligenceThreadCommand {
            thread_id: self.thread_id,
        }
        .handle(ctx)?;
        with_images(ctx.db, messages_of(ctx.db, &thread)?)
    }
}

/// Adds a message to a thread, with the images the model saw with it.
pub struct CreateIntelligenceMessageCommand {
    pub thread_id: i64,
    pub item: ThreadItem,
    pub images: Vec<(String, ImageBytes)>,
}
impl DatabaseCommand for CreateIntelligenceMessageCommand {
    type Response = ();
    type Access = Write;

    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let message: IntelligenceThreadMessage = ctx
            .db
            .insert()
            .set(IntelligenceThreadMessage::THREAD_ID, self.thread_id)
            .set_group(self.item)
            .returning_one()
            .found()?;
        for (media_type, data) in self.images {
            ctx.db
                .insert::<IntelligenceThreadImage>()
                .set(IntelligenceThreadImage::MESSAGE_ID, message.id)
                .set(IntelligenceThreadImage::MEDIA_TYPE, media_type)
                .set(IntelligenceThreadImage::DATA, data)
                .execute()?;
        }
        Ok(().invalidating_nothing())
    }
}

pub struct GetIntelligenceThreadCommand {
    pub thread_id: i64,
}
impl DatabaseCommand for GetIntelligenceThreadCommand {
    type Response = IntelligenceThread;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        ctx.db
            .select()
            .filter_eq(IntelligenceThread::ID, self.thread_id)
            .one()
            .found()
    }
}

pub struct UpdateIntelligenceThreadRemoteCommand {
    pub thread_id: i64,
    pub remote: Option<RemoteChain>,
}
impl DatabaseCommand for UpdateIntelligenceThreadRemoteCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        ctx.db
            .update::<IntelligenceThread>()
            .set_group(self.remote)
            .filter_eq(IntelligenceThread::ID, self.thread_id)
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Deletes the threads nothing has been added to for `days`, with their
/// messages, unless a thread still carries on from one: a fork outlasts its
/// parent by no more than the fork's own idle time, since the next pass takes
/// the parent. Then drops the prompts no thread opens with any more. The runs
/// and cost events that pointed at a thread stay, without their transcript.
pub struct DeleteIdleThreadsCommand {
    pub days: i64,
}
impl DatabaseCommand for DeleteIdleThreadsCommand {
    type Response = ();
    type Access = Write;
    fn handle(
        self,
        ctx: &mut CommandContext<'_>,
    ) -> DatabaseCommandResult<Changed<Self::Response>> {
        let retention = days_in_secs(self.days);
        ctx.db
            .delete::<IntelligenceThread>()
            .filter("created_at <= unixepoch() - ?")
            .bind(retention)
            .filter(
                "NOT EXISTS (SELECT 1 FROM intelligence_thread_messages AS message
                    WHERE message.thread_id = intelligence_threads.id
                    AND message.created_at > unixepoch() - ?)",
            )
            .bind(retention)
            .filter(
                "NOT EXISTS (SELECT 1 FROM intelligence_threads AS fork
                    WHERE fork.parent_thread_id = intelligence_threads.id)",
            )
            .execute()?;
        ctx.db
            .delete::<ThreadPrompt>()
            .filter_not_in(
                "id",
                ctx.db
                    .read()
                    .select_column::<IntelligenceThread, i64>("prompt_id"),
            )
            .execute()?;
        Ok(().invalidating_nothing())
    }
}

/// Empties the thread messages older than `days` and keeps their rows, so a
/// transcript still reads and a tool call keeps its identifier (and a response
/// its success or failure), pairing a request with its response. A message
/// already blank is left alone, so a pass rewrites each row once. A fork reads
/// what it inherits as the thread holding it does now, which a fork started
/// later would have copied emptied anyway.
pub struct BlankOldThreadMessagesCommand {
    pub days: i64,
}
impl DatabaseCommand for BlankOldThreadMessagesCommand {
    type Response = usize;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<usize>> {
        let cutoff = ctx.now - days_in_secs(self.days);
        let mut blanked = 0;
        for (kinds, set, unblank) in [
            ("0, 1, 5", "text = ''", "text <> ''"),
            (
                "2",
                "text = '', signature = NULL, in_content = 0",
                "(text <> '' OR signature IS NOT NULL)",
            ),
            (
                "3",
                "kind = 2, text = '', redacted_trace = NULL, in_content = 0",
                "TRUE",
            ),
            ("4", "arguments = '{}'", "arguments <> '{}'"),
        ] {
            blanked += ctx.db.container().execute_statement(
                &format!(
                    "UPDATE intelligence_thread_messages SET {set}
                     WHERE created_at < ? AND kind IN ({kinds}) AND {unblank}"
                ),
                [&cutoff.into_value()],
            )?;
        }
        ctx.db
            .delete::<IntelligenceThreadImage>()
            .filter(
                "message_id IN (SELECT id FROM intelligence_thread_messages
                    WHERE created_at < ?)",
            )
            .bind(cutoff)
            .execute()?;
        Ok(blanked.invalidating_nothing())
    }
}

/// An intelligence thread with its messages and cost events, oldest first.
#[derive(FromRequest)]
#[request(GetIntelligenceThreadRequest)]
pub struct GetIntelligenceThreadDetailCommand {
    pub thread_id: i64,
}
impl DatabaseCommand for GetIntelligenceThreadDetailCommand {
    type Response = IntelligenceThreadDetail;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        let thread = GetIntelligenceThreadCommand {
            thread_id: self.thread_id,
        }
        .handle(ctx)?;
        let messages = messages_of(ctx.db, &thread)?;
        let cost_events = ctx
            .db
            .select()
            .filter("thread_id = ?")
            .bind(self.thread_id)
            .order("id ASC")
            .all()?;
        Ok(IntelligenceThreadDetail {
            thread,
            messages,
            cost_events,
        })
    }
}

#[cfg(test)]
mod tests;
