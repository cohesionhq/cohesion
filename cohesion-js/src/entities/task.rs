//! `Task` in scripts.

use cohesion_core::model::VersionTarget;
use cohesion_core::{Task, TaskPriority, TaskStatus};
use cohesion_db::{
    GetTaskFolderCommand, ListTagsCommand, ListVersionsCommand, MoveTaskToFolderCommand,
    SetTaggedCommand,
    commands::{
        ChangeTaskAssigneeCommand, ChangeTaskDescriptionCommand, ChangeTaskPriorityCommand,
        ChangeTaskSizeCommand, ChangeTaskStatusCommand, ChangeTaskTitleCommand,
        ScheduleTaskCommand,
    },
};
use cohesion_macros::js_entity;
use rquickjs::{Class, Ctx, Exception, Value};

use crate::conversion::FromJsField;
use crate::entities::rewind::JsVersionSummary;
use crate::entities::tag::{JsTag, find_or_create_tag, find_tag};
use crate::entities::task_folder::JsTaskFolder;

/// A to-do item in a workspace.
///
/// Get them from `Workspace.createTask()` or `SearchResult.open()`.
///
/// Link one in a message as `[Fix login](cohesion://task/3/42)`, its title,
/// `workspaceId` and `id`; the user opens it with a tap.
#[js_entity(Task, metadata)]
impl JsTask {
    /// Assigning saves immediately, e.g. `task.status = TaskStatus.completed`
    /// (or the string `"completed"`).
    ///
    /// Any other value throws a `TypeError`.
    #[qjs(set, rename = "status")]
    pub fn set_status<'js>(&mut self, ctx: Ctx<'js>, status: String) -> rquickjs::Result<()> {
        let parsed = TaskStatus::from_str_strict(&status)
            .map_err(|error| Exception::throw_type(&ctx, &error.to_string()))?;
        self.inner = self.write(
            &ctx,
            ChangeTaskStatusCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                status: parsed,
            },
        )?;
        Ok(())
    }
    /// Assigning saves immediately, e.g. `task.priority = TaskPriority.high`
    /// (or the string `"high"`).
    ///
    /// Any other value throws a `TypeError`.
    #[qjs(set, rename = "priority")]
    pub fn set_priority<'js>(&mut self, ctx: Ctx<'js>, priority: String) -> rquickjs::Result<()> {
        let parsed = TaskPriority::from_str_strict(&priority)
            .map_err(|error| Exception::throw_type(&ctx, &error.to_string()))?;
        self.inner = self.write(
            &ctx,
            ChangeTaskPriorityCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                priority: parsed,
            },
        )?;
        Ok(())
    }
    /// Assigning saves immediately, e.g. `task.size = 2`; `null` clears it.
    ///
    /// Anything but a whole number from 1 to 5 or `null` throws a
    /// `TypeError`.
    #[qjs(set, rename = "size")]
    pub fn set_size<'js>(&mut self, ctx: Ctx<'js>, size: Value<'js>) -> rquickjs::Result<()> {
        let size = Option::from_js_field(&ctx, size)?;
        self.inner = self.write(
            &ctx,
            ChangeTaskSizeCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                size,
            },
        )?;
        Ok(())
    }
    /// Assigning saves immediately, e.g. `task.assignee = { kind: "user" }`,
    /// `{ kind: "agent", agentId: agent.id }` or
    /// `{ kind: "contact", contactId: contact.id }`; `null` leaves it
    /// unassigned.
    ///
    /// Any other `kind` throws a `TypeError`, and so does an agent that
    /// doesn't exist or a contact that isn't a live one of the task's
    /// workspace.
    #[qjs(set, rename = "assignee")]
    pub fn set_assignee<'js>(
        &mut self,
        ctx: Ctx<'js>,
        assignee: Value<'js>,
    ) -> rquickjs::Result<()> {
        let assignee = Option::from_js_field(&ctx, assignee)?;
        self.inner = self.write(
            &ctx,
            ChangeTaskAssigneeCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                assignee,
            },
        )?;
        Ok(())
    }
    /// Assigning saves immediately; `""` clears it.
    #[qjs(set, rename = "description")]
    pub fn set_description<'js>(
        &mut self,
        ctx: Ctx<'js>,
        description: String,
    ) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            ChangeTaskDescriptionCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                description,
            },
        )?;
        Ok(())
    }
    /// Assigning saves immediately, e.g. `task.dueDate = "2026-10-20"`; `null`
    /// clears it.
    ///
    /// A day is `"YYYY-MM-DD"`; any other text throws.
    #[qjs(set, rename = "startDate")]
    pub fn set_start_date<'js>(
        &mut self,
        ctx: Ctx<'js>,
        start_date: Option<String>,
    ) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            ScheduleTaskCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                start_date,
                due_date: self.inner.due_date.clone(),
            },
        )?;
        Ok(())
    }
    /// Assigning saves immediately, e.g. `task.dueDate = "2026-10-20"`; `null`
    /// clears it.
    ///
    /// A day is `"YYYY-MM-DD"`; any other text throws.
    #[qjs(set, rename = "dueDate")]
    pub fn set_due_date<'js>(
        &mut self,
        ctx: Ctx<'js>,
        due_date: Option<String>,
    ) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            ScheduleTaskCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                start_date: self.inner.start_date.clone(),
                due_date,
            },
        )?;
        Ok(())
    }
    #[qjs(set, rename = "title")]
    pub fn set_title<'js>(&mut self, ctx: Ctx<'js>, title: String) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            ChangeTaskTitleCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                title,
            },
        )?;
        Ok(())
    }
    pub fn cancel<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            ChangeTaskStatusCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                status: TaskStatus::Cancelled,
            },
        )?;
        Ok(())
    }
    pub fn complete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            ChangeTaskStatusCommand {
                workspace_id: self.inner.workspace_id,
                task_id: self.inner.id,
                status: TaskStatus::Completed,
            },
        )?;
        Ok(())
    }

    /// This task's kept versions, newest first: what it held, when, and who
    /// changed it. Restoring one undoes changes; a deleted task has them too.
    pub fn versions<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsVersionSummary>> {
        let versions = self.read(
            &ctx,
            ListVersionsCommand {
                workspace_id: self.inner.workspace_id,
                item: VersionTarget::Task {
                    task_id: self.inner.id,
                },
            },
        )?;
        Ok(versions
            .into_iter()
            .map(|version| JsVersionSummary::new(version, self.db.clone()))
            .collect())
    }
    pub fn folder<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Option<JsTaskFolder>> {
        match self.inner.task_folder_id {
            Some(folder_id) => {
                let value = self.read(
                    &ctx,
                    GetTaskFolderCommand {
                        workspace_id: self.inner.workspace_id,
                        folder_id,
                    },
                )?;
                Ok(Some(JsTaskFolder::new(value, self.db.clone())))
            }
            None => Ok(None),
        }
    }

    #[qjs(rename = "moveTo")]
    pub fn move_folder<'js>(
        &mut self,
        ctx: Ctx<'js>,
        folder: Option<Class<'js, JsTaskFolder>>,
    ) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            MoveTaskToFolderCommand {
                task_id: self.inner.id,
                workspace_id: self.inner.workspace_id,
                folder_id: folder.map(|folder| folder.borrow().inner().id),
            },
        )?;
        Ok(())
    }

    /// The tags on the task, by name.
    pub fn tags<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsTag>> {
        let tags = self.read(
            &ctx,
            ListTagsCommand {
                workspace_id: self.inner.workspace_id,
            },
        )?;
        Ok(tags
            .into_iter()
            .filter(|tag| self.inner.tag_ids.0.contains(&tag.id))
            .map(|tag| JsTag::new(tag, self.db.clone()))
            .collect())
    }

    /// Puts the workspace's tag called `name` on the task, matching the
    /// name ignoring case and creating the tag when the workspace has none by
    /// that name.
    ///
    /// Tagging a task that already has it changes nothing. Not available in
    /// read-only contexts.
    pub fn tag<'js>(&mut self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
        let tag = find_or_create_tag(&ctx, &self.db, self.inner.workspace_id, name)?;
        self.set_tagged(&ctx, tag.id, true)
    }

    /// Takes the workspace's tag called `name` (ignoring case) off the
    /// task; does nothing when the task doesn't have it.
    ///
    /// The tag itself stays in the workspace. Not available in read-only
    /// contexts.
    pub fn untag<'js>(&mut self, ctx: Ctx<'js>, name: String) -> rquickjs::Result<()> {
        match find_tag(&ctx, &self.db, self.inner.workspace_id, &name)? {
            Some(tag) => self.set_tagged(&ctx, tag.id, false),
            None => crate::traced_command::ensure_writable(&ctx, &self.db),
        }
    }
}

impl JsTask {
    fn set_tagged(&mut self, ctx: &Ctx<'_>, tag_id: i64, tagged: bool) -> rquickjs::Result<()> {
        self.inner = self.write(
            ctx,
            SetTaggedCommand {
                workspace_id: self.inner.workspace_id,
                tag_id,
                task_id: self.inner.id,
                tagged,
            },
        )?;
        Ok(())
    }
}
