//! `ActorSessionMessage`, the plain object `actors.messages` and `actors.send`
//! return, and the `ActorResponse` `actors.send` takes.

use cohesion_core::model::{self, ActorSessionMessageContent};
use cohesion_core::ts_documentation::TsDocumentation;
use serde::Serialize;

/// An actor session message, serialized to the object a script receives.
#[derive(Serialize)]
pub struct ActorSessionMessage {
    id: i64,
    #[serde(rename = "sessionId")]
    session_id: i64,
    #[serde(rename = "sentAt")]
    sent_at: i64,
    answered: bool,
    #[serde(rename = "attachmentCount")]
    attachment_count: i64,
    content: ActorSessionMessageContent,
}

impl From<model::ActorSessionMessage> for ActorSessionMessage {
    fn from(message: model::ActorSessionMessage) -> Self {
        Self {
            id: message.id,
            session_id: message.session_id,
            sent_at: message.sent_at,
            answered: message.answered,
            attachment_count: message.attachment_count,
            content: message.content,
        }
    }
}

impl TsDocumentation for ActorSessionMessage {
    const TS: &'static str = r#"/**
 * One entry in an actor session's transcript, as `actors.messages(sessionId)`
 * returns it: plain data. Switch on `content.sender`, then `content.kind`,
 * to read what it says; `content`'s fields are snake_case, as the actor
 * sent them. `sentAt` is in Unix seconds.
 *
 * The actor's `question`, `permission` and `plan` are requests: the session
 * needs attention until one is answered, with an `ActorResponse` naming
 * the request's `id` as `request_id`.
 */
interface ActorSessionMessage {
  id: number;
  sessionId: number;
  sentAt: number;
  /** Whether a request has been answered; always `false` for anything else. */
  answered: boolean;
  /** How many images came with it; `images.attachFromActorSession(id)` shows them to the user. */
  attachmentCount: number;
  content:
    | { sender: "actor"; kind: "text"; text: string }
    | {
        sender: "actor";
        kind: "thinking";
        text: string;
        /** When it ended, in Unix seconds; `null` while it goes on or unknown. */
        ended_at: number | null;
      }
    | {
        sender: "actor";
        kind: "toolUse";
        /** The tool, as the actor names it, such as `"Edit"`. */
        tool: string;
        category:
          | "command"
          | "read"
          | "edit"
          | "search"
          | "web"
          | "agent"
          | "skill"
          | "other";
        summary: string;
        /** When it ended, in Unix seconds; `null` while it runs or unknown. */
        ended_at: number | null;
      }
    | {
        sender: "actor";
        kind: "question";
        questions: {
          header: string | null;
          question: string;
          /** The answers to pick from; none for a question answered in words. */
          options: { label: string; description: string | null }[];
          multi_select: boolean;
          /** Whether it may be answered in words (`other`) instead. */
          allows_other: boolean;
        }[];
      }
    | {
        sender: "actor";
        kind: "permission";
        subject:
          | { kind: "command"; command: string; cwd: string | null }
          | {
              kind: "fileChange";
              changes: { path: string; change: "add" | "update" | "delete"; diff: string | null }[];
            }
          | { kind: "tool"; name: string; input: unknown };
        reason: string | null;
        /** The decisions it takes, in the order to offer them. */
        decisions: ("allow" | "allowForSession" | "deny" | "abort")[];
      }
    | { sender: "actor"; kind: "plan"; plan: string }
    | { sender: "user"; kind: "text"; text: string }
    | ({ sender: "user" } & ActorResponse);
}

/**
 * A response to one of an actor's requests, which `actors.send` takes in
 * place of text: the answers to a `question` (one per question, in order,
 * picking only its options' labels, and `other` only where it allows
 * it), the decision on a `permission` (one it offers), or whether a
 * `plan` is approved. Each request takes one response.
 *
 * Approving a `plan` starts its implementation in the same session, which
 * works again: wait on or follow that session, and never start a second session
 * to implement the plan.
 */
type ActorResponse =
  | { kind: "answers"; request_id: number; answers: { selected: string[]; other?: string | null }[] }
  | {
      kind: "permission";
      request_id: number;
      decision: "allow" | "allowForSession" | "deny" | "abort";
      /** What to tell the actor with it, such as why not. */
      message?: string | null;
    }
  | {
      kind: "plan";
      request_id: number;
      /** `true` has the actor implement the plan in this session. */
      approved: boolean;
      /** What to change, when not approved. */
      feedback?: string | null;
    };"#;
    const TS_BRIEF: &'static str = Self::TS;
}
