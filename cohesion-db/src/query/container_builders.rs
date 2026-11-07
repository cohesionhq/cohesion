//! The methods that start each query builder: reads on the
//! [`ReadableDatabase`] trait both views implement, writes on
//! [`WriteContainer`] only.

use cohesion_core::value::FromValue;
use cohesion_core::{Model, Projection, SoftDelete};

use crate::container::{Container, ReadContainer, WriteContainer};
use crate::query::clauses::Filtered;
use crate::query::delete::Delete;
use crate::query::insert::Insert;
use crate::query::select::Select;
use crate::query::update::Update;

/// Reads over a borrowed connection: what both [`ReadContainer`] and
/// [`WriteContainer`] offer.
pub trait ReadableDatabase<'c>: Copy {
    /// The connection behind the view.
    fn container(self) -> &'c Container;

    /// Starts a `SELECT` of every column of `M`.
    fn select<M: Model>(self) -> Select<'c, M> {
        Select::new(self.container())
    }

    /// Starts a `SELECT` of just the `id` of `M` rows.
    fn select_id<M: Model>(self) -> Select<'c, M, i64> {
        Select::column(self.container(), "id")
    }

    /// Starts a `SELECT` of one `column` of `M` rows, each read as a `T`:
    /// a list of values, or a subquery for `filter_in`.
    fn select_column<M: Model, T: FromValue>(self, column: &'static str) -> Select<'c, M, T> {
        Select::column(self.container(), column)
    }

    /// Starts a `SELECT` of the rows of `M` that have not been soft-deleted.
    fn select_live<M: SoftDelete>(self) -> Select<'c, M> {
        self.select::<M>().live()
    }

    /// Starts a `SELECT` of `P`'s columns of its model's rows, with `P`'s joins.
    fn select_as<P: Projection>(self) -> Select<'c, P::Of, P> {
        Select::projected(self.container())
    }

    /// Starts a `SELECT` of `P`'s columns of the rows of its model that have
    /// not been soft-deleted.
    fn select_live_as<P: Projection>(self) -> Select<'c, P::Of, P>
    where
        P::Of: SoftDelete,
    {
        self.select_as::<P>().live()
    }
}

impl<'c> ReadableDatabase<'c> for ReadContainer<'c> {
    fn container(self) -> &'c Container {
        self.container
    }
}

impl<'c> ReadableDatabase<'c> for WriteContainer<'c> {
    fn container(self) -> &'c Container {
        self.container
    }
}

impl<'c> WriteContainer<'c> {
    /// Starts an `INSERT` of one `M` row.
    pub fn insert<M: Model>(self) -> Insert<'c, M> {
        Insert::new(self.container)
    }

    /// Starts an `UPDATE` of `M` rows.
    pub fn update<M: Model>(self) -> Update<'c, M> {
        Update::new(self.container)
    }

    /// Starts an `UPDATE` of the rows of `M` that have not been soft-deleted.
    pub fn update_live<M: SoftDelete>(self) -> Update<'c, M> {
        self.update::<M>().live()
    }

    /// Starts a hard `DELETE` of `M` rows; soft deletes are an [`Update`].
    pub fn delete<M: Model>(self) -> Delete<'c, M> {
        Delete::new(self.container)
    }
}
