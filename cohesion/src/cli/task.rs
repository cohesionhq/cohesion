use std::io::{self, IsTerminal};

use clap::builder::{PossibleValuesParser, TypedValueParser};
use clap::{ArgGroup, Args, Subcommand};
use cohesion_core::model::{ListSymbol, TagFilter};
use cohesion_core::{
    Task, TaskFolder, TaskFolderFilter, TaskPriority, TaskSort, TaskStatus,
    protocol::{
        ChangeTasksStatusRequest, CreateTaskFolderRequest, CreateTaskRequest,
        DeleteTaskFolderRequest, EditTaskDescriptionRequest, ListTaskFoldersRequest,
        ListTasksRequest, ScheduleTaskRequest, SetTaggedRequest, SetTaskFolderSymbolRequest,
        SetTaskPriorityRequest,
    },
};

use super::{
    calendar_day::{DayArg, parse_day, parse_day_arg},
    noun::Noun,
    table,
    tag::{find_tag, names, tags},
    terminal_io::{confirmed, edit_in_editor, words_and_stdin},
};
use crate::{cli_context::Context, cli_error::CliError, cli_macros::row, name_lookup::find};

#[derive(Args)]
#[command(args_conflicts_with_subcommands = true)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub verb: Option<TaskCommand>,
    #[command(flatten)]
    pub list: TaskListArgs,
}

#[derive(Args)]
pub struct TaskListArgs {
    /// Only list tasks in this folder (id or title).
    #[arg(short = 'f', long)]
    pub folder: Option<String>,
    /// Include completed and cancelled tasks.
    #[arg(short = 'a', long)]
    pub all: bool,
    /// Only list tasks with this tag (id or name).
    #[arg(long)]
    pub tag: Option<String>,
}

#[derive(Subcommand)]
pub enum TaskCommand {
    /// Add a task.
    Add {
        /// What the task is; several words are joined with spaces.
        #[arg(required = true)]
        title: Vec<String>,
        /// File the task in this folder (id or title).
        #[arg(short = 'f', long)]
        folder: Option<String>,
        /// How urgent the task is.
        #[arg(short = 'p', long, value_parser = priority_parser())]
        priority: Option<TaskPriority>,
        /// Notes on the task, as Markdown.
        #[arg(short = 'd', long)]
        description: Option<String>,
        /// The day work can begin: YYYY-MM-DD, today or tomorrow.
        #[arg(long, value_name = "DAY", value_parser = parse_day)]
        start: Option<String>,
        /// The day the task is due: YYYY-MM-DD, today or tomorrow.
        #[arg(long, value_name = "DAY", value_parser = parse_day)]
        due: Option<String>,
    },
    /// Set or clear a task's start and due days; a date left out keeps its value.
    #[command(group(ArgGroup::new("days").required(true).multiple(true)))]
    Schedule {
        id: i64,
        /// YYYY-MM-DD, today, tomorrow, or none to clear it.
        #[arg(long, value_name = "DAY", value_parser = parse_day_arg, group = "days")]
        start: Option<DayArg>,
        /// YYYY-MM-DD, today, tomorrow, or none to clear it.
        #[arg(long, value_name = "DAY", value_parser = parse_day_arg, group = "days")]
        due: Option<DayArg>,
    },
    /// Print a task with its description.
    Show { id: i64 },
    /// Replace a task's description with text or piped stdin, or edit it in $VISUAL or $EDITOR
    /// when given neither.
    Describe {
        id: i64,
        /// The new description; several words are joined with spaces, and `""` clears it.
        text: Vec<String>,
    },
    /// Set a task's priority.
    Priority {
        id: i64,
        #[arg(value_parser = priority_parser())]
        priority: TaskPriority,
    },
    /// Put a tag on a task.
    Tag {
        id: i64,
        /// Tag id or name.
        tag: String,
    },
    /// Take a tag off a task.
    Untag {
        id: i64,
        /// Tag id or name.
        tag: String,
    },
    /// Manage task folders, or list them when no subcommand is given.
    Folder(Noun<TaskFolderCommand>),
    Complete {
        #[arg(required = true)]
        ids: Vec<i64>,
    },
    Cancel {
        #[arg(required = true)]
        ids: Vec<i64>,
    },
}

#[derive(Subcommand)]
pub enum TaskFolderCommand {
    /// Create a task folder.
    Create { title: String },
    /// Delete a task folder, after asking for confirmation.
    Delete {
        /// Folder id or title.
        folder: String,
        /// Don't ask for confirmation.
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// Draw a task folder with a symbol, or with a plain folder when none is given.
    Symbol {
        /// Folder id or title.
        folder: String,
        #[arg(value_parser = symbol_parser())]
        symbol: Option<ListSymbol>,
    },
}

fn symbol_parser() -> impl TypedValueParser<Value = ListSymbol> {
    PossibleValuesParser::new(ListSymbol::CASES.iter().copied())
        .map(|text| ListSymbol::from_str_strict(&text).expect("clap only passes listed cases"))
}

fn priority_parser() -> impl TypedValueParser<Value = TaskPriority> {
    PossibleValuesParser::new(TaskPriority::CASES.iter().copied())
        .map(|text| TaskPriority::from_str_strict(&text).expect("clap only passes listed cases"))
}

pub async fn run(ctx: &Context, TaskArgs { verb, list }: TaskArgs) -> Result<(), CliError> {
    match verb {
        None => list_tasks(ctx, list).await,
        Some(TaskCommand::Add {
            title,
            folder,
            priority,
            description,
            start,
            due,
        }) => {
            let details = NewTaskDetails {
                priority,
                description,
                start,
                due,
            };
            add(ctx, title, folder, details).await
        }
        Some(TaskCommand::Schedule { id, start, due }) => schedule(ctx, id, start, due).await,
        Some(TaskCommand::Show { id }) => show(ctx, id).await,
        Some(TaskCommand::Describe { id, text }) => set_description(ctx, id, text).await,
        Some(TaskCommand::Priority { id, priority }) => set_priority(ctx, id, priority).await,
        Some(TaskCommand::Tag { id, tag }) => set_tagged(ctx, id, tag, true).await,
        Some(TaskCommand::Untag { id, tag }) => set_tagged(ctx, id, tag, false).await,
        Some(TaskCommand::Folder(Noun { verb })) => match verb {
            None => list_folders(ctx).await,
            Some(TaskFolderCommand::Create { title }) => create_folder(ctx, title).await,
            Some(TaskFolderCommand::Delete { folder, yes }) => {
                delete_folder(ctx, folder, yes).await
            }
            Some(TaskFolderCommand::Symbol { folder, symbol }) => {
                set_folder_symbol(ctx, folder, symbol).await
            }
        },
        Some(TaskCommand::Complete { ids }) => mark_tasks(ctx, ids, TaskStatus::Completed).await,
        Some(TaskCommand::Cancel { ids }) => mark_tasks(ctx, ids, TaskStatus::Cancelled).await,
    }
}

/// Tasks as `[ ]`, `[x]` (completed) or `[-]` (cancelled), with their id, title and
/// priority (unless "none"), start and due days when set, their tags, and their folder unless
/// listing a single folder.
async fn list_tasks(
    ctx: &Context,
    TaskListArgs { folder, all, tag }: TaskListArgs,
) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let folders = ctx.call(ListTaskFoldersRequest { workspace_id }).await?;
    let folder_id = match folder {
        Some(query) => Some(find(&folders, &query)?.id),
        None => None,
    };
    let task_tags = tags(ctx, workspace_id).await?;
    let tags = TagFilter {
        with: match tag {
            Some(query) => vec![find(&task_tags, &query)?.id],
            None => Vec::new(),
        },
        without: Vec::new(),
    };
    let statuses = all.then(|| {
        vec![
            TaskStatus::Incomplete,
            TaskStatus::Completed,
            TaskStatus::Cancelled,
        ]
    });
    let folder = folder_id.map_or(TaskFolderFilter::Any, TaskFolderFilter::Folder);
    let tasks = every_task(ctx, workspace_id, folder, statuses, tags).await?;

    let empty = match all {
        true => "No tasks.\nAdd one with `cohesion task add <title>`.",
        false => "No open tasks.\nAdd one with `cohesion task add <title>`.",
    };
    table::list(&tasks, empty, |t| {
        let mark = match t.status {
            TaskStatus::Incomplete => "[ ]",
            TaskStatus::Completed => "[x]",
            TaskStatus::Cancelled => "[-]",
        };
        let folder = t
            .task_folder_id
            .filter(|_| folder_id.is_none())
            .and_then(|id| folders.iter().find(|f| f.id == id))
            .map(|f| format!("({})", f.title))
            .unwrap_or_default();
        let priority = match t.priority {
            TaskPriority::None => "",
            priority => priority.as_str(),
        };
        let schedule = schedule_line(t).unwrap_or_default();
        let tags = names(&task_tags, &t.tag_ids);
        row![mark, >t.id, t.title, priority, schedule, tags, folder]
    });
    Ok(())
}

/// Every task in workspace `workspace_id` the filters cover, in creation
/// order, read a page at a time.
async fn every_task(
    ctx: &Context,
    workspace_id: i64,
    folder: TaskFolderFilter,
    statuses: Option<Vec<TaskStatus>>,
    tags: TagFilter,
) -> Result<Vec<Task>, CliError> {
    let mut tasks = Vec::new();
    loop {
        let page = ctx
            .call(ListTasksRequest {
                tags: tags.clone(),
                ids: None,
                workspace_id: Some(workspace_id),
                folder,
                statuses: statuses.clone(),
                priorities: None,
                sizes: None,
                start_date: None,
                due_date: None,
                assignee: None,
                sort: TaskSort::Created,
                descending: false,
                offset: tasks.len() as u64,
                limit: Some(500),
                around: None,
            })
            .await?;
        let last = page.tasks.is_empty();
        tasks.extend(page.tasks);
        if last || tasks.len() as i64 >= page.total {
            return Ok(tasks);
        }
    }
}

/// `starts 2026-10-14 · due 2026-10-20`, or whichever of the two is set.
fn schedule_line(task: &Task) -> Option<String> {
    let parts: Vec<_> = [("starts", &task.start_date), ("due", &task.due_date)]
        .into_iter()
        .filter_map(|(label, day)| day.as_ref().map(|day| format!("{label} {day}")))
        .collect();
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The optional fields of `task add`.
struct NewTaskDetails {
    priority: Option<TaskPriority>,
    description: Option<String>,
    start: Option<String>,
    due: Option<String>,
}

async fn add(
    ctx: &Context,
    title: Vec<String>,
    folder: Option<String>,
    details: NewTaskDetails,
) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let folder_id = match folder {
        Some(query) => Some(ctx.find_folder(workspace_id, &query).await?.id),
        None => None,
    };
    let task = ctx
        .call(CreateTaskRequest {
            workspace_id,
            title: title.join(" "),
            folder_id,
            priority: details.priority,
            size: None,
            assignee: None,
            description: details.description,
            start_date: details.start,
            due_date: details.due,
            tag_ids: Vec::new(),
        })
        .await?;
    println!("Added task {}: {}.", task.id, task.title);
    Ok(())
}

/// Task `task_id` in the current workspace, whatever its status.
async fn find_task(ctx: &Context, workspace_id: i64, task_id: i64) -> Result<Task, CliError> {
    let tasks = ctx
        .call(ListTasksRequest {
            tags: TagFilter::default(),
            ids: Some(vec![task_id]),
            workspace_id: Some(workspace_id),
            folder: TaskFolderFilter::Any,
            priorities: None,
            sizes: None,
            start_date: None,
            due_date: None,
            assignee: None,
            statuses: Some(vec![
                TaskStatus::Incomplete,
                TaskStatus::Completed,
                TaskStatus::Cancelled,
            ]),
            sort: TaskSort::Created,
            descending: false,
            offset: 0,
            limit: None,
            around: None,
        })
        .await?;
    tasks
        .tasks
        .into_iter()
        .next()
        .ok_or_else(|| CliError::UnknownName("task", task_id.to_string()))
}

/// The title, then one `key: value` line per set field, then the description.
async fn show(ctx: &Context, task_id: i64) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let task = find_task(ctx, workspace_id, task_id).await?;
    println!("{}", task.title);
    println!("status: {}", task.status);
    if task.priority != TaskPriority::None {
        println!("priority: {}", task.priority);
    }
    if let Some(size) = task.size {
        println!("size: {}", size.get());
    }
    if let Some(day) = &task.start_date {
        println!("starts: {day}");
    }
    if let Some(day) = &task.due_date {
        println!("due: {day}");
    }
    if let Some(folder_id) = task.task_folder_id {
        let folders = ctx.call(ListTaskFoldersRequest { workspace_id }).await?;
        if let Some(folder) = folders.iter().find(|f| f.id == folder_id) {
            println!("folder: {}", folder.title);
        }
    }
    if !task.description.is_empty() {
        println!("\n{}", task.description.trim_end());
    }
    Ok(())
}

/// Sets the days given and keeps the task's current value for the other.
async fn schedule(
    ctx: &Context,
    task_id: i64,
    start: Option<DayArg>,
    due: Option<DayArg>,
) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let task = find_task(ctx, workspace_id, task_id).await?;
    let task = ctx
        .call(ScheduleTaskRequest {
            workspace_id,
            task_id,
            start_date: start.map_or(task.start_date, |DayArg(day)| day),
            due_date: due.map_or(task.due_date, |DayArg(day)| day),
        })
        .await?;
    let line = schedule_line(&task).unwrap_or_else(|| "no dates".to_string());
    println!("Scheduled task {}: {line}.", task.id);
    Ok(())
}

/// Saves only if the description changed.
async fn set_description(ctx: &Context, task_id: i64, text: Vec<String>) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let description = if text.is_empty() && io::stdin().is_terminal() {
        let task = find_task(ctx, workspace_id, task_id).await?;
        let edited = edit_in_editor(&task.description)?;
        if edited == task.description {
            println!("No changes to task {task_id}.");
            return Ok(());
        }
        edited
    } else {
        words_and_stdin(&text)?
    };
    let task = ctx
        .call(EditTaskDescriptionRequest {
            workspace_id,
            task_id,
            description: description.trim_end().to_string(),
        })
        .await?;
    println!("Saved the description of task {}: {}.", task.id, task.title);
    Ok(())
}

async fn set_priority(ctx: &Context, task_id: i64, priority: TaskPriority) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let task = ctx
        .call(SetTaskPriorityRequest {
            workspace_id,
            task_id,
            priority,
        })
        .await?;
    println!("Set task {} to {} priority.", task.id, task.priority);
    Ok(())
}

async fn set_tagged(
    ctx: &Context,
    task_id: i64,
    tag: String,
    tagged: bool,
) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let found = find_tag(ctx, workspace_id, &tag).await?;
    let task = ctx
        .call(SetTaggedRequest {
            workspace_id,
            tag_id: found.id,
            task_id,
            tagged,
        })
        .await?;
    match tagged {
        true => println!(
            "Tagged task {}: {} with \"{}\".",
            task.id, task.title, found.name
        ),
        false => println!(
            "Untagged task {}: {} from \"{}\".",
            task.id, task.title, found.name
        ),
    }
    Ok(())
}

async fn list_folders(ctx: &Context) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let folders = ctx.call(ListTaskFoldersRequest { workspace_id }).await?;
    table::list(
        &folders,
        "No task folders.\nCreate one with `cohesion task folder create <title>`.",
        |f| row![>f.id, f.title],
    );
    Ok(())
}

async fn create_folder(ctx: &Context, title: String) -> Result<(), CliError> {
    let folder = ctx
        .call(CreateTaskFolderRequest {
            workspace_id: ctx.workspace().await?,
            title,
        })
        .await?;
    println!("Created {}.", describe(&folder));
    Ok(())
}

async fn delete_folder(ctx: &Context, folder: String, yes: bool) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let found = ctx.find_folder(workspace_id, &folder).await?;
    if !confirmed(&format!("Delete {}?", describe(&found)), yes)? {
        return Ok(());
    }
    let deleted = ctx
        .call(DeleteTaskFolderRequest {
            workspace_id,
            folder_id: found.id,
        })
        .await?;
    println!("Deleted {}.", describe(&deleted));
    Ok(())
}

async fn set_folder_symbol(
    ctx: &Context,
    folder: String,
    symbol: Option<ListSymbol>,
) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let found = ctx.find_folder(workspace_id, &folder).await?;
    let folder = ctx
        .call(SetTaskFolderSymbolRequest {
            workspace_id,
            folder_id: found.id,
            symbol,
        })
        .await?;
    match folder.symbol {
        Some(symbol) => println!("Gave {} the {} symbol.", describe(&folder), symbol.as_str()),
        None => println!("Took the symbol off {}.", describe(&folder)),
    }
    Ok(())
}

async fn mark_tasks(ctx: &Context, ids: Vec<i64>, status: TaskStatus) -> Result<(), CliError> {
    let workspace_id = ctx.workspace().await?;
    let updated = ctx
        .call(ChangeTasksStatusRequest {
            workspace_id,
            status: status.clone(),
            task_ids: ids,
        })
        .await?;
    if updated.is_empty() {
        println!("Already {}.", status.as_str());
        return Ok(());
    }
    println!(
        "Marked {} as {}.",
        updated
            .into_iter()
            .map(|x| x.title)
            .collect::<Vec<_>>()
            .join(", "),
        status.as_str(),
    );
    Ok(())
}

/// `task folder "Title" (id)`, as messages name a folder.
fn describe(folder: &TaskFolder) -> String {
    format!("task folder \"{}\" ({})", folder.title, folder.id)
}
