//! `services`: the functions installed extensions serve, one service at a
//! time through `services.use(identifier)`, looked up afresh on each call
//! so installs and grant changes show up in every later job, even in an
//! extension's long-lived scope.

use std::sync::Arc;

use cohesion_core::model::InstalledService;
use cohesion_core::ts_documentation::TsDocumentation;
use cohesion_db::{GetCallableServiceCommand, GuardedDatabaseContext, WriteArea};
use rquickjs::function::Opt;
use rquickjs::{Ctx, Exception, Function, JsLifetime, Object, Value, class::Trace};

use cohesion_macros::js_global;

use super::global::Global;
use crate::connector_access::throw_server_error;
use crate::conversion::FromJsField;
use crate::sandbox::ScriptDeadline;
use crate::service_access::{CurrentServices, ServiceAccess, ServiceCall, ServiceHost};
use crate::traced_command::read;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct ServicesGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// The functions installed extensions serve, listed under "Installed services"
/// in the code tool's description.
///
/// Each call reaches the extension and waits for its answer.
#[js_global("services")]
#[rquickjs::methods]
impl ServicesGlobal {
    /// The service the extension `identifier` (such as `"com.example.counter"`)
    /// serves: a frozen object with one function per function it declares, each
    /// taking its one argument and returning the service's answer.
    ///
    /// Throws if no installed extension with that identifier serves scripts. A
    /// function not declared read-only throws when called from a script with
    /// read-only access to Cohesion, such as a turn a cue started.
    #[qjs(rename = "use")]
    #[ts(generics = "K extends keyof ServiceMap", returns = "ServiceMap[K]")]
    pub fn use_service<'js>(
        &self,
        ctx: Ctx<'js>,
        #[ts(type = "K")] identifier: String,
    ) -> rquickjs::Result<Object<'js>> {
        let host = ctx.userdata::<CurrentServices>().map(|host| host.0.clone());
        let Some((host, access)) =
            host.and_then(|host| host.access.get().cloned().map(|access| (host, access)))
        else {
            return Err(Exception::throw_message(
                &ctx,
                "`services` is not available here: this script can't call services.",
            ));
        };
        let Some(service) = read(
            &ctx,
            &self.database,
            GetCallableServiceCommand {
                script_identifier: identifier.clone(),
            },
        )?
        else {
            return Err(Exception::throw_reference(
                &ctx,
                &format!(
                    "No installed extension `{identifier}` serves scripts: it must list and be \
                     granted `service`, and declare a `service` block in its manifest."
                ),
            ));
        };
        service_object(&ctx, &self.database, host, access, service)
    }
}

/// The empty `ServiceMap` the static declarations end with, which the
/// installed services' runtime declarations merge into.
pub struct ServiceMap;

impl TsDocumentation for ServiceMap {
    const TS: &'static str = "/** The installed services, by their extension's identifier, \
        as `services.use` returns them. */\ninterface ServiceMap {}";
    const TS_BRIEF: &'static str = Self::TS;
}

impl Global for ServicesGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .globals()
            .set("services", ServicesGlobal { database })?;
        Ok(())
    }
}

/// A frozen object holding one function per name `service` declares.
fn service_object<'js>(
    ctx: &Ctx<'js>,
    database: &GuardedDatabaseContext,
    host: Arc<ServiceHost>,
    access: Arc<dyn ServiceAccess>,
    service: InstalledService,
) -> rquickjs::Result<Object<'js>> {
    // In an extension's scope, the scope each call blocks until it returns.
    let caller = database.extension().cloned();
    let read_only = !database.may_write(WriteArea::UserData);
    let script_identifier: Arc<str> = service.script_identifier.into();
    let object = Object::new(ctx.clone())?;
    for function in service.functions {
        let target = Target {
            extension_id: service.extension_id,
            script_identifier: script_identifier.clone(),
            name: function.name.clone(),
            takes_args: function.args.is_some(),
        };
        let host = host.clone();
        let access = access.clone();
        let caller = caller.clone();
        let callable = Function::new(ctx.clone(), move |ctx: Ctx<'js>, args: Opt<Value<'js>>| {
            call(
                &ctx,
                &host,
                &access,
                caller.as_deref().map(String::as_str),
                &target,
                args.0,
                read_only,
            )
        })?
        .with_name(&function.name)?;
        object.set(function.name, callable)?;
    }
    let freeze: Function = ctx.globals().get::<_, Object>("Object")?.get("freeze")?;
    freeze.call::<_, Value>((object.clone(),))?;
    Ok(object)
}

/// The service function a `services.use` function calls.
struct Target {
    extension_id: i64,
    script_identifier: Arc<str>,
    name: String,
    /// Whether it declares an argument; one that doesn't is sent `null`.
    takes_args: bool,
}

/// Calls `target` with `args` as JSON and answers what it returned, giving up
/// when the calling script's time runs out.
///
/// Throws when `args` has no JSON, when `caller`'s scope would wait on itself,
/// or when the call fails.
fn call<'js>(
    ctx: &Ctx<'js>,
    host: &ServiceHost,
    access: &Arc<dyn ServiceAccess>,
    caller: Option<&str>,
    target: &Target,
    args: Option<Value<'js>>,
    read_only: bool,
) -> rquickjs::Result<Value<'js>> {
    let args = match args.filter(|args| target.takes_args && !args.is_undefined()) {
        Some(args) => serde_json::Value::from_js_field(ctx, args)?,
        None => serde_json::Value::Null,
    };
    let _wait = match caller {
        Some(caller) => Some(
            host.waits
                .wait(caller, &target.script_identifier)
                .ok_or_else(|| {
                    Exception::throw_message(
                        ctx,
                        &format!(
                            "`services.use(\"{0}\").{1}` would wait on {0}, which is already \
                             waiting on this call: a service can't call back into itself, \
                             directly or through other services.",
                            target.script_identifier, target.name
                        ),
                    )
                })?,
        ),
        None => None,
    };
    let deadline = ScriptDeadline::remaining_until(ctx)?;
    let result = access
        .call(ServiceCall {
            extension_id: target.extension_id,
            name: target.name.clone(),
            args,
            read_only,
            deadline,
        })
        .map_err(|error| throw_server_error(ctx, error))?;
    ctx.json_parse(serde_json::to_string(&result).expect("JSON values serialize"))
}
