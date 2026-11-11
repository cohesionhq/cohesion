//! An agent's agenda: the steps it means to take, in order. Its tools add
//! items, finish one and clear them all; each change posts an `agenda` line
//! to its feed in the same transaction, naming the items it changed.

use cohesion_core::{
    FromRequest,
    invalidation::Invalidation,
    model::{
        AGENDA_ITEM_CHARS, AGENDA_LIMIT, AgentAgendaChange, AgentAgendaItem, AgentAgendaUpdate,
        AgentMessageContent, IntelligenceAgentMessageSender,
    },
    protocol::ListAgentAgendaRequest,
};

use super::CreateAgentMessageCommand;
use crate::changed::{Changed, Invalidates};
use crate::{
    CommandContext, DatabaseCommand, DatabaseCommandResult, Filtered, Found, Read,
    ReadableDatabase, ValidationError, Write,
};

/// What a change to an agenda left: the agenda in order, and the change as
/// its `agenda` line tells it.
#[derive(Debug)]
pub struct AgendaEdit {
    pub agenda: Vec<AgentAgendaItem>,
    pub update: AgentAgendaUpdate,
}

/// Agent `agent_id`'s agenda, in order, finished items included.
#[derive(FromRequest)]
#[request(ListAgentAgendaRequest)]
pub struct ListAgentAgendaCommand {
    pub agent_id: i64,
}
impl DatabaseCommand for ListAgentAgendaCommand {
    type Response = Vec<AgentAgendaItem>;
    type Access = Read;
    fn handle(self, ctx: &mut CommandContext<'_, Read>) -> DatabaseCommandResult<Self::Response> {
        Ok(agenda(ctx.db, self.agent_id)?)
    }
}

/// Adds `items` to agent `agent_id`'s agenda at `position` (from 1; the
/// items there move down), or at its end. Each item is one line of at most
/// [`AGENDA_ITEM_CHARS`] characters, and the agenda holds at most
/// [`AGENDA_LIMIT`], finished ones included. The line it posts is marked
/// with `run_id`, the fork that added them, if one did.
pub struct AppendAgentAgendaCommand {
    pub agent_id: i64,
    pub run_id: Option<i64>,
    pub items: Vec<String>,
    pub position: Option<i64>,
}
impl DatabaseCommand for AppendAgentAgendaCommand {
    type Response = AgendaEdit;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<AgendaEdit>> {
        let items = self
            .items
            .iter()
            .map(|item| item.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect::<Vec<_>>();
        if items.is_empty() {
            return Err(ValidationError::new("Give at least one item to add.").into());
        }
        if items.iter().any(String::is_empty) {
            return Err(ValidationError::new("An agenda item needs some text.").into());
        }
        if items
            .iter()
            .any(|item| item.chars().count() > AGENDA_ITEM_CHARS)
        {
            return Err(ValidationError::in_field(
                "items",
                format!("An agenda item holds at most {AGENDA_ITEM_CHARS} characters."),
            )
            .into());
        }
        let current = agenda(ctx.db, self.agent_id)?;
        if current.len() + items.len() > AGENDA_LIMIT {
            return Err(ValidationError::in_field(
                "items",
                format!(
                "The agenda holds at most {AGENDA_LIMIT} items, finished ones included, and has \
                 {}. Reset it and add what's left.",
                current.len()
            ),
            )
            .into());
        }
        let end = current.len() as i64 + 1;
        let at = self.position.unwrap_or(end);
        if !(1..=end).contains(&at) {
            return Err(ValidationError::in_field(
                "position",
                format!(
                    "Position {at} is outside the agenda, which has {} items; use 1 to {end}.",
                    current.len()
                ),
            )
            .into());
        }
        // The items from `at` on move down past the new ones.
        let added = items.len() as i64;
        for item in current.iter().rev().filter(|item| item.position >= at) {
            ctx.db
                .update::<AgentAgendaItem>()
                .set(AgentAgendaItem::POSITION, item.position + added)
                .filter_eq(AgentAgendaItem::ID, item.id)
                .execute()?;
        }
        let mut new_ids = Vec::new();
        for (offset, text) in items.into_iter().enumerate() {
            let item: AgentAgendaItem = ctx
                .db
                .insert()
                .set(AgentAgendaItem::AGENT_ID, self.agent_id)
                .set(AgentAgendaItem::POSITION, at + offset as i64)
                .set(AgentAgendaItem::TEXT, text)
                .set_now(AgentAgendaItem::ADDED_AT)
                .returning_one()
                .found()?;
            new_ids.push(item.id);
        }
        let agenda = agenda(ctx.db, self.agent_id)?;
        let update = AgentAgendaUpdate {
            change: AgentAgendaChange::Appended,
            items: texts(&agenda, &new_ids),
        };
        post(ctx, self.agent_id, self.run_id, agenda, update)
    }
}

/// Finishes the item at `position` (from 1) of agent `agent_id`'s agenda,
/// or else its first open one. The line it posts is marked with `run_id`,
/// the fork that finished it, if one did.
pub struct FinishAgentAgendaItemCommand {
    pub agent_id: i64,
    pub run_id: Option<i64>,
    pub position: Option<i64>,
}
impl DatabaseCommand for FinishAgentAgendaItemCommand {
    type Response = AgendaEdit;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<AgendaEdit>> {
        let current = agenda(ctx.db, self.agent_id)?;
        let item = match self.position {
            None => current
                .iter()
                .find(|item| item.finished_at.is_none())
                .ok_or_else(|| ValidationError::new("Every item on the agenda is finished."))?,
            Some(position) => {
                let item = current
                    .iter()
                    .find(|item| item.position == position)
                    .ok_or_else(|| {
                        ValidationError::in_field(
                            "position",
                            format!(
                                "The agenda has no item {position}; it has {}.",
                                current.len()
                            ),
                        )
                    })?;
                if item.finished_at.is_some() {
                    return Err(ValidationError::in_field(
                        "position",
                        format!("Item {position} is already finished."),
                    )
                    .into());
                }
                item
            }
        };
        ctx.db
            .update::<AgentAgendaItem>()
            .set_now(AgentAgendaItem::FINISHED_AT)
            .filter_eq(AgentAgendaItem::ID, item.id)
            .execute()?;
        let finished = item.id;
        let agenda = agenda(ctx.db, self.agent_id)?;
        let update = AgentAgendaUpdate {
            change: AgentAgendaChange::Finished,
            items: texts(&agenda, &[finished]),
        };
        post(ctx, self.agent_id, self.run_id, agenda, update)
    }
}

/// Clears agent `agent_id`'s agenda. The line it posts names what was on
/// it, marked with `run_id`, the fork that cleared it, if one did; an
/// empty agenda posts none.
pub struct ResetAgentAgendaCommand {
    pub agent_id: i64,
    pub run_id: Option<i64>,
}
impl DatabaseCommand for ResetAgentAgendaCommand {
    type Response = AgendaEdit;
    type Access = Write;
    fn handle(self, ctx: &mut CommandContext<'_>) -> DatabaseCommandResult<Changed<AgendaEdit>> {
        let cleared = agenda(ctx.db, self.agent_id)?;
        let ids: Vec<i64> = cleared.iter().map(|item| item.id).collect();
        let update = AgentAgendaUpdate {
            change: AgentAgendaChange::Reset,
            items: texts(&cleared, &ids),
        };
        if cleared.is_empty() {
            return Ok(AgendaEdit {
                agenda: Vec::new(),
                update,
            }
            .invalidating_nothing());
        }
        ctx.db
            .delete::<AgentAgendaItem>()
            .filter_eq(AgentAgendaItem::AGENT_ID, self.agent_id)
            .execute()?;
        post(ctx, self.agent_id, self.run_id, Vec::new(), update)
    }
}

/// Agent `agent_id`'s agenda, in order.
fn agenda<'c>(
    db: impl ReadableDatabase<'c>,
    agent_id: i64,
) -> Result<Vec<AgentAgendaItem>, crate::DatabaseError> {
    db.select::<AgentAgendaItem>()
        .filter_eq(AgentAgendaItem::AGENT_ID, agent_id)
        .order("position")
        .all()
}

/// The text of the items of `agenda` that `changed` names, in order.
fn texts(agenda: &[AgentAgendaItem], changed: &[i64]) -> Vec<String> {
    agenda
        .iter()
        .filter(|item| changed.contains(&item.id))
        .map(|item| item.text.clone())
        .collect()
}

/// Posts `update` as an `agenda` line in agent `agent_id`'s feed, marked
/// with `run_id`, and answers with `agenda`, the agenda it left.
fn post(
    ctx: &mut CommandContext<'_>,
    agent_id: i64,
    run_id: Option<i64>,
    agenda: Vec<AgentAgendaItem>,
    update: AgentAgendaUpdate,
) -> DatabaseCommandResult<Changed<AgendaEdit>> {
    ctx.run(CreateAgentMessageCommand {
        agent_id,
        run_id,
        run_description: None,
        sender: IntelligenceAgentMessageSender::Agent,
        content: AgentMessageContent::Agenda(update.clone()),
        reply_to: None,
    })?;
    Ok(AgendaEdit { agenda, update }.invalidating([Invalidation::Agent(agent_id)]))
}

#[cfg(test)]
mod tests;
