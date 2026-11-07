//! TypeScript declarations of what scripts can see, so editors can offer
//! completion and type checking for the script API.

/// A script-facing type's TypeScript declaration. Implemented by
/// `#[derive(TextEnum)]` (a string union type), `#[js_entity]` (a global
/// class), `#[js_global]` (a namespace of functions) and
/// `#[derive(JsOptions)]` (an interface for an options argument).
pub trait TsDocumentation {
    /// Declaration source, valid on its own at the top level of a `.d.ts`
    /// file; namespaces merge, so declarations can be joined in any order.
    const TS: &'static str;
    /// `TS` with only the first paragraph of each doc comment: what each
    /// item is, without the detail.
    const TS_BRIEF: &'static str;
}
