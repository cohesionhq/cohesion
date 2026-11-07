use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::Initiative;
use cohesion_core::protocol::ImageUpload;
use cohesion_db::{
    CommandError, DatabaseContext, DeleteAgentCueCommand, FollowActorSessionCommand,
    GuardedDatabaseContext, ListAgentCuesCommand, SetAgentCueCommand,
};
use cohesion_js::ForegroundEffect;

use super::agent_tool::role_database;
use super::image_tray::ImageTray;

use crate::transcript_item::ToolCallStatus;

/// What became of the [`ForegroundEffect`]s an agent's script asked for, one note per effect, so the model can confirm them or correct itself.
#[derive(Default)]
pub(super) struct AppliedEffects {
    notes: Vec<String>,
    /// Whether any effect was refused or failed.
    refused: bool,
}
impl AppliedEffects {
    /// Applies a foreground script's effects (a chat turn's or a fork's):
    /// the `agent` global's, and the actor sessions it
    /// handed work to follow.
    /// Cues are set and removed through the database the turn's role
    /// writes (`role_database`), so a turn an event started changes none and
    /// asks the user instead.
    pub(super) async fn foreground(
        database: &DatabaseContext,
        agent_id: i64,
        initiative: Initiative,
        effects: Vec<ForegroundEffect>,
    ) -> Self {
        let writes = role_database(database, initiative);
        let mut applied = Self::default();
        for effect in effects {
            match effect {
                ForegroundEffect::Cue {
                    name,
                    hook,
                    code,
                    instructions,
                    effort,
                } => {
                    let note = format!("Cue \"{name}\" set on {hook}, at {effort} effort.");
                    let result = writes
                        .send_write(SetAgentCueCommand {
                            agent_id,
                            name: name.clone(),
                            hook,
                            script: code,
                            instructions,
                            effort,
                        })
                        .await;
                    match result {
                        Some(result) => _ = applied.record(agent_id, result, note),
                        None => applied.refuse(&format!(
                            "Cue \"{name}\" not set: a turn your cue started can't set cues. \
                             Ask the user with `ask_choice`."
                        )),
                    }
                }
                ForegroundEffect::RemoveCue(name) => {
                    applied.remove_cue(database, &writes, agent_id, name).await;
                }
                ForegroundEffect::Follow(actor_session_id) => {
                    let result = database
                        .send(FollowActorSessionCommand {
                            session_id: actor_session_id,
                            agent_id,
                            initiative,
                        })
                        .await;
                    applied.record(
                        agent_id,
                        result,
                        format!(
                            "You follow actor session {actor_session_id}: a fork of this conversation wakes \
                             each time it stops on its own (it needs the user, finishes, fails or \
                             is interrupted), never when someone stops it."
                        ),
                    );
                }
            }
        }
        applied
    }

    /// These notes, then the one numbering the images the script attached,
    /// once they are in `tray`, the session's.
    pub(super) fn attached(mut self, tray: &ImageTray, images: Vec<ImageUpload>) -> Self {
        self.notes.extend(tray.attach(images));
        self
    }

    /// The script's output with the notes after it; a failure when an effect
    /// was refused, so the model notices.
    pub(super) fn tool_result(self, output: String) -> ToolCallStatus {
        let text = self.append_to(output);
        if self.refused {
            ToolCallStatus::Failure(text)
        } else {
            ToolCallStatus::Success(text)
        }
    }

    /// `output` followed by one note per line.
    pub(super) fn append_to(&self, output: String) -> String {
        if self.notes.is_empty() {
            return output;
        }
        format!("{output}\n\n{}", self.notes.join("\n"))
    }

    /// Removes the agent's cue named `name`, which the model chose, so it
    /// may name none, through `writes`, the turn's role's database.
    async fn remove_cue(
        &mut self,
        database: &DatabaseContext,
        writes: &GuardedDatabaseContext,
        agent_id: i64,
        name: String,
    ) {
        let cue = match database.send(ListAgentCuesCommand { agent_id }).await {
            Ok(cues) => cues.into_iter().find(|cue| cue.name == name),
            Err(e) => {
                self.record::<()>(agent_id, Err(e), String::new());
                return;
            }
        };
        match cue {
            Some(cue) => {
                let result = writes
                    .send_write(DeleteAgentCueCommand { cue_id: cue.id })
                    .await;
                match result {
                    Some(result) => {
                        self.record(agent_id, result, format!("Cue \"{name}\" removed."));
                    }
                    None => self.refuse(&format!(
                        "Cue \"{name}\" not removed: a turn your cue started can't remove cues. \
                         Ask the user with `ask_choice`."
                    )),
                }
            }
            None => self.refuse(&format!("You have no cue named \"{name}\".")),
        }
    }

    /// Notes `result`; whether the effect applied.
    fn record<T>(&mut self, agent_id: i64, result: Result<T, CommandError>, note: String) -> bool {
        match result {
            Ok(_) => {
                self.notes.push(note);
                true
            }
            Err(e) => {
                log_error!(
                    Module::Intelligence,
                    "Agent {agent_id}: could not apply an effect: {e:?}"
                );
                self.refuse("An internal error stopped one of your agent actions.");
                false
            }
        }
    }

    fn refuse(&mut self, note: &str) {
        self.notes.push(note.to_string());
        self.refused = true;
    }
}
