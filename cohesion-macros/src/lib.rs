//! Derives for cohesion's models and commands. Kept in their own crate
//! because proc macros must be; `cohesion-core` re-exports them so users never
//! depend on this crate directly. This root holds only the macro entry
//! points; each one's expansion lives in its own module.

use proc_macro::TokenStream;
use syn::{DeriveInput, LitStr, parse_macro_input};

mod column_group_derive;
mod column_ref;
mod commands_derive;
mod derive_input;
mod fields_derive;
mod from_request_derive;
mod identifier_case;
mod joins;
mod js_entity_attribute;
mod js_entity_expand;
mod js_global_attribute;
mod js_options_derive;
mod model_derive;
mod projection_derive;
mod replies_derive;
mod text_enum_derive;
mod text_enum_rename_rule;
mod ts_declaration;
mod ts_interface_derive;
mod ts_method_members;
mod ts_type_mapping;
mod ts_wire_derive;

use js_entity_attribute::JsEntityArgs;
use js_entity_expand::JsEntityExpand;

/// Implements `cohesion_core::Model` for a struct with named fields.
///
/// ```ignore
/// #[derive(Model)]
/// #[model(table = "workspaces")]
/// pub struct Workspace { pub id: i64, pub title: String }
/// ```
///
/// It also implements `cohesion_core::stored_model::Named`: the struct's
/// name in lower words (`TaskFolder` is "task folder"), as a client's
/// messages call the model.
///
/// Every field is a column named after it, unless marked `#[model(skip)]`, in
/// which case it is filled with `Default::default()` when read. A column
/// `deleted: bool` also implements `cohesion_core::stored_model::SoftDelete`.
///
/// `#[model(json)]` stores a serde type as JSON text; a stored value that
/// doesn't parse is a `RowError`. `#[model(group)]` holds several nullable
/// columns as one field of a `#[derive(ColumnGroup)]` enum.
///
/// ```ignore
/// pub struct Feed {
///     pub id: i64,
///     #[model(json)]
///     pub status: FeedStatus,
///     #[model(group)]
///     pub backend: EndpointBackend,
/// }
/// ```
///
/// A model can read through other tables. `join = Model, on = "..."` (or
/// `left_join`) adds a join to every read of the model; `#[model(expr =
/// "...")]` reads a stored column from a SQL expression (a literal, or a path
/// to a `&str` constant) and `#[model(or_from = Model)]` from the column or,
/// where that is `NULL`, the joined model's. Writes still set the stored
/// column; only reads differ, for `select`, `select_live` and the row an
/// `Insert` or `Update` returns. Such a model does not implement `SingleTable`,
/// so `Delete::returning_*` is unavailable for it.
///
/// ```ignore
/// #[derive(Model)]
/// #[model(table = "intelligence_endpoints")]
/// #[model(left_join = Client, on = "clients.id = intelligence_endpoints.client_id")]
/// pub struct IntelligenceEndpoint {
///     pub id: i64,
///     #[model(expr = "COALESCE(intelligence_endpoints.name, clients.client_name)")]
///     pub name: String,
/// }
/// ```
#[proc_macro_derive(Model, attributes(model))]
pub fn derive_model(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    model_derive::model(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `cohesion_core::stored_model::Projection` for a struct with
/// named fields: a read of some of a model's columns, or of SQL expressions
/// over them, for when the whole row is more than a caller needs.
///
/// ```ignore
/// #[derive(Projection)]
/// #[projection(of = Article)]
/// pub struct ArticleSummary {
///     pub id: i64,
///     pub title: String,
///     #[projection(expr = "substr(contents, 1, 280)")]
///     pub excerpt: String,
/// }
/// ```
///
/// A plain field is the model's column of the same name, and must be one of
/// the model's fields with the same type, or the derive fails to compile. A
/// field marked `#[projection(expr = "...")]` is read from `<expr> AS
/// <field>`, which the compiler cannot check; the expression may also be a
/// path to a `&str` constant, so a filter can reuse it. `#[projection(json)]`
/// reads a column of JSON text, and `#[projection(group)]` a `ColumnGroup`
/// field of the model, from its columns.
///
/// A projection can join other models' tables, and take columns from them:
///
/// ```ignore
/// #[derive(Projection)]
/// #[projection(of = Actor)]
/// #[projection(join = Client, on = "clients.id = actors.client_id")]
/// pub struct ActorListing {
///     pub id: i64,
///     /// `clients.client_name`, checked against `Client`.
///     #[projection(from = Client)]
///     pub client_name: String,
///     #[projection(expr = "actors.product || ' on ' || clients.client_name")]
///     pub title: String,
/// }
/// ```
///
/// `#[projection(or_from = Model)]` reads `COALESCE(<table>.<field>,
/// <joined>.<field>)`: the model's own value, else the joined model's.
/// `join` is an inner join, `left_join` a left one. Base columns are
/// qualified by their table in the generated SQL, so `id` is never
/// ambiguous; an `expr` or an `on` writes its own qualified names.
#[proc_macro_derive(Projection, attributes(projection))]
pub fn derive_projection(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    projection_derive::projection(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `cohesion_core::stored_model::ColumnGroup` for an enum whose
/// variants are combinations of nullable columns of one model, so the model
/// holds them as a single field (`#[model(group)]`).
///
/// ```ignore
/// #[derive(ColumnGroup)]
/// pub enum AccountHost {
///     Extension { extension_id: i64, identifier: String },
///     Orphaned,
/// }
/// ```
///
/// A variant's fields are its columns, named after them; a column two
/// variants share is stored once. A row is the variant whose non-`Option`
/// columns are all set and whose other columns are all `NULL`; a row
/// matching no variant, or two, is a `RowError`. Writing sets every column
/// of the group, `NULL` for the other variants'.
#[proc_macro_derive(ColumnGroup)]
pub fn derive_column_group(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    column_group_derive::column_group(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Lets other macros read a struct's fields, which a macro cannot otherwise
/// see. Emits the hidden `cohesion_core::__cohesion_fields_<Name>!`, which
/// calls back a given macro with each field's `///` docs, name and type:
///
/// ```ignore
/// #[derive(Fields)]
/// pub struct Workspace { pub id: i64, pub title: String }
/// ```
///
/// A field marked `#[fields(skip)]` is left out, so `#[js_entity]` gives it
/// no getter, no `toJSON` entry and no TypeScript declaration.
///
/// `#[js_entity(Workspace)]` is the consumer: deriving `Fields` is what lets a
/// struct, table-backed or not, be exposed to scripts. Only for structs in
/// cohesion-core, where `#[js_entity]` looks for the macro.
#[proc_macro_derive(Fields, attributes(fields))]
pub fn derive_fields(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    fields_derive::fields(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `From<Request>` for a command struct by moving each field of
/// the same name out of the request named in `#[request(path::To::Request)]`.
///
/// ```ignore
/// #[derive(FromRequest)]
/// #[request(SearchRequest, ignore(page_size))]
/// pub struct SearchCommand { pub workspace_id: i64, pub query: String }
/// ```
///
/// The request is destructured exhaustively (no `..`), so a request field the
/// command neither names nor lists in `ignore(...)` is a compile error
/// ("pattern does not mention field"): adding a request field forces every
/// command to decide.
///
/// A command's `metadata` and `has_metadata` fields are never taken from the
/// request: metadata filters are for scripts, so they start empty, and a
/// request that carries either field fails to compile.
///
/// Also implements `cohesion_db::DbRequest` for the request, with this struct
/// as its `Command`, which is what routes the request to the database. It is
/// therefore meant to be used inside cohesion-db, on a `DatabaseCommand`
/// whose `Response` is the request's `Call::Reply`; each request can have
/// only one such command.
#[proc_macro_derive(FromRequest, attributes(request))]
pub fn derive_from_request(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    from_request_derive::from_request(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Stores a fieldless enum as text. Implements `as_str`, `Display`, `FromStr`,
/// `IntoValue` and `FromValue`, so the database, the command line and scripts
/// all see the same text (unless `store = "integer"`, below).
///
/// Each variant's text is its name in the `rename_all` case, which takes the
/// same spellings as serde's: `lowercase`, `UPPERCASE`, `camelCase`,
/// `snake_case`, `kebab-case` (the default) and `lower space case`
/// (`task folder`). Parsing also accepts the camelCase
/// spelling, which is what `#[serde(rename_all = "camelCase")]` puts in JSON.
///
/// `rename_all = "custom"` instead takes each variant's text from its own
/// `#[text_enum("...")]`, which every variant must have, such as
/// `task:create`. A custom enum also implements serde's `Serialize` and
/// `Deserialize` with that text, so don't derive them as well.
///
/// `JS_CASES` pairs each variant's camelCase name, usable as a JS property,
/// with its text, so `mount_enum!` can define `TaskStatus.completed`.
///
/// ```ignore
/// #[derive(TextEnum)]
/// #[text_enum(default = Incomplete, rename_all = "snake_case")]
/// pub enum TaskStatus { Incomplete, Completed }
///
/// #[derive(TextEnum)]
/// #[text_enum(rename_all = "custom")]
/// pub enum Permission {
///     #[text_enum("task:create")]
///     HookOnCreateTask,
/// }
/// ```
///
/// `store = "integer"` stores each variant as its discriminant instead, so
/// SQL can order by it, while JSON, scripts and the command line still use
/// the text. Give every variant an explicit `= n` and never renumber one:
/// the numbers are what the database holds.
///
/// ```ignore
/// #[derive(TextEnum)]
/// #[text_enum(default = None, store = "integer")]
/// pub enum TaskPriority { None = 0, Low = 1, High = 2 }
/// ```
///
/// Unknown text (or, stored as integers, an unknown number) reads as the
/// `default` variant (after logging a warning) if one is given, and is an
/// error otherwise. `from_str_strict` always errors
/// on unknown text, for input that should be rejected rather than defaulted.
#[proc_macro_derive(TextEnum, attributes(text_enum))]
pub fn derive_text_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    text_enum_derive::text_enum(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generates the glue for the protocol's `Command` enum from one
/// `#[reply(...)]` attribute per variant.
///
/// ```ignore
/// #[derive(Commands)]
/// pub enum Command {
///     #[reply(ScriptResult, via = Script)]
///     RunScript(RunScriptRequest),
///     #[reply(Vec<Workspace>, request = ListWorkspacesRequest)]
///     ListWorkspaces,
///     #[reply(Workspace)]
///     CreateWorkspace(CreateWorkspaceRequest),
/// }
/// ```
///
/// A tuple variant's single field is its request type; a unit variant names
/// it with `request = Path` (a unit struct). `via = Path` picks the route kind
/// and defaults to `cohesion_core::protocol::Database`. Generates `Call` and
/// `From<Request> for Command` for every request, and `Command::dispatch`,
/// which sends each variant to the context's `Route<Kind, Request>`, and
/// `Command::declare_ts`, the TypeScript `Requests` and `Replies` maps keyed
/// by each variant's camelCase name, documented with its `///` comments.
/// Request and reply types must implement `TsWire`.
#[proc_macro_derive(Commands, attributes(reply))]
pub fn derive_commands(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    commands_derive::commands(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Generates the glue for the protocol's `ReplyData` enum: for every variant
/// `Name(T)`, `From<T>`, `TryFrom<ReplyData> for T` (the error is the reply
/// that did not match) and `ReplyPayload for T`.
///
/// ```ignore
/// #[derive(Replies)]
/// pub enum ReplyData { Workspaces(Vec<Workspace>), Workspace(Workspace) }
/// ```
///
/// Every variant must be a single-field tuple variant, and payload types must
/// be distinct, since each one maps back to its variant. A variant
/// `Name(Box<T>)` converts from and to `T`, so a large payload can be boxed
/// without the commands that return it changing.
#[proc_macro_derive(Replies)]
pub fn derive_replies(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    replies_derive::replies(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Declares a script-facing entity class wrapping a struct, from its methods:
///
/// ```ignore
/// #[js_entity(Workspace)]
/// impl JsWorkspace {
///     pub fn delete<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<()> { ... }
/// }
/// ```
///
/// Generates `struct JsWorkspace { inner: Workspace, db: GuardedDatabaseContext }`
/// with `new`, `inner`, and `read` and `write` (which run a read or write
/// command through `db`), and one `#[rquickjs::methods]` block
/// holding the methods written here plus an enumerable getter per field
/// (camelCased, so `workspace_id` is `workspaceId`) and `toJSON`. The JS class
/// is named after the struct unless given `js = "Name"`.
///
/// `#[js_entity(Task, metadata)]` also adds `metadata(key)` and
/// `setMetadata(key, value)` (a read and a write), which store strings in the `metadata` table
/// under the model's `Model::TABLE`. Only for table-backed models; the
/// table also needs an `AFTER DELETE` trigger that removes its rows'
/// metadata.
///
/// Only for cohesion-js: the generated code uses `crate::traced_command::{read, write}`
/// and `crate::conversion::ToJs`, and every field's type must implement `ToJs`.
///
/// The struct must `#[derive(Fields)]`, whatever else it derives: its fields
/// come from the `__cohesion_fields_<Name>!` macro that derive emits, which
/// calls `__js_entity_expand!` with them. A `Model` is a table-backed case.
///
/// Also implements `cohesion_core::ts_documentation::TsDocumentation`, declaring the class
/// in the `cohesion` namespace with each column, getter, setter and method,
/// and their `///` comments. Types are mapped as written (`Vec<JsTask>` is
/// `Task[]`); where that says too little, override it with
/// `#[ts(type = "number | string")]` on an argument or
/// `#[ts(returns = "Task | Document")]` on a method, give a method type
/// parameters with `#[ts(generics = "K extends keyof ServiceMap")]`, or
/// leave a method out with `#[ts(skip)]`.
#[proc_macro_attribute]
pub fn js_entity(args: TokenStream, item: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args as JsEntityArgs);
    let item = parse_macro_input!(item as syn::ItemImpl);
    js_entity_attribute::js_entity_impl(args, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Reads a script's options object into a struct, and declares the struct
/// as a TypeScript interface:
///
/// ```ignore
/// #[derive(JsOptions)]
/// pub struct TaskListOptions {
///     /// Which statuses to include.
///     pub status: Option<OneOrMore<TaskStatus>>,
/// }
/// ```
///
/// Implements rquickjs's `FromJs`, so a method takes `options: TaskListOptions`,
/// or `options: Opt<TaskListOptions>` to make the whole argument optional.
/// `undefined` or `null` reads as `{}`, since `Opt` only covers an argument
/// left out, not one passed as `undefined`.
/// Each field is read from the property of its camelCased name (`api_key` is
/// `apiKey`), or the name given by `#[js_options(rename = "baseURL")]`,
/// through `crate::conversion::FromJsField`. An `Option` field may be left out
/// (or `null`); leaving out any other field throws a `TypeError` naming it.
///
/// Also implements `cohesion_core::ts_documentation::TsDocumentation`, declaring
/// `interface TaskListOptions` with each field's `///` comments; an `Option`
/// field is declared optional (`status?: ...`). Methods taking the struct
/// name it as the argument's type.
///
/// An enum of such structs is an object whose shape depends on a tag
/// property, valued with each variant's camelCase name:
///
/// ```ignore
/// #[derive(JsOptions)]
/// #[js_options(tag = "kind")]
/// pub enum EndpointOptions {
///     Provider(ProviderEndpointOptions),
///     Custom(CustomEndpointOptions),
/// }
/// ```
///
/// `{ kind: "custom", ... }` reads as `Custom`, with the variant's struct
/// read from the same object. It is declared as a union TypeScript narrows
/// on, `({ kind: "provider" } & ProviderEndpointOptions) | ...`, so document
/// each shape on its struct.
///
/// Only for cohesion-js, like `#[js_entity]`; list every options type under
/// `options` in `script_api!` (an enum's structs too) so its declaration
/// reaches `TS_DECLARATIONS`.
#[proc_macro_derive(JsOptions, attributes(js_options))]
pub fn derive_js_options(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    js_options_derive::js_options(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `cohesion_core::ts_documentation::TsDocumentation` for a struct a script
/// returns and the host reads with serde, declaring `interface Name` with
/// each field's `///` comments. Fields keep their Rust names, as serde reads
/// them, except that one with `#[serde(alias = "...")]` or
/// `#[serde(rename = "...")]` is declared under that key, the one scripts
/// write; an `Option` field is declared optional.
///
/// An enum tagged with `#[serde(tag = "kind", rename_all = "camelCase")]`,
/// each variant wrapping one such struct, is declared as the union
/// TypeScript narrows on, `({ kind: "text" } & TextBlock) | ...`.
///
/// ```ignore
/// #[derive(Deserialize, TsInterface)]
/// pub struct ExtensionManifest { pub identifier: String, ... }
/// ```
#[proc_macro_derive(TsInterface)]
pub fn derive_ts_interface(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ts_interface_derive::ts_interface(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `cohesion_core::ts_wire::TsWire` for a type clients send or
/// receive, declaring it as the TypeScript its serde JSON matches, with its
/// `///` comments:
///
/// ```ignore
/// #[derive(Serialize, Deserialize, TsWire)]
/// #[ts_wire(input)]
/// pub struct ListThingsRequest { pub workspace_id: i64, pub limit: Option<i64> }
/// ```
///
/// Follows serde's `rename_all = "camelCase"`, `tag`, `content`,
/// `transparent`, `flatten`, `default`, `skip_serializing_if`, `rename` and
/// `untagged` (and ignores `alias`, which only adds a key serde reads), and `#[derive(TextEnum)]`'s custom texts; any other serde
/// attribute is a compile error. An `Option` field is `T | null`; one marked
/// `default` or `skip_serializing_if` may also be absent, as may every `Option`
/// field of a type marked `#[ts_wire(input)]`, since serde reads a missing
/// one as `None`. A type serde converts (`from`, `try_from`, `into`) gives its JSON as
/// `#[ts_wire(type = "'any' | number")]`.
///
/// Every field's type must implement `TsWire` too; `declare` adds the
/// declarations of the types a type refers to, so declaring `Command`'s
/// requests and replies reaches every type the protocol uses.
#[proc_macro_derive(TsWire, attributes(ts_wire))]
pub fn derive_ts_wire(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    ts_wire_derive::ts_wire(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Implements `cohesion_core::ts_documentation::TsDocumentation` for a host object's
/// `#[rquickjs::methods]` block, declaring its methods as functions in the
/// namespace named by the argument, which is where scripts find the object:
///
/// ```ignore
/// #[js_global("cohesion")]
/// #[rquickjs::methods]
/// impl CohesionGlobal {
///     /// Every live workspace.
///     pub fn workspaces<'js>(&self, ctx: Ctx<'js>) -> rquickjs::Result<Vec<JsWorkspace>> { ... }
/// }
/// ```
///
/// Must come before `#[rquickjs::methods]`. Takes the same `#[ts(...)]`
/// overrides as `#[js_entity]`.
#[proc_macro_attribute]
pub fn js_global(args: TokenStream, item: TokenStream) -> TokenStream {
    let name = parse_macro_input!(args as LitStr);
    let item = parse_macro_input!(item as syn::ItemImpl);
    js_global_attribute::js_global_impl(name, item)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// The second half of `#[js_entity]`, called back by the model's fields
/// macro with the model's columns appended. Not for direct use.
#[doc(hidden)]
#[proc_macro]
pub fn __js_entity_expand(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as JsEntityExpand);
    js_entity_expand::js_entity_expand(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
