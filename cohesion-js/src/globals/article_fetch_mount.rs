//! The `ArticleFetch` mount: no global, only the mark that lets the
//! sandbox's scripts fetch an article's page.

use rquickjs::JsLifetime;

use crate::globals::Global;

/// Stored as context userdata in a sandbox that mounts `ArticleFetch`;
/// `Article.resnapshot` fetches only when it is there.
#[derive(JsLifetime)]
pub struct ArticleFetchMount;

impl Global for ArticleFetchMount {
    fn install<'a>(
        context: &rquickjs::Ctx<'a>,
        _database: cohesion_db::GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context
            .store_userdata(Self)
            .expect("no userdata is borrowed while a sandbox is built");
        Ok(())
    }
}
