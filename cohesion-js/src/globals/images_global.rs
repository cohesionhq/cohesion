use cohesion_core::protocol::ImageUpload;
use cohesion_db::{
    GuardedDatabaseContext, ListActorSessionMessageAttachmentsCommand, validate_images,
};
use cohesion_macros::{JsOptions, js_global};
use rquickjs::{Ctx, Exception, JsLifetime, class::Trace};

use super::agent_effect::AgentImages;
use super::global::Global;
use crate::traced_command::read;

/// An image for `images.attach()`.
#[derive(JsOptions)]
pub struct ImageOptions {
    /// `"image/png"`, `"image/jpeg"`, `"image/gif"` or `"image/webp"`.
    pub media_type: String,
    /// The image, base64-encoded: at most 5 MB once encoded. A
    /// `network.fetch` with `encoding: "base64"` returns a body in this
    /// form.
    pub data: String,
}

#[derive(Trace, JsLifetime)]
#[rquickjs::class]
pub struct ImagesGlobal {
    #[qjs(skip_trace)]
    database: GuardedDatabaseContext,
}
/// Images to show the user: attach them here, then send them with
/// `send_message`'s `images`.
///
/// Attaching takes effect once the script finishes, and only if it does not
/// throw. The script's result then numbers what it attached (`Attached as
/// image 3.`), and `send_message` takes those numbers. A script attaches at
/// most 10 images, and the latest 10 stay attached; send any image again
/// by its number.
#[js_global("images")]
#[rquickjs::methods]
impl ImagesGlobal {
    /// Attaches `image`, made or fetched by the script. Throws if it isn't a
    /// PNG, JPEG, GIF or WebP image, isn't base64, or is over 5 MB.
    pub fn attach<'js>(&self, ctx: Ctx<'js>, image: ImageOptions) -> rquickjs::Result<()> {
        attach(
            &ctx,
            vec![ImageUpload {
                media_type: image.media_type,
                data: image.data,
            }],
        )
    }

    /// Attaches every image that came with actor session message `messageId`
    /// (its `attachmentCount`, such as a screenshot the actor took), in
    /// the order they came. Throws if it has none.
    #[qjs(rename = "attachFromActorSession")]
    pub fn attach_from_actor_session<'js>(
        &self,
        ctx: Ctx<'js>,
        message_id: i64,
    ) -> rquickjs::Result<()> {
        let attachments = read(
            &ctx,
            &self.database,
            ListActorSessionMessageAttachmentsCommand { message_id },
        )?;
        if attachments.is_empty() {
            return Err(Exception::throw_reference(
                &ctx,
                &format!("Actor session message {message_id} has no images."),
            ));
        }
        attach(
            &ctx,
            attachments
                .into_iter()
                .map(|attachment| ImageUpload {
                    media_type: attachment.media_type,
                    data: attachment.data.to_base64(),
                })
                .collect(),
        )
    }
}

/// Adds `images` to the running agent job's, once they and those it already
/// attached pass a message's check; throws when they don't, or when no agent
/// job is collecting images.
fn attach(ctx: &Ctx<'_>, images: Vec<ImageUpload>) -> rquickjs::Result<()> {
    let attached = ctx.userdata::<AgentImages>().ok_or_else(|| {
        Exception::throw_internal(
            ctx,
            "Images can only be attached while an agent's turn or run is going.",
        )
    })?;
    let mut attached = attached.0.borrow_mut();
    let start = attached.len();
    attached.extend(images);
    if let Err(e) = validate_images(&attached) {
        attached.truncate(start);
        return Err(Exception::throw_type(ctx, &e.message));
    }
    Ok(())
}

impl Global for ImagesGlobal {
    fn install<'a>(
        context: &Ctx<'a>,
        database: GuardedDatabaseContext,
    ) -> Result<(), rquickjs::Error> {
        context.globals().set("images", ImagesGlobal { database })?;
        Ok(())
    }
}
