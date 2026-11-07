//! `AgentMessage`, the plain object `onAgentMessage` and `onAgentReaction`
//! receive.

use cohesion_core::model::{
    AgentMessageContent, IntelligenceAgentMessage, IntelligenceAgentMessageSender,
};
use cohesion_core::ts_documentation::TsDocumentation;
use serde::Serialize;

/// An agent message, serialized to the object a script receives.
#[derive(Serialize)]
pub struct AgentMessage {
    id: i64,
    #[serde(rename = "agentId")]
    agent_id: i64,
    #[serde(rename = "sentAt")]
    sent_at: i64,
    sender: IntelligenceAgentMessageSender,
    #[serde(rename = "runDescription")]
    run_description: Option<String>,
    reaction: Option<String>,
    content: AgentMessageContent,
}

impl From<IntelligenceAgentMessage> for AgentMessage {
    fn from(message: IntelligenceAgentMessage) -> Self {
        Self {
            id: message.id,
            agent_id: message.agent_id,
            sent_at: message.sent_at,
            sender: message.sender,
            run_description: message.run_description,
            reaction: message.reaction,
            content: message.message_content,
        }
    }
}

impl TsDocumentation for AgentMessage {
    const TS: &'static str = r#"/**
 * A message in an agent's conversation, as `onAgentMessage` and
 * `onAgentReaction` receive it: plain data, read when the hook runs. Switch on `content.kind` to read what
 * it says. `sentAt` is in Unix seconds.
 */
interface AgentMessage {
  id: number;
  /** The agent whose conversation it is in. */
  agentId: number;
  sentAt: number;
  /**
   * `"user"` for what the user wrote, `"agent"` for the agent in the
   * conversation or a fork of it that another agent's question woke,
   * `"backgroundAgent"` for a fork one of its cues woke (and another agent's
   * question to it).
   */
  sender: "user" | "agent" | "backgroundAgent";
  /**
   * What woke the fork that sent it, e.g. `"Test task" was
   * created`; `null` for the user's and the agent's own messages.
   */
  runDescription: string | null;
  /**
   * The emoji the other side reacted with (the agent on the user's
   * messages, the user on the agent's), usually one of 👍 👎 ❤️ 😂 😮 😢 🎉 🙏;
   * `null` when there is none.
   */
  reaction: string | null;
  content: AgentMessageContent;
}

/**
 * What an agent message says, by `kind`:
 * - `"text"`: a message the user or the agent wrote, or a note a fork
 *   posted.
 * - `"codeExecution"`: a script the agent ran, with how it ended: its
 *   result, why it failed, or that the user stopped it.
 * - `"error"`: why the agent's turn or a fork failed.
 * - `"documentation"`: the agent loaded a module of the script API, such
 *   as `"network"`.
 * - `"choice"`: the agent or a fork asked a question with answers
 *   to pick from, lettered A, B, C… The user's pick is recorded in
 *   `selected`; it posts no message of its own, and picking fires nothing.
 * - `"memory"`: the agent remembered or forgot a memory.
 * - `"forward"`: the user forwarded a task, document or journal entry, with
 *   a note; read the item from its `cohesion://` link.
 * - `"askedAgent"`: the agent asked another agent, named in `agent`, a
 *   question, and how it was answered. The same question stands as
 *   `"askedByAgent"` in the other agent's conversation, naming the agent
 *   that asked; both say the same `reply` as it changes, which fires
 *   nothing.
 */
type AgentMessageContent =
  | { kind: "text"; text: string }
  | {
      kind: "codeExecution";
      /** What the script does, as the agent described it. */
      title: string;
      code: string;
      /** How the script ended. */
      outcome:
        | { status: "succeeded"; result: string }
        | { status: "failed"; error: string }
        | { status: "cancelled" };
    }
  | { kind: "error"; error: string }
  | { kind: "documentation"; module: string }
  | {
      kind: "choice";
      /** The question. */
      question: string;
      /** The answers on offer, in letter order. */
      options: string[];
      /**
       * From a fork: what the agent needs to act on the pick,
       * which the user doesn't see; `null` from the conversation itself.
       */
      context: string | null;
      /** The index in `options` of the user's pick; `null` until they pick. */
      selected: number | null;
    }
  | { kind: "memory"; memory: string; change: "remembered" | "forgotten" }
  | {
      kind: "forward";
      /** What the user wrote with the item, or `""`. */
      note: string;
      item: {
        kind: "task" | "document" | "journal";
        workspace_id: number;
        id: number;
        /** The title when it was forwarded; a journal entry's is its day, `YYYY-MM-DD`. */
        title: string;
      };
    }
  | {
      kind: "askedAgent" | "askedByAgent";
      /** The other agent, by its name when it was asked. */
      agent: { id: number; name: string };
      question: string;
      /** Waiting, the answer, or why none came. */
      reply:
        | { status: "waiting" }
        | { status: "answered"; text: string }
        | { status: "unanswered"; reason: string };
    };"#;
    const TS_BRIEF: &'static str = Self::TS;
}
