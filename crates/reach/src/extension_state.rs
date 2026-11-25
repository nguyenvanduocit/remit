//! The extensions as the owner sees them, and his word on one, his or the Assistant's for him (ADR-0087): disabled, it is
//! no source and its process ends; enabled, it is one again.

use std::fmt;
use std::sync::Arc;

use model::{Actor, DisabledExtension, ExtensionOrigin, ExtensionTool, ExtensionToolInput, Time};
use serde::Deserialize;
use serde::de::{Deserializer, MapAccess, Visitor};
use serde_json::Value;
use source::Extension;

use crate::Reach;
use chains::chain_core::gist;
use chains::owner_acts::{ACT_EXTENSION_STATE, for_owner};
use chains::request::Service;
use chains::request::{Done, Request};
use chains::{Error, Result};

/// A manifest the server holds, with when the owner applied it: none for a plugin that ships with Remit.
#[derive(Debug, Clone)]
pub(crate) struct InstalledExtension {
    pub ext: Extension,
    pub applied_at: Option<Time>,
}

/// What the owner reads of an installed extension: disabled is the owner's record of disabling it, if any.
fn extension_view(i: &InstalledExtension, disabled: Option<&DisabledExtension>) -> model::Extension {
    let e = &i.ext;
    let origin = if e.bundled {
        ExtensionOrigin::Bundled
    } else if e.mcp.is_some() {
        ExtensionOrigin::Mcp
    } else if e.auth.is_some() {
        ExtensionOrigin::Api
    } else {
        ExtensionOrigin::Program
    };
    model::Extension {
        id: e.id.clone(),
        label: e.label.clone(),
        capability: e.capability.clone(),
        sha256: if origin == ExtensionOrigin::Program {
            e.sha256.clone()
        } else {
            String::new()
        },
        origin,
        enabled: disabled.is_none(),
        disabled_by: disabled.map(|d| d.by.clone()),
        disabled_at: disabled.map(|d| d.at),
        tools: e
            .tools
            .iter()
            .map(|t| ExtensionTool {
                name: format!("{}_{}", e.id, t.name),
                what: t.description.clone(),
                effect: t.effect.as_str().to_owned(),
                inputs: tool_inputs(&t.input_schema),
            })
            .collect(),
        poll: e.poll,
        ui: e.ui.is_some(),
        applied_at: i.applied_at,
    }
}

/// One property of a tool's input schema, as the owner reads it.
#[derive(Deserialize, Default)]
#[serde(default)]
struct Property {
    r#type: Value,
    description: String,
    r#enum: Vec<Value>,
    default: Option<Value>,
    items: Option<Items>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Items {
    r#type: Value,
}

/// The properties of a schema in the order they are written (the order of the plugin's struct fields): serde_json's map
/// sorts its keys, so they are read as a list.
struct Ordered(Vec<(String, Property)>);

impl<'de> Deserialize<'de> for Ordered {
    fn deserialize<D: Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = Ordered;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("an object of properties")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> std::result::Result<Ordered, A::Error> {
                let mut out = Vec::new();
                while let Some((k, v)) = m.next_entry::<String, Property>()? {
                    out.push((k, v));
                }
                Ok(Ordered(out))
            }
        }
        d.deserialize_map(V)
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct SchemaDoc {
    properties: Option<Ordered>,
    required: Vec<String>,
}

/// The top-level inputs of a tool's input schema, in the order its properties are written. The manifest was checked to be
/// JSON when it was accepted; a schema that is not an object of properties has none.
fn tool_inputs(schema: &str) -> Vec<ExtensionToolInput> {
    let Ok(SchemaDoc {
        properties: Some(Ordered(props)),
        required,
    }) = serde_json::from_str::<SchemaDoc>(schema)
    else {
        return Vec::new();
    };
    props
        .into_iter()
        .map(|(name, p)| {
            let mut r#type = schema_type(&p.r#type);
            if r#type == "array"
                && let Some(item) = p
                    .items
                    .as_ref()
                    .map(|i| schema_type(&i.r#type))
                    .filter(|t| !t.is_empty())
            {
                r#type = format!("array of {item}");
            }
            ExtensionToolInput {
                required: required.contains(&name),
                name,
                r#type,
                what: p.description,
                choices: p.r#enum.iter().map(choice_text).collect(),
                default: p.default.map(|d| d.to_string()),
            }
        })
        .collect()
}

/// A JSON Schema type as written, one name or several ("string or null"); empty when it names none.
fn schema_type(t: &Value) -> String {
    match t {
        Value::String(s) => s.clone(),
        Value::Array(names) => names.iter().filter_map(Value::as_str).collect::<Vec<_>>().join(" or "),
        _ => String::new(),
    }
}

/// An enum value as the owner reads it: a string bare, anything else as JSON.
fn choice_text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

impl Reach {
    /// The extensions the server holds, disabled ones too: the plugins that ship with it, then the ones the owner applied,
    /// by id. A manifest that is not one any more is logged and left out.
    pub(crate) async fn installed_extensions(&self) -> Result<Vec<InstalledExtension>> {
        let stored = self.store.read(|r| r.extensions()).await?;
        let mut out: Vec<InstalledExtension> = self
            .bundled_extensions()
            .await
            .into_iter()
            .map(|ext| InstalledExtension { ext, applied_at: None })
            .collect();
        for e in stored {
            let ext: Extension = match serde_json::from_str(&e.doc) {
                Ok(ext) => ext,
                Err(err) => {
                    tracing::error!(id = e.id, %err, "an extension's manifest is not one");
                    continue;
                }
            };
            if let Err(err) = ext.validate() {
                tracing::error!(id = e.id, %err, "an extension's manifest is no longer accepted");
                continue;
            }
            out.push(InstalledExtension {
                ext,
                applied_at: Some(e.applied_at),
            });
        }
        Ok(out)
    }

    /// The plugin sources as the owner sees them: every one installed, disabled or not, the bundled ones first.
    pub async fn extensions(&self) -> Result<Vec<model::Extension>> {
        let installed = self.installed_extensions().await?;
        let disabled = self.store.read(|r| r.disabled_extensions()).await?;
        Ok(installed
            .iter()
            .map(|i| extension_view(i, disabled.iter().find(|d| d.id == i.ext.id)))
            .collect())
    }

    /// Tells the owner's screens how the extension id stands now, or that it is gone.
    pub(crate) async fn publish_extension(&self, id: &str) {
        match self.extensions().await {
            Ok(views) => match views.iter().find(|v| v.id == id) {
                Some(v) => self.notify.extension_updated(v),
                None => self.notify.extension_deleted(id),
            },
            Err(err) => tracing::error!(id, %err, "an extension's standing was not read"),
        }
    }

    /// The owner's word on one extension, his or the Assistant's for him: disabled, it is no source and its process ends;
    /// enabled, it is one again.
    pub async fn set_extension_disabled(self: &Arc<Self>, id: &str, disabled: bool, by: Actor) -> Result<()> {
        for_owner(ACT_EXTENSION_STATE, &by)?;
        let installed = self.installed_extensions().await?;
        if !installed.iter().any(|i| i.ext.id == id) {
            let ids: Vec<&str> = installed.iter().map(|i| i.ext.id.as_str()).collect();
            return Err(Error::refused(format!(
                "no extension {:?} is installed: the ones that are {}",
                gist(id, 40),
                ids.join(", ")
            )));
        }
        let (key, at) = (id.to_owned(), self.now());
        self.store
            .update(move |tx| -> Result<()> {
                if disabled {
                    tx.put_disabled_extension(&DisabledExtension { id: key, by, at })?;
                } else {
                    tx.delete_disabled_extension(&key)?;
                }
                Ok(())
            })
            .await?;
        if disabled {
            self.plugins.stop(id);
        }
        self.load_extensions().await;
        self.publish_extension(id).await;
        Ok(())
    }

    /// The owner taking an extension out of the sources until he enables it.
    pub async fn disable_extension(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "DisableExtension", move |o| async move {
            o.set_extension_disabled(&id, true, by).await
        })
        .await
    }

    /// The owner offering a disabled extension to agents again.
    pub async fn enable_extension(self: &Arc<Self>, req: Request, id: String) -> Result<Done> {
        let by = req.by.clone();
        self.act_done(req, "EnableExtension", move |o| async move {
            o.set_extension_disabled(&id, false, by).await
        })
        .await
    }
}
