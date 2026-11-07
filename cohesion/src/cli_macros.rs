//! The CLI's `macro_rules!` macros.

/// Builds a `Row` from `Display` cells; prefix a cell with `>` to right-align it, as for
/// ids and numbers: `row![marker, >w.id, w.title]`.
macro_rules! row {
    (@cells [$($cell:expr),*]) => {
        $crate::cli::table::Row::new(vec![$($cell),*])
    };
    (@cells [$($cell:expr),*] > $next:expr $(, $($rest:tt)*)?) => {
        row!(@cells [$($cell,)* $crate::cli::table::Cell::Right($next.to_string())] $($($rest)*)?)
    };
    (@cells [$($cell:expr),*] $next:expr $(, $($rest:tt)*)?) => {
        row!(@cells [$($cell,)* $crate::cli::table::Cell::Left($next.to_string())] $($($rest)*)?)
    };
    ($($cells:tt)*) => {
        row!(@cells [] $($cells)*)
    };
}
pub(crate) use row;
