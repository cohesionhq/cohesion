//! Every `macro_rules!` in the crate. `script_api!` is invoked in
//! `script_api.rs`, the conversion macros in `conversion/`; `mount_enum!` is
//! exported for any crate that mounts a `TextEnum` as a global.

/// Defines `$en` as a frozen global object of its cases, named after the
/// type, so scripts can write `TaskStatus.completed`.
#[macro_export]
macro_rules! mount_enum {
    ($ctx:expr, $en:ty) => {{
        let ctx: &::rquickjs::Ctx = $ctx;
        (|| -> ::rquickjs::Result<()> {
            let object = ::rquickjs::Object::new(ctx.clone())?;
            for (key, text) in <$en>::JS_CASES {
                object.set(*key, *text)?;
            }
            unsafe { ::rquickjs::qjs::JS_FreezeObject(ctx.as_raw().as_ptr(), object.as_raw()) };
            ctx.globals().set(stringify!($en), object)
        })()
    }};
}

/// The whole script API, listed once by module so its parts cannot drift
/// apart: each module names its types by kind, and this generates the
/// module's [`ScriptModule`](crate::script_api::ScriptModule) const,
/// [`MODULES`](crate::script_api::MODULES),
/// [`ENTITIES`](crate::script_api::ENTITIES),
/// [`mount_enums`](crate::script_api::mount_enums), a
/// [`ToJs`](crate::conversion::ToJs) and
/// [`FromJsField`](crate::conversion::FromJsField) impl per text enum and
/// [`TS_DOCUMENTATION`](crate::script_api::TS_DOCUMENTATION). A type listed
/// in no module, or in two, is missing from the API or defined twice.
/// Invoked once, in `script_api.rs`, where the names it expands to are in
/// scope.
macro_rules! script_api {
    (@load upfront) => { Load::Upfront };
    (@load on_demand) => { Load::OnDemand };
    ($(
        $(#[$doc:meta])*
        $module:ident = $name:literal, $load:ident, $summary:literal {
            $(globals: [$($global:ty),* $(,)?],)?
            $(entities: [$($entity:ty),* $(,)?],)?
            $(enums: [$($enum:ty),* $(,)?],)?
            $(options: [$($options:ty),* $(,)?],)?
            $(interfaces: [$($interface:ty),* $(,)?],)?
            $(mounted: [$($mounted:ty),* $(,)?],)?
        }
    )*) => {
        $(
            $(#[$doc])*
            pub const $module: ScriptModule = ScriptModule {
                name: $name,
                summary: $summary,
                load: script_api!(@load $load),
                declarations: &[
                    $($(<$global as TsDocumentation>::TS,)*)?
                    $($(<$entity as TsDocumentation>::TS,)*)?
                    $($(<$enum as TsDocumentation>::TS,)*)?
                    $($(<$options as TsDocumentation>::TS,)*)?
                    $($(<$interface as TsDocumentation>::TS,)*)?
                    $($(<$mounted>::TS_GLOBAL,)*)?
                ],
                brief: &[
                    $($(<$global as TsDocumentation>::TS_BRIEF,)*)?
                    $($(<$entity as TsDocumentation>::TS_BRIEF,)*)?
                    $($(<$enum as TsDocumentation>::TS_BRIEF,)*)?
                    $($(<$options as TsDocumentation>::TS_BRIEF,)*)?
                    $($(<$interface as TsDocumentation>::TS_BRIEF,)*)?
                    $($(<$mounted>::TS_GLOBAL_BRIEF,)*)?
                ],
            };
        )*

        /// Every module, in the order [`TS_DECLARATIONS`] lists them.
        pub const MODULES: &[ScriptModule] = &[$($module,)*];

        /// Every entity class, exposed as the global `<Name>` for `instanceof`.
        pub const ENTITIES: &[Definer] = &[$($($(define::<$entity>,)*)?)*];

        /// Defines each mounted enum as a frozen global of its cases, so
        /// scripts can write `TaskStatus.completed`.
        pub fn mount_enums(ctx: &Ctx<'_>) -> rquickjs::Result<()> {
            $($($(crate::mount_enum!(ctx, $mounted)?;)*)?)*
            Ok(())
        }

        // `TextEnum`s become their stored text, as on the wire.
        $($($(impl ToJs for $enum {
            fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                self.as_str().into_js(ctx)
            }
        })*)?)*

        // ...and are read back strictly: unknown text throws a `TypeError`.
        $($($(impl FromJsField for $enum {
            fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
                let text = String::from_js(ctx, value)?;
                <$enum>::from_str_strict(&text)
                    .map_err(|error: ValueError| crate::conversion::throw_value_error(ctx, error))
            }
        })*)?)*

        /// The TypeScript declaration of everything scripts can see, one
        /// [`TsDocumentation`] per entry, module by module;
        /// [`TS_DECLARATIONS`] joins them.
        pub const TS_DOCUMENTATION: &[&str] = &[$(
            $($(<$global as TsDocumentation>::TS,)*)?
            $($(<$entity as TsDocumentation>::TS,)*)?
            $($(<$enum as TsDocumentation>::TS,)*)?
            $($(<$options as TsDocumentation>::TS,)*)?
            $($(<$interface as TsDocumentation>::TS,)*)?
            $($(<$mounted>::TS_GLOBAL,)*)?
        )*];
    };
}

/// Types whose `IntoJs` already gives the right JS value.
macro_rules! to_js_via_into_js {
    ($($ty:ty),* $(,)?) => {$(
        impl ToJs for $ty {
            fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
                // An i64 past 2^53 loses precision as a JS number; ids and timestamps stay well below.
                self.clone().into_js(ctx)
            }
        }
    )*};
}

/// Types whose `FromJs` already reads the right JS value.
macro_rules! from_js_field_via_from_js {
    ($($ty:ty),* $(,)?) => {$(
        impl FromJsField for $ty {
            fn from_js_field<'js>(ctx: &Ctx<'js>, value: Value<'js>) -> rquickjs::Result<Self> {
                <$ty>::from_js(ctx, value)
            }
        }
    )*};
}
