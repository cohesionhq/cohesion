//! A sandbox: one QuickJS runtime and context with the script API mounted.
//!
//! A runtime, not just a context, per script: rquickjs caches class
//! prototypes per runtime, so with a shared runtime one script could change
//! `Workspace.prototype` (or the `Object.prototype` behind it) for
//! every later script, whoever sent it. A runtime costs well under a millisecond.

use std::any::Any;
use std::cell::RefCell;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::{Duration, Instant};

use rquickjs::convert::Coerced;
use rquickjs::function::Rest;
use rquickjs::{CatchResultExt, Context, Ctx, Function, JsLifetime, Runtime, Value};
use serde::de::DeserializeOwned;

use cohesion_db::{DatabaseEventTrace, GuardedDatabaseContext, SandboxKey};
use tokio::sync::mpsc;

use super::caught_error::{caught, internal};
use super::sandbox_options::SandboxOptions;
use super::script_argument::ScriptArgument;
use super::strip_types::strip_types;
use crate::actor_access::CurrentActors;
use crate::connector_access::{ConnectorAccess, CurrentConnectors};
use crate::engine::CueEvaluation;
use crate::engine::Hosts;
use crate::globals::{
    AccountsGlobal, ActorsGlobal, AgentEffects, AgentGlobal, AgentImages, ArticleFetchMount,
    CohesionGlobal, ConnectorsGlobal, CurrentStorage, Effects, Global, ImagesGlobal,
    IntelligenceGlobal, LoggerGlobal, Mount, NetworkGlobal, NotificationsGlobal, ScopeClosing,
    ServicesGlobal, SleepMount, StorageGlobal, TimerGlobal, TimerRegistration, TimerSchedule,
    UsageGlobal,
};
use crate::script_api::{ENTITIES, mount_enums};
use crate::script_error::ScriptError;
use crate::script_output::ScriptOutput;
use crate::script_stop::ScriptStop;
use crate::service_access::CurrentServices;
use crate::traced_command::CurrentTrace;

/// When the running job's time is up, and the stop that may end it sooner,
/// stored as context userdata by each job as it starts, so a call that
/// blocks outside the interpreter, where the interrupt handler can't stop
/// it, can give up in time.
#[derive(JsLifetime)]
pub(crate) struct ScriptDeadline {
    at: Instant,
    stop: Option<ScriptStop>,
}

impl ScriptDeadline {
    /// When the running job's time is up, for a call that blocks outside
    /// the interpreter to bound itself by; throws if that has already
    /// passed. Every job sets a deadline before it runs, so a context
    /// without one counts as out of time.
    pub(crate) fn remaining_until(ctx: &Ctx<'_>) -> rquickjs::Result<Instant> {
        let deadline = ctx
            .userdata::<ScriptDeadline>()
            .map_or_else(Instant::now, |deadline| deadline.at);
        if Instant::now() >= deadline {
            return Err(rquickjs::Exception::throw_message(
                ctx,
                "This script's time ran out.",
            ));
        }
        Ok(deadline)
    }

    /// The stop the running job is held to, if it is
    /// [`stoppable`](Sandbox::stoppable).
    pub(crate) fn stop(ctx: &Ctx<'_>) -> Option<ScriptStop> {
        ctx.userdata::<ScriptDeadline>()
            .and_then(|deadline| deadline.stop.clone())
    }
}

/// Holds `U` as a sandbox context's userdata until dropped, so a job that
/// panics still removes it: on a persistent sandbox it would otherwise
/// outlive the job and apply to the next one.
struct Attached<'s, U>
where
    U: for<'js> JsLifetime<'js> + 'static,
    for<'js> <U as JsLifetime<'js>>::Changed<'static>: Any,
{
    sandbox: &'s Sandbox,
    _marker: PhantomData<U>,
}

impl<U> Attached<'_, U>
where
    U: for<'js> JsLifetime<'js> + 'static,
    for<'js> <U as JsLifetime<'js>>::Changed<'static>: Any,
{
    /// Removes the userdata and hands it back, consuming the guard.
    fn take(self) -> Option<U> {
        let value = self.remove();
        std::mem::forget(self);
        value
    }

    fn remove(&self) -> Option<U> {
        self.sandbox
            .context
            .with(|ctx| ctx.remove_userdata::<U>())
            .ok()
            .flatten()
            .map(|boxed| *boxed)
    }
}

impl<U> Drop for Attached<'_, U>
where
    U: for<'js> JsLifetime<'js> + 'static,
    for<'js> <U as JsLifetime<'js>>::Changed<'static>: Any,
{
    fn drop(&mut self) {
        // A failure to remove is ignored: this may run while unwinding, where
        // a second panic would abort the process.
        self.remove();
    }
}

/// A single runtime, thread-isolated, held to its [`SandboxOptions`].
pub struct Sandbox {
    // Declared first so the context drops before its runtime.
    context: Context,
    runtime: Runtime,
    time_limit: Duration,
    database: GuardedDatabaseContext,
    /// What stops the scripts `stoppable` runs, besides their deadline.
    stop: RefCell<Option<ScriptStop>>,
}
impl Sandbox {
    pub fn new(
        options: SandboxOptions,
        database: GuardedDatabaseContext,
        mounts: &[Mount],
        timer: Option<mpsc::UnboundedSender<TimerRegistration>>,
    ) -> Result<Self, ScriptError> {
        let runtime = Runtime::new().map_err(internal)?;
        runtime.set_memory_limit(options.memory_limit_bytes);
        let context = Context::full(&runtime).map_err(internal)?;

        context.with(|ctx| {
            if let Some(timer) = timer {
                _ = ctx.store_userdata(TimerSchedule(timer));
            }
            // Entity classes and enums go on every sandbox, whatever its
            // mounts: any global can hand out entities or take enum values.
            mount_api(&ctx).catch(&ctx).map_err(|e| caught(&ctx, e))?;
            for mount in mounts {
                let install = match mount {
                    Mount::Cohesion => CohesionGlobal::install,
                    Mount::Intelligence => IntelligenceGlobal::install,
                    Mount::Log => LoggerGlobal::install,
                    Mount::Network => NetworkGlobal::install,
                    Mount::Accounts => AccountsGlobal::install,
                    Mount::Timer => TimerGlobal::install,
                    Mount::Agent => AgentGlobal::install,
                    Mount::Connectors => ConnectorsGlobal::install,
                    Mount::Images => ImagesGlobal::install,
                    Mount::Actors => ActorsGlobal::install,
                    Mount::Services => ServicesGlobal::install,
                    Mount::Storage => StorageGlobal::install,
                    Mount::Usage => UsageGlobal::install,
                    Mount::Notifications => NotificationsGlobal::install,
                    Mount::ArticleFetch => ArticleFetchMount::install,
                    Mount::Sleep => SleepMount::install,
                };
                install(&ctx, database.clone())
                    .catch(&ctx)
                    .map_err(|e| caught(&ctx, e))?;
            }
            Ok::<(), ScriptError>(())
        })?;

        Ok(Self {
            context,
            runtime,
            time_limit: options.time_limit,
            database,
            stop: RefCell::default(),
        })
    }

    /// Starts the time limit of the job about to run, which the job's
    /// service calls are capped at too.
    fn refresh_runtime_limits(&self) {
        let deadline = Instant::now() + self.time_limit;
        let stop = self.stop.borrow().clone();
        let interrupt = stop.clone();
        self.runtime.set_interrupt_handler(Some(Box::new(move || {
            Instant::now() > deadline || interrupt.as_ref().is_some_and(ScriptStop::is_stopped)
        })));
        self.context
            .with(|ctx| ctx.store_userdata(ScriptDeadline { at: deadline, stop }))
            .expect("no userdata is borrowed between script jobs");
    }

    /// Evaluates `script`, JavaScript or TypeScript, returning its completion
    /// value as JSON.
    pub fn evaluate(&self, script: String) -> Result<ScriptOutput, ScriptError> {
        let script = strip_types(script)?;
        self.refresh_runtime_limits();
        self.context.with(|ctx| {
            let value: Value = ctx.eval(script).catch(&ctx).map_err(|e| caught(&ctx, e))?;
            to_output(&ctx, value)
        })
    }

    /// Lets this sandbox's `services` and `actors` reach what `hosts`
    /// serve, for the sandbox's life; without it, both throw.
    pub(crate) fn serve(&self, hosts: Hosts) {
        self.context.with(move |ctx| {
            ctx.store_userdata(CurrentServices(hosts.services))
                .expect("no userdata is borrowed outside a job");
            ctx.store_userdata(CurrentActors(hosts.actors))
                .expect("no userdata is borrowed outside a job");
        });
    }

    /// Lets the scope this persistent sandbox serves cut its scripts' sleeps
    /// short with `closing`, for the sandbox's life.
    pub(crate) fn closing_with(&self, closing: ScriptStop) {
        self.context.with(move |ctx| {
            ctx.store_userdata(ScopeClosing(closing))
                .expect("no userdata is borrowed outside a job");
        });
    }

    /// Stores `value` as the context's userdata of type `U` until the
    /// returned guard is dropped, panic or not.
    fn attach<U>(&self, value: U) -> Attached<'_, U>
    where
        U: for<'js> JsLifetime<'js> + 'static,
        for<'js> <U as JsLifetime<'js>>::Changed<'static>: Any,
    {
        self.context
            .with(move |ctx| ctx.store_userdata(value))
            .expect("no userdata is borrowed between script jobs");
        Attached {
            sandbox: self,
            _marker: PhantomData,
        }
    }

    /// Whether a trace is attached, for tests of the attacher.
    #[cfg(test)]
    pub(crate) fn has_trace(&self) -> bool {
        self.context
            .with(|ctx| ctx.userdata::<CurrentTrace>().is_some())
    }

    /// Runs `exec` with `trace` attached to every write its scripts make.
    pub fn traced<T>(
        &self,
        trace: DatabaseEventTrace,
        exec: impl FnOnce(&Self) -> Result<T, ScriptError>,
    ) -> Result<T, ScriptError> {
        let _attached = self.attach(CurrentTrace(trace));
        exec(self)
    }

    /// Runs `exec` with `stop` able to interrupt its scripts; once it has,
    /// `exec` fails with [`ScriptError::Stopped`].
    pub fn stoppable<T>(
        &self,
        stop: ScriptStop,
        exec: impl FnOnce(&Self) -> Result<T, ScriptError>,
    ) -> Result<T, ScriptError> {
        if stop.is_stopped() {
            return Err(ScriptError::Stopped);
        }
        let _stoppable = Stoppable::new(self, stop.clone());
        exec(self).map_err(|e| {
            if stop.is_stopped() {
                ScriptError::Stopped
            } else {
                e
            }
        })
    }

    /// Runs `exec` with `sandbox`'s store as what its scripts' `storage`
    /// global reaches.
    pub fn storing<T>(
        &self,
        sandbox: SandboxKey,
        exec: impl FnOnce(&Self) -> Result<T, ScriptError>,
    ) -> Result<T, ScriptError> {
        let _attached = self.attach(CurrentStorage(sandbox));
        exec(self)
    }

    /// Runs `exec` with `access` as what its scripts' `connectors` global
    /// reaches connectors through; with `None`, the global throws.
    pub fn with_connectors<T>(
        &self,
        access: Option<Arc<dyn ConnectorAccess>>,
        exec: impl FnOnce(&Self) -> Result<T, ScriptError>,
    ) -> Result<T, ScriptError> {
        let _attached = access.map(|access| self.attach(CurrentConnectors(access)));
        exec(self)
    }

    /// Runs `exec`, returning its result with the effects (`E`, a
    /// [`ForegroundEffect`](crate::ForegroundEffect)) the agent globals
    /// recorded meanwhile, in call order, and the images it attached. If `exec` fails, all are
    /// dropped with it.
    pub fn collecting_effects<E: 'static, T>(
        &self,
        exec: impl FnOnce(&Self) -> Result<T, ScriptError>,
    ) -> Result<(T, Effects<E>), ScriptError> {
        let attached = self.attach(AgentEffects::<E>::default());
        let attaching = self.attach(AgentImages::default());
        let result = exec(self);
        let requests = attached
            .take()
            .map(|e| e.0.into_inner())
            .unwrap_or_default();
        let images = attaching
            .take()
            .map(|i| i.0.into_inner())
            .unwrap_or_default();
        result.map(|output| (output, Effects { requests, images }))
    }

    /// Evaluates `code` (JavaScript or TypeScript) to a function and calls
    /// it with each of `arguments` inflated into its JS entity. Returns
    /// whether it returned something truthy, the first argument as JSON
    /// (`null` when there is none), taken before the call, and the payload:
    /// that subject when the function returned exactly `true`, otherwise
    /// what it returned, as JSON.
    pub fn evaluate_cue(
        &self,
        code: String,
        arguments: &[ScriptArgument],
    ) -> Result<CueEvaluation, ScriptError> {
        let code = strip_types(code)?;
        self.refresh_runtime_limits();
        self.context.with(|ctx| {
            let value: Value = ctx.eval(code).catch(&ctx).map_err(|e| caught(&ctx, e))?;
            let Some(function) = value.into_function() else {
                return Err(ScriptError::Exception {
                    message: "A cue's code must be a function expression, such as \
                              `(task) => task.priority === TaskPriority.urgent`."
                        .to_string(),
                    stack: None,
                });
            };
            let arguments = arguments
                .iter()
                .map(|argument| argument.inflate(&ctx, &self.database))
                .collect::<Result<Vec<_>, _>>()?;
            let subject = match arguments.first() {
                Some(subject) => to_output(&ctx, subject.clone())?,
                None => ScriptOutput("null".to_string()),
            };
            let result: Value = function
                .call((Rest(arguments),))
                .catch(&ctx)
                .map_err(|e| caught(&ctx, e))?;
            let payload = match result.as_bool() {
                Some(true) => subject.clone(),
                _ => to_output(&ctx, result.clone())?,
            };
            let Coerced(fired) = result.get::<Coerced<bool>>().map_err(internal)?;
            Ok(CueEvaluation {
                fired,
                subject,
                payload,
            })
        })
    }

    /// Calls the global function `function_name` with no arguments,
    /// returning its result as JSON.
    pub fn call<Str: AsRef<str>>(&self, function_name: Str) -> Result<ScriptOutput, ScriptError> {
        self.call_with_arguments(function_name, &[])
    }

    /// Calls the global function `function_name`, inflating each of
    /// `arguments` into its JS entity, and returns its result as JSON.
    pub fn call_with_arguments<Str: AsRef<str>>(
        &self,
        function_name: Str,
        arguments: &[ScriptArgument],
    ) -> Result<ScriptOutput, ScriptError> {
        self.refresh_runtime_limits();
        self.context.with(|ctx| {
            let function_name = function_name.as_ref();
            let function: Function = ctx
                .globals()
                .get::<_, Option<Function>>(function_name)
                .catch(&ctx)
                .map_err(|e| caught(&ctx, e))?
                .ok_or_else(|| ScriptError::MissingFunction(function_name.to_string()))?;
            let arguments = arguments
                .iter()
                .map(|argument| argument.inflate(&ctx, &self.database))
                .collect::<Result<Vec<_>, _>>()?;
            let value: Value = function
                .call((Rest(arguments),))
                .catch(&ctx)
                .map_err(|e| caught(&ctx, e))?;
            to_output(&ctx, value)
        })
    }

    pub fn call_returning<Str: AsRef<str>, T: DeserializeOwned>(
        &self,
        function_name: Str,
    ) -> Result<T, ScriptError> {
        self.call_returning_with_arguments(function_name, &[])
    }

    /// Calls the global function `function_name` with `arguments`, as
    /// [`Self::call_with_arguments`] does, and reads its result as `T`. A
    /// result that isn't a `T` is [`ScriptError::InvalidFormat`].
    pub fn call_returning_with_arguments<Str: AsRef<str>, T: DeserializeOwned>(
        &self,
        function_name: Str,
        arguments: &[ScriptArgument],
    ) -> Result<T, ScriptError> {
        let ScriptOutput(json) = self.call_with_arguments(function_name, arguments)?;
        serde_json::from_str(&json).map_err(|e| ScriptError::InvalidFormat(e.to_string()))
    }
}

fn mount_api(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
    for define in ENTITIES {
        define(ctx, &ctx.globals())?;
    }
    mount_enums(ctx)
}

fn to_output<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> Result<ScriptOutput, ScriptError> {
    match ctx
        .json_stringify(value)
        .catch(ctx)
        .map_err(|e| caught(ctx, e))?
    {
        Some(text) => text.to_string().map(ScriptOutput).map_err(internal),
        None => Ok(ScriptOutput("undefined".to_string())),
    }
}

/// Holds a [`ScriptStop`] on its sandbox until dropped, panic or not.
struct Stoppable<'a>(&'a Sandbox);

impl<'a> Stoppable<'a> {
    fn new(sandbox: &'a Sandbox, stop: ScriptStop) -> Self {
        *sandbox.stop.borrow_mut() = Some(stop);
        Self(sandbox)
    }
}

impl Drop for Stoppable<'_> {
    fn drop(&mut self) {
        *self.0.stop.borrow_mut() = None;
    }
}
