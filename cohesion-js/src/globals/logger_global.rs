use cohesion_core::log::Module;
use cohesion_core::log_info;
use cohesion_db::GuardedDatabaseContext;
use rquickjs::{Ctx, JsLifetime, class::Trace};

use cohesion_macros::js_global;

use super::global::Global;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct LoggerGlobal {}
#[js_global("log")]
#[rquickjs::methods]
impl LoggerGlobal {
    pub fn info(&self, line: String) -> rquickjs::Result<()> {
        log_info!(Module::Script, "log.info: {}", line);
        Ok(())
    }
}
impl Global for LoggerGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        _database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context.globals().set("log", LoggerGlobal {})?;
        Ok(())
    }
}
