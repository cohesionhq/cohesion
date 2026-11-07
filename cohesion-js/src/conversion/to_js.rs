use cohesion_core::model::{ActorMode, ActorModel, MetadataKey, ResourceKind, VersionedItem};
use cohesion_core::{TaskAssignee, TaskSize};
use rquickjs::{Ctx, IntoJs};

/// A model column's value as a JS value; what the generated getters and
/// `toJSON` call. Every column type of a model with a `#[js_entity]` needs one.
pub trait ToJs {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>>;
}

to_js_via_into_js!(i64, f64, bool, String);

impl<T: ToJs> ToJs for Option<T> {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        match self {
            Some(value) => value.to_js(ctx),
            None => Ok(rquickjs::Value::new_null(ctx.clone())),
        }
    }
}

/// Its number, as on the wire.
impl ToJs for TaskSize {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        i64::from(self.get()).into_js(ctx)
    }
}

/// `{ kind: "user" }`, `{ kind: "agent", agentId }` or
/// `{ kind: "contact", contactId }`.
impl ToJs for TaskAssignee {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        let object = rquickjs::Object::new(ctx.clone())?;
        match self {
            Self::User => object.set("kind", "user")?,
            Self::Agent { agent_id } => {
                object.set("kind", "agent")?;
                object.set("agentId", *agent_id)?;
            }
            Self::Contact { contact_id } => {
                object.set("kind", "contact")?;
                object.set("contactId", *contact_id)?;
            }
        }
        Ok(object.into_value())
    }
}

impl ToJs for VersionedItem {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        let object = rquickjs::Object::new(ctx.clone())?;
        match self {
            Self::Task { task_id, status } => {
                object.set("kind", "task")?;
                object.set("taskId", *task_id)?;
                object.set("status", status.to_js(ctx)?)?;
            }
            Self::Document { document_id } => {
                object.set("kind", "document")?;
                object.set("documentId", *document_id)?;
            }
        }
        Ok(object.into_value())
    }
}

/// Plain objects, as on the wire.
impl ToJs for Vec<ResourceKind> {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        ctx.json_parse(serde_json::to_string(self).expect("resource kinds serialize"))
    }
}

/// Their texts, as on the wire.
impl ToJs for Vec<MetadataKey> {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        ctx.json_parse(serde_json::to_string(self).expect("metadata keys serialize"))
    }
}

/// Plain objects, as on the wire.
impl ToJs for Vec<ActorMode> {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        ctx.json_parse(serde_json::to_string(self).expect("actor modes serialize"))
    }
}

/// Plain objects, as on the wire.
impl ToJs for Vec<ActorModel> {
    fn to_js<'js>(&self, ctx: &Ctx<'js>) -> rquickjs::Result<rquickjs::Value<'js>> {
        ctx.json_parse(serde_json::to_string(self).expect("actor models serialize"))
    }
}
