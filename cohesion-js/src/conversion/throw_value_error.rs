use cohesion_core::value::ValueError;
use rquickjs::{Ctx, Error, Exception};

/// The `TypeError` a value a script passed throws when it is the wrong type
/// or not one the field accepts, as [`throw_command_error`](crate::traced_command::throw_command_error)
/// is for a failed command.
pub(crate) fn throw_value_error(ctx: &Ctx<'_>, error: ValueError) -> Error {
    match error {
        ValueError::WrongType { expected, found } => {
            Exception::throw_type(ctx, &format!("Expected type {}, got {}", expected, found))
        }
        ValueError::Invalid(err) => Exception::throw_type(ctx, &format!("Invalid value {}", err)),
    }
}
