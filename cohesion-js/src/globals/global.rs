use cohesion_db::GuardedDatabaseContext;
use rquickjs::Ctx;

pub trait Global {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error>;
}
