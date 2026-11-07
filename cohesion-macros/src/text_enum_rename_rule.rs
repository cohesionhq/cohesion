//! The cases `#[text_enum(rename_all = "...")]` can spell a variant in.

use crate::identifier_case::camel_case;

/// How `#[text_enum(rename_all = "...")]` spells a PascalCase variant name,
/// using serde's names for the cases.
#[derive(Clone, Copy)]
pub(crate) enum RenameRule {
    Lower,
    Upper,
    Camel,
    Snake,
    Kebab,
    LowerSpace,
    /// Each variant's text comes from its `#[text_enum("...")]`.
    Custom,
}
impl RenameRule {
    pub(crate) const NAMES: &'static str = "\"lowercase\", \"UPPERCASE\", \"camelCase\", \"snake_case\", \
        \"kebab-case\", \"lower space case\", \"custom\"";

    pub(crate) fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "lowercase" => Self::Lower,
            "UPPERCASE" => Self::Upper,
            "camelCase" => Self::Camel,
            "snake_case" => Self::Snake,
            "kebab-case" => Self::Kebab,
            "lower space case" => Self::LowerSpace,
            "custom" => Self::Custom,
            _ => return None,
        })
    }

    pub(crate) fn apply(self, variant: &str) -> String {
        match self {
            Self::Lower => variant.to_lowercase(),
            Self::Upper => variant.to_uppercase(),
            Self::Camel => camel_case(variant),
            Self::Snake => separated(variant, '_'),
            Self::Kebab => separated(variant, '-'),
            Self::LowerSpace => separated(variant, ' '),
            Self::Custom => unreachable!("custom text comes from each variant's attribute"),
        }
    }
}

/// Splits a PascalCase name at each capital and lowercases it, so
/// `ChatCompletions` with `-` is `chat-completions`.
fn separated(ident: &str, separator: char) -> String {
    let mut out = String::with_capacity(ident.len() + 4);
    for (i, c) in ident.chars().enumerate() {
        if c.is_uppercase() {
            if i > 0 {
                out.push(separator);
            }
            out.extend(c.to_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}
