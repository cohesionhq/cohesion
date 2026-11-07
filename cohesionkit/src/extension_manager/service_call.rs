//! Calling a service extension's `serviceRequest`, for a client through
//! `callExtension`, telling the service which client calls, or for a script
//! through `services.use`.

use cohesion_core::log::Module;
use cohesion_core::log_info;
use cohesion_core::model::ExtensionPermissionKind;
use cohesion_core::protocol::ServerError;
use cohesion_db::GetExtensionServiceCommand;
use cohesion_js::{ScriptArgument, ScriptError, ScriptOutput};

use super::{ExtensionError, ExtensionManager, enabled, require, script_fault};

/// The longest request name a service call takes, in characters.
const MAX_CALL_NAME: usize = 128;
/// The largest `args` a service call takes, as JSON.
const MAX_CALL_ARGS: usize = 64 * 1024;

/// Who calls a service, which decides what the call may reach.
#[derive(Clone, Copy)]
pub(crate) enum ServiceCaller {
    /// Client `client_id`, through `callExtension`: a service holding
    /// `service` or `service:isolated`, at any name if it declares none.
    Client { client_id: i64 },
    /// A script, through `services.use`: only a service holding `service`
    /// and a function it declares; with `read_only`, for a script whose
    /// database access is read-only, one declared `readOnly`. The service
    /// still runs with its own grants.
    Script { read_only: bool },
}

impl ServiceCaller {
    /// What `serviceRequest` receives as `caller`: `{ clientId }` for a
    /// client, `null` for a script, which runs as no client.
    fn as_argument(self) -> serde_json::Value {
        match self {
            Self::Client { client_id } => serde_json::json!({ "clientId": client_id }),
            Self::Script { .. } => serde_json::Value::Null,
        }
    }
}

impl ExtensionManager {
    /// What extension `extension_id`'s `serviceRequest(name, args, caller)`
    /// returns, `null` for `undefined`, once `caller` may reach it.
    pub(crate) async fn call_service(
        &self,
        extension_id: i64,
        caller: ServiceCaller,
        name: String,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, ServerError> {
        if name.is_empty() || name.chars().count() > MAX_CALL_NAME {
            return Err(ServerError::validation(
                Some("name".to_string()),
                format!("A request name must be 1 to {MAX_CALL_NAME} characters."),
            ));
        }
        let args_len = serde_json::to_vec(&args)
            .expect("a JSON value always serializes")
            .len();
        if args_len > MAX_CALL_ARGS {
            return Err(ServerError::validation(
                Some("args".to_string()),
                "A request's arguments can be at most 64 KB as JSON.",
            ));
        }

        let (extension, grants) = self.read_grants(extension_id).await?;
        let service = match caller {
            ServiceCaller::Client { .. }
                if grants
                    .iter()
                    .any(|grant| grant.permission == ExtensionPermissionKind::ServiceIsolated) =>
            {
                ExtensionPermissionKind::ServiceIsolated
            }
            _ => ExtensionPermissionKind::Service,
        };
        let permissions = enabled(grants);
        require(&permissions, Some(service))?;
        // A declared service answers only the names it declares.
        let declared = self
            .inner
            .database
            .send(GetExtensionServiceCommand { extension_id })
            .await?;
        let function = declared
            .as_ref()
            .and_then(|service| service.functions.iter().find(|f| f.name == name));
        match (caller, &declared, function) {
            (ServiceCaller::Script { .. }, None, _) => {
                return Err(ServerError::validation(
                    None,
                    format!(
                        "{} declares no service for scripts to call.",
                        extension.name
                    ),
                ));
            }
            (_, Some(_), None) => {
                return Err(ServerError::validation(
                    Some("name".to_string()),
                    format!("{} doesn't serve `{name}`.", extension.name),
                ));
            }
            (ServiceCaller::Script { read_only: true }, _, Some(function))
                if !function.read_only =>
            {
                return Err(ServerError::validation(
                    Some("name".to_string()),
                    format!(
                        "{}'s `{name}` may change data, so a script with read-only access to \
                         Cohesion, such as a background run, can't call it.",
                        extension.name
                    ),
                ));
            }
            _ => {}
        }

        log_info!(
            Module::Extensions,
            "Calling serviceRequest '{name}' on extension '{}'.",
            extension.script_identifier
        );
        let arguments = vec![
            ScriptArgument::String(name),
            ScriptArgument::Json(args),
            ScriptArgument::Json(caller.as_argument()),
        ];
        let called = self
            .scoped_with(extension.clone(), permissions, move |sandbox| {
                // `undefined` has no JSON; it answers `null`.
                let ScriptOutput(json) =
                    sandbox.call_with_arguments("serviceRequest", &arguments)?;
                if json == "undefined" {
                    return Ok(serde_json::Value::Null);
                }
                serde_json::from_str(&json).map_err(|e| ScriptError::InvalidFormat(e.to_string()))
            })
            .await;
        called.map_err(|e| match e {
            ExtensionError::Script(error) => {
                script_fault(&extension.name, "serviceRequest", error, |message| {
                    format!("serviceRequest didn't return JSON: {message}")
                })
                .map_or_else(ServerError::internal, |message| {
                    ServerError::validation(None, message)
                })
            }
            e => e.into(),
        })
    }
}
