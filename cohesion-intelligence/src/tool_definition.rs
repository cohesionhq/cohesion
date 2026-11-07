use std::borrow::Cow;

#[derive(Debug)]
pub struct Tool {
    /// The name the model calls the tool by, sent to the provider as-is.
    pub name: &'static str,
    /// Fixed for most tools; a code tool's adds what it loaded at session
    /// start.
    pub description: Cow<'static, str>,
    pub fields: Vec<Field>,
}
#[derive(Debug)]
pub struct Field {
    pub name: &'static str,
    pub description: &'static str,
    pub kind: FieldKind,
    pub required: bool,
}
#[derive(Debug)]
pub enum FieldKind {
    String,
    Integer,
    /// An array of strings.
    StringList,
    /// An array of integers.
    IntegerList,
    Boolean,
}
