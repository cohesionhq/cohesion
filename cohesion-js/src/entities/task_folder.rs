//! `TaskFolder` in scripts.

use cohesion_core::model::TagFilter;
use cohesion_core::{TaskFolder, TaskFolderFilter};
use cohesion_db::{CreateTaskCommand, DeleteTaskFolderCommand, ListTasksCommand};
use cohesion_macros::js_entity;
use rquickjs::Ctx;

use crate::entities::{
    task::JsTask,
    workspace_options::{TaskCreateOptions, TaskListOptions},
};

#[js_entity(TaskFolder, metadata)]
impl JsTaskFolder {
    fn tasks<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "Omit<TaskListOptions, \"inboxOnly\" | \"tag\">")] options: TaskListOptions,
    ) -> rquickjs::Result<Vec<JsTask>> {
        Ok(self
            .read(
                &ctx,
                ListTasksCommand(options.into_listing(
                    Some(self.inner.workspace_id),
                    TaskFolderFilter::Folder(self.inner.id),
                    TagFilter::default(),
                )),
            )?
            .into_iter()
            .map(|t| JsTask::new(t, self.db.clone()))
            .collect())
    }
    fn create<'js>(
        &mut self,
        ctx: Ctx<'js>,
        #[ts(type = "Omit<TaskCreateOptions, \"folder\">")] options: TaskCreateOptions,
    ) -> rquickjs::Result<JsTask> {
        let task = self.write(
            &ctx,
            CreateTaskCommand {
                title: options.title,
                workspace_id: self.inner.workspace_id,
                folder_id: Some(self.inner.id),
                priority: options.priority,
                size: options.size,
                assignee: options.assignee,
                description: options.description,
                start_date: options.start_date,
                due_date: options.due_date,
                tag_ids: Vec::new(),
            },
        )?;
        Ok(JsTask::new(task, self.db.clone()))
    }
    fn delete<'js>(&mut self, ctx: Ctx<'js>) -> rquickjs::Result<()> {
        self.inner = self.write(
            &ctx,
            DeleteTaskFolderCommand {
                workspace_id: self.inner.workspace_id,
                folder_id: self.inner.id,
            },
        )?;
        Ok(())
    }
}
