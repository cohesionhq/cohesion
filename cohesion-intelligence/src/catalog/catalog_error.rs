use cohesion_db::CommandError;

#[derive(Debug)]
pub enum CatalogError {
    CannotDownload(String),
    CannotRead(String),
    ParseError(String),
    CommandError(CommandError),
}
