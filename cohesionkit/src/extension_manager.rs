//! The one way to an extension's scoped worker: installing and uninstalling
//! extensions, changing their grants, starting their scopes on first use and
//! calling their functions there, among them services' `serviceRequest` for
//! clients and scripts, queueing their `onLaunch`, and the updates
//! registries offer for them.

mod extension_error;
mod extension_fn;
mod extension_updates;
mod registry_fns;
mod script_services;
mod service_call;
mod service_manifest;

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex};

use cohesion_core::log::Module;
use cohesion_core::log_error;
use cohesion_core::model::{
    ConnectorDefinition, Extension, ExtensionManifest, ExtensionPermission,
    ExtensionPermissionKind, HookKind, ServiceDescription,
};
use cohesion_core::protocol::{
    ExtensionUpdate, ServerError, SetExtensionHookRequest, SetExtensionPermissionRequest,
};
use cohesion_db::{
    CommandError, DatabaseContext, DeleteExtensionCommand, GetExtensionCommand, GetExtensionQuery,
    GetExtensionsWithPermissionCommand, InstallExtensionCommand, IsExtensionHookEnabledCommand,
    ListExtensionPermissionsCommand, SandboxKey, SetExtensionHookCommand,
    SetExtensionPermissionCommand, WriteArea,
};
use cohesion_js::{
    Sandbox, ScopeHandle, ScriptArgument, ScriptEngine, ScriptError, globals::Mount,
};
use serde::de::DeserializeOwned;

use crate::connectors::ServiceDescriptionFn;
use crate::extension_icon::accepted_icon;
use crate::hooks::{HookManager, LaunchHook};

pub(crate) use extension_error::{ExtensionError, script_fault};
pub(crate) use extension_fn::ExtensionFn;
pub(crate) use registry_fns::RegistryResultsFn;
pub(crate) use script_services::ScriptServices;
pub(crate) use service_call::ServiceCaller;

/// Starts, calls and restarts extensions' scopes, by extension id. A cheap
/// clone; clones share the scopes.
#[derive(Clone)]
pub(crate) struct ExtensionManager {
    inner: Arc<Inner>,
}

struct Inner {
    database: DatabaseContext,
    engine: ScriptEngine,
    hooks: HookManager,
    /// One per extension id ever used, never removed, so every caller of an
    /// id shares one lock.
    slots: Mutex<HashMap<i64, Arc<tokio::sync::Mutex<Slot>>>>,
    /// The extensions with a launch queued and not yet run.
    launches: Mutex<HashSet<i64>>,
    /// What the last update check found.
    updates: Mutex<Vec<ExtensionUpdate>>,
}

/// An extension's place in the manager. Its lock is held while the scope is
/// started or shut down, never during a call, so a restart waits for a spawn
/// already under way and then discards what it started.
#[derive(Default)]
struct Slot {
    /// The scope this slot started, while it may still be running.
    scope: Option<SandboxKey>,
}

impl ExtensionManager {
    pub(crate) fn new(database: DatabaseContext, engine: ScriptEngine, hooks: HookManager) -> Self {
        Self {
            inner: Arc::new(Inner {
                database,
                engine,
                hooks,
                slots: Mutex::default(),
                launches: Mutex::default(),
                updates: Mutex::default(),
            }),
        }
    }

    /// Runs `job` on extension `extension_id`'s scope, starting the scope
    /// from the installed script first if it isn't running. Fails without
    /// running anything when the extension isn't installed or lacks
    /// `requires` (or what it [needs](ExtensionPermissionKind::needs)),
    /// checked now rather than when the caller read it.
    pub(crate) async fn scoped<T: Send + 'static>(
        &self,
        extension_id: i64,
        requires: Option<ExtensionPermissionKind>,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<T, ExtensionError> {
        let scope = self.running_scope(extension_id, requires, None).await?;
        let scope = scope.expect("a call that needs no hook always finds a scope");
        Ok(scope.run(job).await?)
    }

    /// Runs `job` on extension `extension_id`'s scope, as
    /// [`scoped`](Self::scoped) does, provided its `hook` is enabled when
    /// the call is made; `None` when it isn't, without running anything.
    pub(crate) async fn hook<T: Send + 'static>(
        &self,
        extension_id: i64,
        hook: HookKind,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<Option<T>, ExtensionError> {
        match self.running_scope(extension_id, None, Some(&hook)).await? {
            Some(scope) => Ok(Some(scope.run(job).await?)),
            None => Ok(None),
        }
    }

    /// Calls extension `extension_id`'s global `function` on its scope, as
    /// [`scoped`](Self::scoped) does, provided it holds what `function`
    /// requires.
    pub(crate) async fn call<F: ExtensionFn>(
        &self,
        extension_id: i64,
        function: F,
    ) -> Result<F::Output, ExtensionError> {
        let requires = Some(F::REQUIRES);
        self.call_function(extension_id, requires, F::NAME, function.arguments())
            .await
    }

    /// Calls extension `extension_id`'s global `function`, as
    /// [`call`](Self::call) does, ignoring what it returns. An extension
    /// that doesn't define it is skipped.
    pub(crate) async fn notify<F: ExtensionFn<Output = ()>>(
        &self,
        extension_id: i64,
        function: F,
    ) -> Result<(), ExtensionError> {
        let arguments = function.arguments();
        self.scoped(
            extension_id,
            Some(F::REQUIRES),
            move |sandbox| match sandbox.call_with_arguments(F::NAME, &arguments) {
                Ok(_) | Err(ScriptError::MissingFunction(_)) => Ok(()),
                Err(e) => Err(e),
            },
        )
        .await
    }

    /// [`call`](Self::call) by name, reading the result as a `T`.
    async fn call_function<T: DeserializeOwned + Send + 'static>(
        &self,
        extension_id: i64,
        requires: Option<ExtensionPermissionKind>,
        function: &'static str,
        arguments: Vec<ScriptArgument>,
    ) -> Result<T, ExtensionError> {
        self.scoped(extension_id, requires, move |sandbox| {
            sandbox.call_returning_with_arguments(function, &arguments)
        })
        .await
    }

    /// Runs `call` on each extension granted `permission` at once, since
    /// each runs in its own scope, and answers their results in priority
    /// order. A call that panics is logged as `what` panicking and fails the
    /// request.
    pub(crate) async fn call_each<T, F>(
        &self,
        permission: ExtensionPermissionKind,
        what: &str,
        call: impl Fn(Extension) -> F,
    ) -> Result<Vec<T>, ServerError>
    where
        T: Send + 'static,
        F: Future<Output = T> + Send + 'static,
    {
        let extensions = self
            .inner
            .database
            .send(GetExtensionsWithPermissionCommand { permission })
            .await?;
        let calls: Vec<_> = extensions
            .into_iter()
            .map(|extension| tokio::spawn(call(extension)))
            .collect();
        let mut results = Vec::with_capacity(calls.len());
        for call in calls {
            results.push(call.await.map_err(|e| {
                log_error!(Module::Extensions, "{what} panicked: {e}");
                ServerError::internal()
            })?);
        }
        Ok(results)
    }

    /// Extension `extension_id`, provided it has `requires` enabled, and
    /// what it [needs](ExtensionPermissionKind::needs).
    pub(crate) async fn granted(
        &self,
        extension_id: i64,
        requires: ExtensionPermissionKind,
    ) -> Result<Extension, ExtensionError> {
        Ok(self.read(extension_id, Some(requires)).await?.0)
    }

    /// Evaluates `script` in the isolated sandbox and reads what its
    /// `manifest()` returns, dropping an icon the daemon doesn't accept and
    /// collapsing its service declaration's text to single lines. A script
    /// that throws, lacks `manifest`, returns the wrong shape, lists
    /// `connector` without `oauth`, or declares a service it may not is the
    /// caller's to fix.
    pub(crate) async fn read_manifest(
        &self,
        script: String,
    ) -> Result<ExtensionManifest, ServerError> {
        Ok(self.read_script(script).await?.0)
    }

    /// [`read_manifest`](Self::read_manifest), and for a script that lists
    /// `oauth`, what its `serviceDescription()` returns, read in the same
    /// isolated sandbox.
    async fn read_script(
        &self,
        script: String,
    ) -> Result<(ExtensionManifest, Option<ServiceDescription>), ServerError> {
        let read = self
            .inner
            .engine
            .isolated(|sandbox| {
                sandbox.evaluate(script)?;
                let manifest = sandbox.call_returning::<_, ExtensionManifest>("manifest")?;
                let service = if manifest
                    .permissions
                    .contains(&ExtensionPermissionKind::OAuth)
                {
                    Some(ServiceDescriptionFn.call_in(sandbox)?)
                } else {
                    None
                };
                Ok((manifest, service))
            })
            .await;
        let (mut manifest, service) = read.map_err(|e| {
            script_fault("A new extension", "manifest", e, |message| message)
                .map_or_else(ServerError::internal, |message| {
                    ServerError::validation(None, message)
                })
        })?;
        let unpaired = manifest.permissions.iter().find_map(|permission| {
            let needed = permission.needs()?;
            (!manifest.permissions.contains(&needed)).then_some((permission, needed))
        });
        if let Some((permission, needed)) = unpaired {
            return Err(ServerError::validation(
                None,
                format!("`{}` requires `{}`.", permission.as_str(), needed.as_str()),
            ));
        }
        service_manifest::check_service(&mut manifest)?;
        manifest.icon = accepted_icon(manifest.icon);
        Ok((manifest, service))
    }

    /// Installs `script`, or replaces the installed extension with its
    /// identifier when `allow_overwrite`, then restarts it.
    pub(crate) async fn install(
        &self,
        script: String,
        allow_overwrite: bool,
    ) -> Result<Extension, ServerError> {
        let (manifest, service) = self.read_script(script.clone()).await?;
        let connector = service.map(|description| ConnectorDefinition {
            identifier: manifest.identifier.clone(),
            icon: manifest.icon.clone(),
            version: manifest.version,
            description,
        });
        let extension = self
            .inner
            .database
            .send(InstallExtensionCommand {
                script,
                manifest,
                connector,
                allow_overwrite,
            })
            .await?;
        self.restart(extension.id).await?;
        Ok(extension)
    }

    /// Enables or disables a permission, then restarts the extension so its
    /// scope respawns with the mounts the grants now allow.
    pub(crate) async fn set_permission(
        &self,
        request: SetExtensionPermissionRequest,
    ) -> Result<(), ServerError> {
        let extension_id = request.extension_id;
        self.inner
            .database
            .send(SetExtensionPermissionCommand::from(request))
            .await?;
        Ok(self.restart(extension_id).await?)
    }

    /// Enables or disables a hook, then restarts the extension.
    pub(crate) async fn set_hook(
        &self,
        request: SetExtensionHookRequest,
    ) -> Result<(), ServerError> {
        let extension_id = request.extension_id;
        self.inner
            .database
            .send(SetExtensionHookCommand::from(request))
            .await?;
        Ok(self.restart(extension_id).await?)
    }

    /// Deletes the extension, stops its scope and its timers, and forgets
    /// the updates found for it or offered by it.
    pub(crate) async fn uninstall(&self, extension_id: i64) -> Result<(), ServerError> {
        let slot = self.slot(extension_id);
        let mut slot = slot.lock().await;
        let extension = self
            .inner
            .database
            .send(DeleteExtensionCommand { extension_id })
            .await?;
        slot.scope = None;
        self.inner
            .engine
            .shutdown_scope(&SandboxKey::Extension(extension.script_identifier));
        self.inner.launches.lock().unwrap().remove(&extension_id);
        self.inner.updates.lock().unwrap().retain(|update| {
            update.extension_id != extension_id && update.registry_extension_id != extension_id
        });
        Ok(())
    }

    /// Takes the launch queued for `extension_id`, so the next restart can
    /// queue another, and answers whether its `launch` hook is still
    /// enabled, which the launch needs to run.
    pub(crate) async fn claim_launch(&self, extension_id: i64) -> Result<bool, CommandError> {
        self.inner.launches.lock().unwrap().remove(&extension_id);
        self.launches(extension_id).await
    }

    /// Discards the extension's scope, so the next call respawns it with its
    /// current script and grants, then queues `onLaunch` if the `launch`
    /// hook is enabled, since timers die with the scope.
    async fn restart(&self, extension_id: i64) -> Result<(), CommandError> {
        {
            let slot = self.slot(extension_id);
            let mut slot = slot.lock().await;
            if let Some(scope) = slot.scope.take() {
                self.inner.engine.shutdown_scope(&scope);
            }
        }
        if self.launches(extension_id).await? {
            self.launch(extension_id);
        }
        Ok(())
    }

    /// Queues `onLaunch` for the extension, unless one is already queued.
    fn launch(&self, extension_id: i64) {
        if self.inner.launches.lock().unwrap().insert(extension_id) {
            self.inner.hooks.send_scoped(LaunchHook {}, extension_id);
        }
    }

    /// Whether the extension's `launch` hook is enabled.
    async fn launches(&self, extension_id: i64) -> Result<bool, CommandError> {
        self.inner
            .database
            .send(IsExtensionHookEnabledCommand {
                extension_id,
                hook: HookKind::Launch,
            })
            .await
    }

    fn slot(&self, extension_id: i64) -> Arc<tokio::sync::Mutex<Slot>> {
        self.inner
            .slots
            .lock()
            .unwrap()
            .entry(extension_id)
            .or_default()
            .clone()
    }

    /// A handle on the extension's running scope, after checking
    /// `requires`, or `None` when `hook` is given and not enabled; when the
    /// scope isn't running, starts it from the extension as it is installed
    /// now. Both checks are made under the slot's lock, which a grant
    /// change's restart takes after committing. A call that checked first
    /// keeps its handle, so its job still runs on the scope it found, as a
    /// call already under way does; every call after the restart sees the
    /// change.
    async fn running_scope(
        &self,
        extension_id: i64,
        requires: Option<ExtensionPermissionKind>,
        hook: Option<&HookKind>,
    ) -> Result<Option<ScopeHandle>, ExtensionError> {
        let slot = self.slot(extension_id);
        let mut slot = slot.lock().await;
        if let Some(hook) = hook {
            let enabled = self
                .inner
                .database
                .send(IsExtensionHookEnabledCommand {
                    extension_id,
                    hook: hook.clone(),
                })
                .await?;
            if !enabled {
                return Ok(None);
            }
        }
        let running = slot
            .scope
            .as_ref()
            .and_then(|scope| self.inner.engine.scope(scope));
        if let Some(handle) = running {
            if requires.is_some() {
                self.read(extension_id, requires).await?;
            }
            return Ok(Some(handle));
        }
        let (extension, permissions) = self.read(extension_id, requires).await?;
        let scope = SandboxKey::Extension(extension.script_identifier.clone());
        let handle = self.spawn(extension, permissions).await?;
        slot.scope = Some(scope);
        Ok(Some(handle))
    }

    /// The extension and every permission recorded for it, enabled or not.
    async fn read_grants(
        &self,
        extension_id: i64,
    ) -> Result<(Extension, Vec<ExtensionPermission>), ExtensionError> {
        let extension = self
            .inner
            .database
            .send(GetExtensionCommand {
                query: GetExtensionQuery::Id(extension_id),
            })
            .await?;
        let grants = self
            .inner
            .database
            .send(ListExtensionPermissionsCommand {
                extension_ids: vec![extension_id],
            })
            .await?;
        Ok((extension, grants))
    }

    /// The extension and its enabled permissions, provided they include
    /// `requires` and what it [needs](ExtensionPermissionKind::needs).
    async fn read(
        &self,
        extension_id: i64,
        requires: Option<ExtensionPermissionKind>,
    ) -> Result<(Extension, Vec<ExtensionPermissionKind>), ExtensionError> {
        let (extension, grants) = self.read_grants(extension_id).await?;
        let permissions = enabled(grants);
        require(&permissions, requires)?;
        Ok((extension, permissions))
    }

    /// Runs `job` on `extension`'s scope, started with `permissions` if it
    /// isn't running, as [`scoped`](Self::scoped) does for grants the caller
    /// has just read and checked.
    pub(crate) async fn scoped_with<T: Send + 'static>(
        &self,
        extension: Extension,
        permissions: Vec<ExtensionPermissionKind>,
        job: impl FnOnce(&Sandbox) -> Result<T, ScriptError> + Send + 'static,
    ) -> Result<T, ExtensionError> {
        let slot = self.slot(extension.id);
        let mut slot = slot.lock().await;
        let running = slot
            .scope
            .as_ref()
            .and_then(|scope| self.inner.engine.scope(scope));
        let handle = match running {
            Some(handle) => handle,
            None => {
                let scope = SandboxKey::Extension(extension.script_identifier.clone());
                let handle = self.spawn(extension, permissions).await?;
                slot.scope = Some(scope);
                handle
            }
        };
        drop(slot);
        Ok(handle.run(job).await?)
    }

    /// Starts the extension's scope with the globals `permissions` mount,
    /// running its top-level script, and answers a handle on it.
    async fn spawn(
        &self,
        extension: Extension,
        permissions: Vec<ExtensionPermissionKind>,
    ) -> Result<ScopeHandle, ScriptError> {
        let mut mounts = Vec::new();
        for mount in permissions.iter().filter_map(Mount::for_permission) {
            if !mounts.contains(&mount) {
                mounts.push(mount);
            }
        }
        mounts.extend([Mount::Log, Mount::Storage, Mount::Sleep]);
        let database = self
            .inner
            .database
            .writing(permissions.iter().filter_map(WriteArea::for_permission))
            .for_extension(Arc::new(extension.script_identifier.clone()));
        self.inner
            .engine
            .spawn_scope(
                SandboxKey::Extension(extension.script_identifier),
                extension.script,
                Cow::Owned(mounts),
                database,
            )
            .await
    }
}

/// The permissions among `grants` that are enabled.
fn enabled(grants: Vec<ExtensionPermission>) -> Vec<ExtensionPermissionKind> {
    grants
        .into_iter()
        .filter(|grant| grant.enabled)
        .map(|grant| grant.permission)
        .collect()
}

/// Fails naming `requires`, or the first permission it
/// [needs](ExtensionPermissionKind::needs), when `permissions` lacks it.
fn require(
    permissions: &[ExtensionPermissionKind],
    requires: Option<ExtensionPermissionKind>,
) -> Result<(), ExtensionError> {
    let mut next = requires;
    while let Some(permission) = next {
        if !permissions.contains(&permission) {
            return Err(ExtensionError::NotGranted(permission));
        }
        next = permission.needs();
    }
    Ok(())
}

#[cfg(test)]
mod tests;
