//! The models.dev catalog: [`ModelCatalog`] downloads it and stores its models
//! and prices as `CatalogModel` rows.

mod catalog_error;
mod model_catalog;
mod models_dev_schema;

pub use model_catalog::ModelCatalog;
