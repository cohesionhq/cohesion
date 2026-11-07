use cohesion_core::DayFilter;
use cohesion_core::model::ModelUsage;
use cohesion_db::{GuardedDatabaseContext, ListModelUsageCommand};
use rquickjs::function::Opt;
use rquickjs::{Ctx, JsLifetime, class::Trace};

use cohesion_macros::{JsOptions, js_global};

use super::global::Global;
use crate::entities::usage::{JsModelUsage, JsUsage};
use crate::traced_command::read;

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct UsageGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}

/// How many tokens Cohesion's models used and what they cost, across every
/// conversation, agent and script. An extension needs the `usage`
/// permission.
///
/// ```js
/// const today = usage.total({ days: "2026-10-20" });
/// `${today.calls} calls, $${today.cost.toFixed(2)}`;
/// ```
#[js_global("usage")]
#[rquickjs::methods]
impl UsageGlobal {
    /// What every model call cost together: calls, tokens by kind, and
    /// `cost` in US dollars.
    pub fn total<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<UsageOptions>,
    ) -> rquickjs::Result<JsUsage> {
        let models = self.models(&ctx, options)?;
        Ok(JsUsage::new(models.iter().collect(), self.database.clone()))
    }

    /// What each model's calls cost together: one entry per provider and
    /// model, costliest first.
    #[qjs(rename = "byModel")]
    pub fn by_model<'js>(
        &self,
        ctx: Ctx<'js>,
        options: Opt<UsageOptions>,
    ) -> rquickjs::Result<Vec<JsModelUsage>> {
        Ok(self
            .models(&ctx, options)?
            .into_iter()
            .map(|model| JsModelUsage::new(model, self.database.clone()))
            .collect())
    }
}

impl UsageGlobal {
    fn models(
        &self,
        ctx: &Ctx<'_>,
        options: Opt<UsageOptions>,
    ) -> rquickjs::Result<Vec<ModelUsage>> {
        let days = options.0.and_then(|options| options.days);
        read(ctx, &self.database, ListModelUsageCommand { days })
    }
}

/// Options for `usage.total()` and `usage.byModel()`.
#[derive(JsOptions)]
pub struct UsageOptions {
    /// Only calls made on this local day, any of these days, or between
    /// these bounds, as task dates are matched
    /// (`{ days: { after: "2026-10-01" } }`); every day when left out.
    pub days: Option<DayFilter>,
}

impl Global for UsageGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context.globals().set("usage", UsageGlobal { database })?;
        Ok(())
    }
}
