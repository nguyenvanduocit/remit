//! The tools that act on a conversation. Each is made as the teammate of INTERCOM_ADMIN_ID, because Intercom records who did
//! it. What each does to the world is its effect in the plugin's declaration, which the chain's grant is checked against before
//! any of them runs.

use plugin_sdk::Error;
use reqwest::Method;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Map, Value, json};
use source::{parse_when, rfc3339, shorten, table};

use super::{Intercom, SAFE_ID, chars, id, null_as_zero, text};

/// The longest note or reply: Intercom takes more, a message that long is not one a customer reads.
const BODY_MAX: usize = 5000;
/// How many tags one read of the workspace's tags lists.
const TAGS_MAX: usize = 200;

impl Intercom {
    /// The teammate the writes are made as, or how to set one.
    fn admin(&self) -> Result<&str, String> {
        if self.admin_id.is_empty() {
            return Err("this connection has no INTERCOM_ADMIN_ID: it is the id of the teammate the writes are made as (Intercom: Settings, Teammates); the owner enters it with the token".to_owned());
        }
        if !SAFE_ID.is_match(&self.admin_id) {
            return Err(format!(
                "INTERCOM_ADMIN_ID {:?} is not an Intercom id",
                shorten(&self.admin_id, 40)
            ));
        }
        Ok(&self.admin_id)
    }
}

/// The path of the conversation `id` with `sub` appended, or why `id` is not one.
fn conversation_path(id: &str, sub: &str) -> Result<String, String> {
    if !SAFE_ID.is_match(id) {
        return Err(format!(
            "id {:?} is not a conversation id: intercom_conversations lists them",
            shorten(id, 40)
        ));
    }
    Ok(format!("/conversations/{}{sub}", urlescape::path_segment(id)))
}

/// `s` trimmed, or why it is not words Intercom is sent as `field`.
fn words(field: &str, s: &str) -> Result<String, String> {
    let s = s.trim();
    match chars(s) {
        0 => Err(format!("{field} is empty")),
        n if n > BODY_MAX => Err(format!("{field} is {n} characters: at most {BODY_MAX}")),
        _ => Ok(s.to_owned()),
    }
}

// ---- tags ----

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct Tag {
    #[serde(deserialize_with = "id")]
    id: String,
    #[serde(deserialize_with = "null_as_zero")]
    name: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
struct TagList {
    #[serde(deserialize_with = "null_as_zero")]
    data: Vec<Tag>,
}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct NoInput {}

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct TagInput {
    /// the conversation id, from intercom_conversations
    id: String,
    /// the name of an existing tag, from intercom_tags
    tag: String,
}

impl Intercom {
    async fn list_tags(&self) -> Result<Vec<Tag>, Error> {
        let raw = self.api().get("/tags", &[]).await?;
        let res: TagList = serde_json::from_slice(&raw).map_err(|e| format!("intercom: the tags are not JSON: {e}"))?;
        Ok(res.data)
    }

    pub(super) async fn tags(self, _: NoInput) -> Result<String, Error> {
        let all = self.list_tags().await?;
        let rows: Vec<Vec<String>> = all.iter().map(|t| vec![t.id.clone(), text(&t.name, 60)]).collect();
        Ok(format!(
            "tags: {}\n{}",
            all.len(),
            table(&["id".to_owned(), "name".to_owned()], &rows, TAGS_MAX)
        ))
    }

    /// The tag called `name`, any case, or the names there are.
    async fn tag_named(&self, name: &str) -> Result<Tag, Error> {
        let name = name.trim();
        if name.is_empty() {
            return Err("tag is empty: give the name of a tag, intercom_tags lists them".into());
        }
        let all = self.list_tags().await?;
        let names: Vec<String> = all.iter().map(|t| t.name.clone()).collect();
        all.into_iter()
            .find(|t| t.name.to_lowercase() == name.to_lowercase())
            .ok_or_else(|| {
                format!(
                    "no tag is called {:?}: the tags are {}",
                    shorten(name, 60),
                    text(&names.join(", "), 400)
                )
                .into()
            })
    }

    pub(super) async fn tag(self, input: TagInput) -> Result<String, Error> {
        let admin = self.admin()?.to_owned();
        let path = conversation_path(&input.id, "/tags")?;
        let t = self.tag_named(&input.tag).await?;
        self.api()
            .send(Method::POST, &path, Some(&json!({"id": t.id, "admin_id": admin})))
            .await?;
        Ok(format!("conversation {} is tagged {:?}", input.id, text(&t.name, 60)))
    }

    pub(super) async fn untag(self, input: TagInput) -> Result<String, Error> {
        let admin = self.admin()?.to_owned();
        conversation_path(&input.id, "")?;
        let t = self.tag_named(&input.tag).await?;
        let path = conversation_path(&input.id, &format!("/tags/{}", urlescape::path_segment(&t.id)))?;
        self.api()
            .send(Method::DELETE, &path, Some(&json!({"admin_id": admin})))
            .await?;
        Ok(format!(
            "conversation {} no longer has the tag {:?}",
            input.id,
            text(&t.name, 60)
        ))
    }
}

// ---- words in the conversation ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct WordsInput {
    /// the conversation id, from intercom_conversations
    id: String,
    /// the text, plain, at most 5000 characters
    body: String,
}

impl Intercom {
    /// Adds the body of `input` to its conversation as `kind`: a note for the teammates or a comment the customer receives.
    async fn say(&self, input: &WordsInput, kind: &str) -> Result<(), Error> {
        let admin = self.admin()?;
        let path = conversation_path(&input.id, "/reply")?;
        let body = words("body", &input.body)?;
        let sent = json!({"message_type": kind, "type": "admin", "admin_id": admin, "body": body});
        self.api().send(Method::POST, &path, Some(&sent)).await?;
        Ok(())
    }

    pub(super) async fn note(self, input: WordsInput) -> Result<String, Error> {
        self.say(&input, "note").await?;
        Ok(format!(
            "a note is added to conversation {}: teammates see it, the customer does not",
            input.id
        ))
    }

    pub(super) async fn reply(self, input: WordsInput) -> Result<String, Error> {
        self.say(&input, "comment").await?;
        Ok(format!("the customer in conversation {} is sent your reply", input.id))
    }
}

// ---- state ----

#[derive(Debug, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct StateInput {
    /// the conversation id, from intercom_conversations
    id: String,
    /// closed, open or snoozed
    state: String,
    /// when a snoozed conversation reopens: RFC3339 or relative to now (+2d); required for snoozed
    #[serde(default)]
    snoozed_until: String,
    /// for closed: a message the customer sees, at most 5000 characters
    #[serde(default)]
    message: String,
}

impl Intercom {
    pub(super) async fn state(self, input: StateInput) -> Result<String, Error> {
        let admin = self.admin()?;
        let path = conversation_path(&input.id, "/parts")?;
        let mut body = Map::from_iter([("admin_id".to_owned(), Value::from(admin))]);
        let has_message = !input.message.trim().is_empty();
        let mut until = None;
        match input.state.as_str() {
            "closed" => {
                body.insert("message_type".to_owned(), "close".into());
                body.insert("type".to_owned(), "admin".into());
                if has_message {
                    body.insert("body".to_owned(), words("message", &input.message)?.into());
                }
            }
            "open" => {
                body.insert("message_type".to_owned(), "open".into());
            }
            "snoozed" => {
                let at = parse_when(&input.snoozed_until, self.now).map_err(|e| format!("snoozed_until: {e}"))?;
                if at <= self.now {
                    return Err("snoozed_until is not in the future".into());
                }
                body.insert("message_type".to_owned(), "snoozed".into());
                body.insert("snoozed_until".to_owned(), at.as_second().into());
                until = Some(at);
            }
            other => return Err(format!("state {:?} is not closed, open or snoozed", shorten(other, 20)).into()),
        }
        if input.state != "closed" && has_message {
            return Err("message is for closing a conversation".into());
        }
        self.api().send(Method::POST, &path, Some(&Value::Object(body))).await?;
        Ok(match until {
            Some(at) => format!("conversation {} is snoozed until {}", input.id, rfc3339(at)),
            None => format!("conversation {} is {}", input.id, input.state),
        })
    }
}
