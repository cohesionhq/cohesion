//! The traits tying each request type to its [`Command`] variant and the
//! [`ReplyData`] variant its reply travels in.

use super::command::Command;
use super::reply_data::ReplyData;

/// Something a client can send, with the type of the reply it gets back.
///
/// `Reply` is the payload inside the matching [`ReplyData`] variant, so a
/// client can turn a reply into a typed value with `C::Reply::try_from`.
pub trait Call: Into<Command> {
    /// The payload a successful reply carries.
    type Reply: ReplyPayload;
}

/// A type that has a [`ReplyData`] variant to travel in. Implemented by
/// `#[derive(Replies)]` only; it exists so a missing variant gets a readable
/// compiler error.
#[diagnostic::on_unimplemented(
    message = "`{Self}` is not a reply type: no `ReplyData` variant carries it",
    label = "no `ReplyData` variant carries `{Self}`",
    note = "add a variant `MyReply({Self})` to `ReplyData` in cohesion-core/src/protocol/reply_data.rs"
)]
pub trait ReplyPayload: Into<ReplyData> + TryFrom<ReplyData, Error = ReplyData> {}
