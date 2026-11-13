use cohesion_core::model::HookKind;
use cohesion_js::ScriptOutput;

/// What woke a fork, for the caption above each message it posts,
/// e.g. `"Test task" was created`. `subject` is the hook's entity as JSON;
/// when it has no `title`, the hook's text form stands in. `timer` names
/// its time, `extension:update` its extension, `client:revoke` its client and
/// `agent:turn:*` its agent.
pub(super) fn wake_description(hook: &HookKind, subject: &ScriptOutput) -> String {
    use HookKind::*;
    match hook {
        Launch => return "Cohesion launched".to_string(),
        AgentMessage => return "A message in another agent’s conversation".to_string(),
        AgentReaction => return "A reaction in another agent’s conversation".to_string(),
        DayStart => return "The day started".to_string(),
        DayEnd => return "The day ended".to_string(),
        _ => {}
    }
    let subject: serde_json::Value =
        serde_json::from_str(&subject.0).expect("a cue subject is JSON");
    if *hook == Timer {
        let hour = subject["hour"]
            .as_i64()
            .expect("a timer subject has its hour");
        let minute = subject["minute"]
            .as_i64()
            .expect("a timer subject has its minute");
        let (hour, period) = match hour {
            0 => (12, "AM"),
            1..=11 => (hour, "AM"),
            12 => (12, "PM"),
            _ => (hour - 12, "PM"),
        };
        return format!("The time reached {hour}:{minute:02} {period}");
    }
    if let AgentTurnStart | AgentTurnEnd = hook {
        let name = subject["name"]
            .as_str()
            .expect("an agent:turn subject has its name");
        return match hook {
            AgentTurnStart => format!("{name} started replying"),
            _ => format!("{name} finished replying"),
        };
    }
    if *hook == ExtensionUpdate {
        let name = subject["name"]
            .as_str()
            .expect("an extension:update subject has its name");
        return format!("An update is available for {name}");
    }
    if *hook == ClientRevoke {
        let name = subject["name"]
            .as_str()
            .expect("a client:revoke subject has its name");
        return format!("{name} was revoked");
    }
    let Some(title) = subject["title"].as_str() else {
        return hook.as_str().to_string();
    };
    match hook {
        TaskCreate => format!("“{title}” was created"),
        TaskStatus => format!("“{title}” changed status"),
        TaskRename => format!("Task renamed to “{title}”"),
        TaskMove => format!("“{title}” was moved"),
        TaskPriority => format!("“{title}” changed priority"),
        TaskSize => format!("“{title}” changed size"),
        TaskAssigned => format!("“{title}” was assigned"),
        TaskDescription => format!("“{title}” was edited"),
        TaskStartDate => format!("“{title}” got a new start date"),
        TaskDueDate => format!("“{title}” got a new due date"),
        TaskStart => format!("“{title}” starts today"),
        TaskDue => format!("“{title}” is due today"),
        TaskTagged => format!("“{title}” was tagged"),
        TaskDelete => format!("“{title}” was deleted"),
        TaskFolderCreate => format!("Folder “{title}” was created"),
        TaskFolderDelete => format!("Folder “{title}” was deleted"),
        WorkspaceCreate => format!("Workspace “{title}” was created"),
        WorkspaceDelete => format!("Workspace “{title}” was deleted"),
        JournalSettled => format!("Journal {title} was written"),
        DocumentSettled => format!("Document “{title}” was written"),
        ResourceImport => format!("“{title}” was imported"),
        ResourceUpdate => format!("“{title}” was updated"),
        ResourceDelete => format!("“{title}” was removed"),
        FeedArticle => format!("“{title}” came in from a feed"),
        FeedDelete => format!("Feed “{title}” was unfollowed"),
        ArticleRead => format!("“{title}” was read"),
        ActorStarted => format!("“{title}” started"),
        ActorProgress => format!("“{title}” made progress"),
        ActorMoved => format!("“{title}” changed status"),
        ActorResolved => format!("“{title}” was resolved"),
        NotificationSend => format!("Notification “{title}” was sent"),
        Launch | AgentMessage | AgentReaction | AgentTurnStart | AgentTurnEnd | DayStart
        | DayEnd | Timer | ExtensionUpdate | ClientRevoke => {
            unreachable!("returned above")
        }
    }
}
