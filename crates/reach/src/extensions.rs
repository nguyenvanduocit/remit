//! Extensions (docs/remit-design.md, K5; ADR-0034): a source nobody shipped, written as a plugin (a program that serves the
//! plugin contract) by a chain or a customer and applied by the owner.
//!
//! The Chair of a chain that holds the extensions capability proposes one with propose_extension: the path of the
//! built program on this server. The platform copies it to the staging directory, starts it once to hear what it says it is
//! (its label, capability, credential variables, tools, events and frontend), and raises a change item that shows the owner
//! all of it with the program's hash. Nothing exists until it is applied in his inbox: Apply installs the program under
//! the extensions directory under a name that holds its hash, stores the manifest pinned to that hash, and the source exists
//! for sessions started after. The program then starts only with that hash (`plugin::RemotePlugin`).

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use model::{
    Card, ChangeKind, ChangedField, InboxItem, InboxKind, InboxOption, InboxStatus, PendingChange, StoredExtension,
};
use regex::Regex;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use source::{
    Extension, ExtensionAuth, ExtensionProcessor, ExtensionSensorKind, ExtensionSlot, ExtensionSubscription,
    ExtensionTool, ExtensionUi, Var,
};
use store::Tx;
use wire::plugin::Manifest;

use crate::Reach;
use crate::mcp_servers::McpImport;
use chains::chain_core::gist;
use chains::chain_core::new_id;
use chains::change_item::{Applied, CHANGE_APPLY, CHANGE_NOT_NOW, change_detail, code_span};
use chains::changes::{Authorship, doc_of, record_change};
use chains::owner_acts::{ACT_APPLY, for_owner};
use chains::tools::Tools;
use chains::{Error, Result};

/// Bytes of a program a chain may propose: a build of a plugin.
const MAX_EXTENSION_PROGRAM: u64 = 256 << 20;
const EXTENSION_STAGING: &str = ".staging";
/// A proposal the owner has not answered by then is proposed again.
const STAGING_KEEPS: Duration = Duration::from_secs(14 * 24 * 60 * 60);
/// What the id a program started only to describe itself is known by begins with: an underscore, which no extension's id
/// holds, so a describe never takes the process of an applied extension, nor another describe's.
const DESCRIBE_AS: &str = "describe_";

static EXTENSION_FILE: LazyLock<Regex> = LazyLock::new(|| {
    #[expect(clippy::expect_used, reason = "a literal pattern")]
    Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$").expect("valid pattern")
});

/// What a Chair writes to propose an extension: the built program, which says the rest itself, or, for an API behind the
/// proxy, its manifest. The program's path in the extensions directory and its hash are not the Chair's to write: the
/// platform names the file by what it holds.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct ProposeExtensionInput {
    /// the source's id: a lower case word of 2 to 24 letters and digits, which connections and tool names are prefixed by
    pub id: String,
    /// the absolute path, on this server, of the built plugin: a program that serves the plugin contract. It says its
    /// label, capability, credential variables, tools, events and frontend itself, so none of them is written here
    #[serde(default)]
    pub binary: String,
    /// for an API behind the proxy only: the name the owner reads, e.g. VNPay
    #[serde(default)]
    pub label: String,
    /// for an API behind the proxy only: which capability of the catalog gives a chain this source: data, observe or
    /// customers
    #[serde(default)]
    pub capability: String,
    /// for an API behind the proxy only: the variables of its credential, each {name, what, secret, optional}: the owner
    /// types the values into his inbox
    #[serde(default)]
    #[schemars(with = "Vec<serde_json::Map<String, Value>>")]
    pub vars: Vec<Var>,
    /// instead of a program: an API the platform reaches for scripts agents write, adding the credential itself under one
    /// scheme: {base: the https address, scheme: bearer (one variable), basic (a user variable and a password variable),
    /// header (one variable and header: its name), hmac-query (one secret, param: the query parameter the hash goes in,
    /// hash: sha256 or sha512) or sigv4 (the key id, the secret key, the region and the service, in that order), vars: the
    /// variables it reads, each declared in vars, readPosts: paths where POST only reads}
    #[serde(default)]
    #[schemars(with = "Option<serde_json::Map<String, Value>>")]
    pub auth: Option<ExtensionAuth>,
}

/// The removal of an extension the owner applied.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct RemoveExtensionInput {
    /// the id of the extension to remove, as list_sources names it
    pub id: String,
    /// why it goes, in a line the owner reads
    pub why: String,
}

/// What a change item holds: the manifest, and where the program the owner reads is staged until he applies it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct ExtensionProposal {
    pub manifest: Extension,
    /// The program as the platform copied it, and its size in bytes; empty for an API behind the proxy.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub staged: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub size: u64,
    /// Makes it the removal of the extension the manifest names (as the owner applied it); `why` is what the Chair said.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub remove: bool,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub why: String,
    /// For an MCP server brought in from this machine's configuration: where its values are found again on Apply.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub import: Option<McpImport>,
}

fn is_zero(n: &u64) -> bool {
    *n == 0
}

/// What is left to do once an extension's apply committed: the program, the sources and the owner's screens follow it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AfterApply {
    /// Extension id was applied from the staged program, if any, and its values found in this machine's configuration.
    Applied {
        id: String,
        staged: String,
        import: Option<McpImport>,
    },
    /// Extension id was removed.
    Removed { id: String },
}

/// The tell that does what follows an apply once its transaction committed, off the caller's path, on o.
fn after(o: Option<Arc<Reach>>, a: AfterApply) -> Applied {
    Applied::Written(Some(Box::new(move |_: &chains::Core| {
        if let Some(o) = o {
            tokio::spawn(async move { o.after_extension(a).await });
        }
    })))
}

/// The manifest a plugin's description fills: everything but where its program is.
pub(crate) fn manifest_of(id: &str, d: &Manifest) -> Extension {
    Extension {
        id: id.to_owned(),
        label: d.label.clone(),
        capability: d.capability.clone(),
        poll: d.poll,
        accounts: d.accounts,
        vars: d
            .vars
            .iter()
            .map(|v| Var {
                name: v.name.clone(),
                what: v.what.clone(),
                secret: v.secret,
                optional: v.optional,
                address: v.address,
            })
            .collect(),
        tools: d
            .tools
            .iter()
            .map(|t| ExtensionTool {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: json_text(&t.input_schema.0),
                effect: t.effect,
                mcp_name: String::new(), // a plugin's tool, not an MCP server's
            })
            .collect(),
        sensor_kinds: d
            .sensor_kinds
            .iter()
            .map(|k| ExtensionSensorKind {
                name: k.name.clone(),
                label: k.label.clone(),
                settings_schema: json_text(&k.settings_schema.0),
            })
            .collect(),
        processors: d
            .processors
            .iter()
            .map(|p| ExtensionProcessor {
                name: p.name.clone(),
                label: p.label.clone(),
                params_schema: json_text(&p.params_schema.0),
                effect: p.effect,
            })
            .collect(),
        subscriptions: d
            .subscriptions
            .iter()
            .map(|s| ExtensionSubscription {
                name: s.name.clone(),
                note: s.note.clone(),
                pattern: s.r#match.clone(),
                expr: s.expr.clone(),
                processor: s.processor.clone(),
                params: s.params.as_ref().map(|p| json_text(&p.0)).unwrap_or_default(),
                action: s.action.clone(),
                job: s.job.clone(),
                with: s.with.clone(),
                paused: s.paused,
            })
            .collect(),
        auth: d.auth.as_ref().map(|a| ExtensionAuth {
            base: a.base.clone(),
            scheme: a.scheme.clone(),
            header: a.header.clone(),
            param: a.param.clone(),
            hash: a.hash.clone(),
            vars: a.vars.clone(),
            read_posts: a.read_posts.clone(),
            base_var: a.base_var.clone(),
            headers: a.headers.clone(),
        }),
        ui: d.ui.as_ref().map(|ui| ExtensionUi {
            entry: ui.entry.clone(),
            slots: ui
                .slots
                .iter()
                .map(|s| ExtensionSlot {
                    slot: s.slot.clone(),
                    id: s.id.clone(),
                    title: s.title.clone(),
                    expose: s.expose.clone(),
                })
                .collect(),
        }),
        ..Extension::default()
    }
}

/// A JSON document as the manifest keeps it: its text, and nothing for none.
fn json_text(v: &Value) -> String {
    if v.is_null() { String::new() } else { v.to_string() }
}

/// The manifest of a program from what it described: the file is named by the hash it holds. The id and the file's name are
/// the Chair's; everything else is the program's own.
pub(crate) fn extension_of(id: &str, file: &str, sum: &str, d: &Manifest) -> Result<Extension> {
    if !d.signals.is_empty() || !d.agents.is_empty() || !d.subscriptions.is_empty() {
        return Err(Error::refused(
            "it ships event signals, agent templates or subscriptions, which only a plugin that ships with Remit may (ADR-0037): a sensor reads a credential for ever, a template becomes an agent when hired and a subscription acts without being asked",
        ));
    }
    let file = if EXTENSION_FILE.is_match(file) && file != "." && file != ".." {
        file
    } else {
        "plugin"
    };
    let mut ext = manifest_of(id, d);
    ext.program = format!("{id}/{}-{file}", sum.get(..12).unwrap_or(sum));
    ext.sha256 = sum.to_owned();
    ext.validate().map_err(|e| Error::refused(e.to_string()))?;
    Ok(ext)
}

/// The manifest of an API behind the proxy from what the Chair wrote.
fn api_extension_of(input: &ProposeExtensionInput) -> Result<Extension> {
    if !input.binary.is_empty() {
        return Err(Error::refused(
            "an extension with auth is an API behind the proxy: give no binary with it",
        ));
    }
    let ext = Extension {
        id: input.id.clone(),
        label: input.label.clone(),
        capability: input.capability.clone(),
        vars: input.vars.clone(),
        auth: input.auth.clone(),
        ..Extension::default()
    };
    ext.validate().map_err(|e| Error::refused(e.to_string()))?;
    Ok(ext)
}

/// The manifest the owner applied under id, none when there is none.
pub(crate) fn stored_extension(all: &[StoredExtension], id: &str) -> Result<Option<Extension>> {
    let Some(e) = all.iter().find(|e| e.id == id) else {
        return Ok(None);
    };
    serde_json::from_str(&e.doc)
        .map(Some)
        .map_err(|err| Error::refused(format!("the manifest stored for {id}: {err}")))
}

/// The fields of an extension's write that move a rail, each as it is and as it would be: all of them, since each is a
/// standing use of a credential. A write that changes none of them is no change. size is the program's size in bytes.
pub(crate) fn extension_rail(cur: Option<&Extension>, next: &Extension, size: u64) -> Vec<ChangedField> {
    fn program(e: &Extension, size: Option<u64>) -> String {
        if e.program.is_empty() {
            return String::new();
        }
        let file = Path::new(&e.program)
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        match size {
            // the size of the program applied before is not kept
            None => format!("sha256 {}, file {file}", e.sha256),
            Some(size) => format!("sha256 {}, {size} bytes, file {file}", e.sha256),
        }
    }
    /// A value as compact JSON, and nothing for one that is empty (no list, no object, false).
    fn compact<T: Serialize>(v: &T) -> String {
        match serde_json::to_string(v).unwrap_or_default().as_str() {
            "null" | "[]" | "{}" | "false" | "" => String::new(),
            s => s.to_owned(),
        }
    }
    let field = |name: &str, before: String, after: String| -> Option<ChangedField> {
        // nothing was there to be changed from
        let before = if cur.is_none() { String::new() } else { before };
        (before != after).then(|| ChangedField {
            name: name.to_owned(),
            before,
            after,
        })
    };
    let before = cur.cloned().unwrap_or_default();
    let old_program = cur.map(|c| program(c, None)).unwrap_or_default();
    let mut out: Vec<ChangedField> = Vec::new();
    if cur.is_none() {
        out.extend(field("label", String::new(), next.label.clone()));
    }
    out.extend(
        [
            field("capability", before.capability.clone(), next.capability.clone()),
            field("program", old_program, program(next, Some(size))),
            field("tools", tool_lines(&before.tools), tool_lines(&next.tools)),
            field("vars", compact(&before.vars), compact(&next.vars)),
            field("poll", compact(&before.poll), compact(&next.poll)),
            field("accounts", accounts_line(before.accounts), accounts_line(next.accounts)),
            field("ui", ui_lines(before.ui.as_ref()), ui_lines(next.ui.as_ref())),
            field("auth", compact(&before.auth), compact(&next.auth)),
            field("mcp", compact(&before.mcp), compact(&next.mcp)),
        ]
        .into_iter()
        .flatten(),
    );
    out
}

/// What the owner reads of a plugin's tools: one line each, with the grant its effect needs, and what the plugin says it
/// does.
fn tool_lines(tools: &[ExtensionTool]) -> String {
    tools
        .iter()
        .map(|t| format!("{} ({}): {}", t.name, t.effect, gist(&t.description, 200)))
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the owner reads of a plugin that provides Claude accounts (ADR-0071): how many it adds, and that the plugin decides
/// where their sessions' requests go. Nothing for a plugin that provides none.
fn accounts_line(provides: bool) -> String {
    if !provides {
        return String::new();
    }
    "provides Claude accounts, one for each connection: agents on them send their work wherever its variables point"
        .to_owned()
}

/// What the owner reads of a plugin's frontend: its entry and each slot it fills.
fn ui_lines(ui: Option<&ExtensionUi>) -> String {
    let Some(ui) = ui else { return String::new() };
    let mut lines = vec![format!("entry {}", ui.entry)];
    lines.extend(
        ui.slots
            .iter()
            .map(|s| format!("{} {}: {} ({})", s.slot, s.id, s.title, s.expose)),
    );
    lines.join("\n")
}

/// The field of an extension's removal that moves a rail: that the source, as applied, goes.
fn removal_rail(cur: &Extension) -> Vec<ChangedField> {
    vec![ChangedField {
        name: "extension".to_owned(),
        before: format!("{} ({}), program sha256 {}", cur.id, cur.label, cur.sha256),
        after: "removed".to_owned(),
    }]
}

/// The ids of the extensions applied, for an error.
fn extension_ids(all: &[StoredExtension]) -> String {
    if all.is_empty() {
        return "none".to_owned();
    }
    all.iter().map(|e| e.id.as_str()).collect::<Vec<_>>().join(", ")
}

/// What the owner reads under the item: who asks, what the source can do, and the fields that move a rail.
fn extension_detail(by: &str, ext: &Extension, fields: &[ChangedField]) -> String {
    let head = match &ext.auth {
        Some(a) => format!(
            "{by} asks to add the source {} ({}): an API at {} that agents reach with scripts they write. The platform adds the credential you give it under the scheme below, so no script holds it; a script reaches that one host and nothing else.\n",
            code_span(&ext.id),
            ext.label,
            code_span(&a.base)
        ),
        None => format!(
            "{by} asks to add the plugin {} ({}). It is a program that runs on this server as its user, with the credential you give it, and says below what it is; agents call its tools by the grant each tool's effect needs.\n",
            code_span(&ext.id),
            ext.label
        ),
    };
    format!("{head}\n{}", change_detail(by, &ChangeKind::Extension, &ext.id, fields))
}

/// The change item that asks the owner to apply or remove an extension.
fn extension_item(o: &Reach, me: &Card, title: String, detail: String, change: PendingChange) -> InboxItem {
    InboxItem {
        id: new_id("i_"),
        home: me.home.clone(),
        card_id: me.id.clone(),
        kind: InboxKind::Change,
        title,
        detail,
        options: vec![
            InboxOption {
                id: CHANGE_APPLY.to_owned(),
                name: "Apply".to_owned(),
                ..InboxOption::default()
            },
            InboxOption {
                id: CHANGE_NOT_NOW.to_owned(),
                name: "Not now".to_owned(),
                ..InboxOption::default()
            },
        ],
        status: InboxStatus::Open,
        created_at: o.now(),
        change: Some(change),
        ..InboxItem::default()
    }
}

/// Copies from into to while hashing it, at most limit bytes and one more: it returns the hash and the bytes copied.
fn copy_hashed(from: &mut impl Read, to: &mut impl Write, limit: u64) -> std::io::Result<(String, u64)> {
    let mut h = Sha256::new();
    let mut size: u64 = 0;
    let mut buf = vec![0u8; 64 * 1024];
    let mut limited = from.take(limit.saturating_add(1));
    loop {
        let n = limited.read(&mut buf)?;
        if n == 0 {
            break;
        }
        let chunk = buf.get(..n).unwrap_or_default();
        h.update(chunk);
        to.write_all(chunk)?;
        size = size.saturating_add(u64::try_from(n).unwrap_or(u64::MAX));
    }
    to.flush()?;
    Ok((hex::encode(h.finalize()), size))
}

/// Makes the file at path executable by the server's own user alone.
fn owner_only_executable(path: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(())
    }
}

impl Reach {
    /// Copies the program at path into the staging directory under the name of its hash and returns where, its hash and its
    /// size.
    fn stage(&self, path: &str) -> Result<(PathBuf, String, u64)> {
        if self.cfg.ext_dir.as_os_str().is_empty() {
            return Err(Error::refused("this server has no extensions directory"));
        }
        let p = Path::new(path);
        if !p.is_absolute() {
            return Err(Error::refused(format!(
                "binary {:?} is not an absolute path",
                gist(path, 80)
            )));
        }
        let info = std::fs::metadata(p).map_err(|e| Error::refused(format!("binary: {e}")))?;
        if !info.is_file() {
            return Err(Error::refused(format!("binary {:?} is not a file", gist(path, 80))));
        }
        if info.len() == 0 || info.len() > MAX_EXTENSION_PROGRAM {
            return Err(Error::refused(format!(
                "the program is {} bytes: at least 1 and at most {MAX_EXTENSION_PROGRAM}",
                info.len()
            )));
        }
        let dir = self.cfg.ext_dir.join(EXTENSION_STAGING);
        std::fs::create_dir_all(&dir).map_err(|e| Error::refused(format!("the staging directory: {e}")))?;
        self.prune_staging(&dir);
        let mut input = std::fs::File::open(p).map_err(|e| Error::refused(format!("binary: {e}")))?;
        let tmp = dir.join(format!("incoming-{}", new_id("")));
        let copied =
            std::fs::File::create(&tmp).and_then(|mut out| copy_hashed(&mut input, &mut out, MAX_EXTENSION_PROGRAM));
        let (sum, size) = match copied {
            Ok((sum, size)) if size <= MAX_EXTENSION_PROGRAM => (sum, size),
            Ok((_, size)) => {
                // a partial copy that cannot be removed is pruned later
                if let Err(err) = std::fs::remove_file(&tmp) {
                    tracing::warn!(%err, "a partial copy of a program was not removed");
                }
                return Err(Error::refused(format!("copying the program: it grew to {size} bytes")));
            }
            Err(e) => {
                // a partial copy that cannot be removed is pruned later
                if let Err(err) = std::fs::remove_file(&tmp) {
                    tracing::warn!(%err, "a partial copy of a program was not removed");
                }
                return Err(Error::refused(format!("copying the program: {e}")));
            }
        };
        let staged = dir.join(&sum);
        owner_only_executable(&tmp).map_err(|e| Error::refused(format!("the staged program: {e}")))?;
        std::fs::rename(&tmp, &staged).map_err(|e| Error::refused(format!("the staged program: {e}")))?;
        Ok((staged, sum, size))
    }

    /// Removes the staged programs no proposal can still be waiting for.
    fn prune_staging(&self, dir: &Path) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        let now = self.now();
        for e in entries.flatten() {
            let Ok(modified) = e.metadata().and_then(|m| m.modified()) else {
                continue;
            };
            if chains::health::elapsed(model::Time(time::OffsetDateTime::from(modified)), now) > STAGING_KEEPS
                && let Err(err) = std::fs::remove_file(e.path())
            {
                tracing::warn!(file = ?e.file_name(), %err, "a staged program was not removed");
            }
        }
    }

    /// Raises the change item that asks the owner to remove the extension input.id: its manifest, its program and the
    /// source. The connections he made for it and the grants of chains stay in the store, naming a source that no longer
    /// exists.
    pub(crate) async fn propose_extension_removal(&self, me: &Card, input: RemoveExtensionInput) -> Result<String> {
        if input.why.trim().is_empty() {
            return Err(Error::refused("why is empty: the owner reads it"));
        }
        let all = self.store.read(|r| r.extensions()).await?;
        let Some(cur) = stored_extension(&all, &input.id)? else {
            return Err(Error::refused(format!(
                "no extension {:?} is applied: the ones that are {}",
                gist(&input.id, 40),
                extension_ids(&all)
            )));
        };
        let fields = removal_rail(&cur);
        let detail = format!(
            "{} asks to remove the source {} ({}): its program is deleted and agents can no longer run it. Why, in the Chair's words: {}\n\n{}",
            me.name,
            code_span(&cur.id),
            cur.label,
            code_span(&gist(&input.why, 400)),
            change_detail(&me.name, &ChangeKind::Extension, &cur.id, &fields)
        );
        let record = serde_json::to_string(&ExtensionProposal {
            manifest: cur.clone(),
            remove: true,
            why: input.why.clone(),
            ..ExtensionProposal::default()
        })?;
        let item = extension_item(
            self,
            me,
            format!("{} asks to remove extension {}", me.name, cur.id),
            detail,
            PendingChange {
                kind: ChangeKind::Extension,
                name: cur.id.clone(),
                record,
                fields,
            },
        );
        self.raise_change(item).await?;
        Ok(format!(
            "The removal of extension {} waits in the owner's inbox, where the Assistant decides it for him. You are told the answer; until then the source works as before.",
            cur.id
        ))
    }

    /// Raises the change item that asks the owner to apply the extension input, as the Chair me.
    pub(crate) async fn propose_extension(&self, me: &Card, input: ProposeExtensionInput) -> Result<String> {
        if self.bundled.lookup(&input.id).is_some() {
            return Err(Error::refused(format!(
                "id {:?} is a plugin that ships with Remit",
                input.id
            )));
        }
        let proposal = if input.auth.is_some() {
            ExtensionProposal {
                manifest: api_extension_of(&input)?,
                ..ExtensionProposal::default()
            }
        } else if input.binary.is_empty() {
            return Err(Error::refused(
                "give binary, the path of the built plugin, or auth for an API the platform reaches",
            ));
        } else {
            let (staged, sum, size) = self.stage(&input.binary)?;
            let describe_as = new_id(DESCRIBE_AS);
            let described = self
                .plugins
                .plugin(&plugin::Spec {
                    id: describe_as.clone(),
                    binary: staged.clone(),
                    args: Vec::new(),
                    sha256: sum.clone(),
                })
                .describe()
                .await;
            self.plugins.stop(&describe_as);
            let d = described.map_err(|e| {
                Error::refused(format!(
                    "the program did not describe itself as a plugin (does main serve the plugin contract, built against this Remit?): {e}"
                ))
            })?;
            let file = Path::new(&input.binary)
                .file_name()
                .map(|f| f.to_string_lossy().into_owned())
                .unwrap_or_default();
            let manifest = extension_of(&input.id, &file, &sum, &d).map_err(|e| {
                Error::refused(format!(
                    "what the program describes is not an extension the server accepts: {e}"
                ))
            })?;
            ExtensionProposal {
                manifest,
                staged: staged.to_string_lossy().into_owned(),
                size,
                ..ExtensionProposal::default()
            }
        };
        let next = proposal.manifest.clone();
        let all = self.store.read(|r| r.extensions()).await?;
        let cur = stored_extension(&all, &next.id)?;
        let fields = extension_rail(cur.as_ref(), &next, proposal.size);
        if fields.is_empty() {
            return Err(Error::refused(format!(
                "{} is applied as it is: nothing to change",
                next.id
            )));
        }
        let item = extension_item(
            self,
            me,
            format!("{} asks to add extension {}", me.name, next.id),
            extension_detail(&me.name, &next, &fields),
            PendingChange {
                kind: ChangeKind::Extension,
                name: next.id.clone(),
                record: serde_json::to_string(&proposal)?,
                fields,
            },
        );
        self.raise_change(item).await?;
        Ok(format!(
            "Extension {} waits in the owner's inbox: the Assistant reads for him what the program says it is (its tools and the grant each needs, its credential variables, its events and frontend) with the program's hash, and applies it or not. You are told the answer ([change result …]); until then nothing of it exists.",
            next.id
        ))
    }

    /// Stores the extension a change item asks for, as the owner who answered it allowed it, if what it replaces is still
    /// what the owner was shown (moved when it is not, and nothing is stored). The program is installed first, under
    /// a name that holds its hash: a transaction that does not commit leaves a file no manifest names. What it returns is
    /// done once the transaction commits (`after_extension`).
    pub fn apply_extension(&self, tx: &Tx<'_>, pending: &PendingChange, w: &Authorship) -> Result<Applied> {
        if !w
            .approved_by
            .as_ref()
            .is_some_and(|by| for_owner(ACT_APPLY, by).is_ok())
        {
            return Err(Error::refused(ACT_APPLY.refusal()));
        }
        if self.cfg.ext_dir.as_os_str().is_empty() {
            return Err(Error::refused("this server has no extensions directory"));
        }
        let p: ExtensionProposal = serde_json::from_str(&pending.record)?;
        if p.remove {
            return self.apply_extension_removal(tx, pending, &p, w);
        }
        p.manifest
            .validate()
            .map_err(|e| Error::refused(format!("the write is no longer one the extensions accept: {e}")))?;
        let all = tx.extensions()?;
        let cur = stored_extension(&all, &p.manifest.id)?;
        if extension_rail(cur.as_ref(), &p.manifest, p.size) != pending.fields {
            return Ok(Applied::Moved);
        }
        self.install_extension_program(&p)?;
        let next = &p.manifest;
        tx.put_extension(&StoredExtension {
            id: next.id.clone(),
            doc: doc_of(Some(next))?,
            applied_at: self.now(),
        })?;
        record_change(
            tx,
            w,
            self.now(),
            ChangeKind::Extension,
            &next.id,
            cur.as_ref(),
            Some(next),
        )?;
        Ok(after(
            self.me(),
            AfterApply::Applied {
                id: next.id.clone(),
                staged: p.staged.clone(),
                import: p.import.clone(),
            },
        ))
    }

    /// Removes the extension a change item asks to remove, if it is still the one the owner was shown. Its program is deleted
    /// once the removal is committed: a transaction that does not commit leaves it, and a manifest that names a file that is
    /// gone does not run.
    fn apply_extension_removal(
        &self,
        tx: &Tx<'_>,
        pending: &PendingChange,
        p: &ExtensionProposal,
        w: &Authorship,
    ) -> Result<Applied> {
        let all = tx.extensions()?;
        let Some(cur) = stored_extension(&all, &p.manifest.id)? else {
            return Ok(Applied::Moved);
        };
        if removal_rail(&cur) != pending.fields {
            return Ok(Applied::Moved);
        }
        tx.delete_extension(&cur.id)?;
        tx.delete_disabled_extension(&cur.id)?;
        record_change::<Extension>(tx, w, self.now(), ChangeKind::Extension, &cur.id, Some(&cur), None)?;
        Ok(after(self.me(), AfterApply::Removed { id: cur.id }))
    }

    /// What follows an extension's apply once it committed: the staged copy goes, the old process ends, the sources take the
    /// extensions as they are now, and the owner's screens are told.
    pub(crate) async fn after_extension(self: &Arc<Self>, after: AfterApply) {
        match after {
            AfterApply::Applied { id, staged, import } => {
                if !staged.is_empty()
                    && let Err(err) = std::fs::remove_file(&staged)
                    && err.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::warn!(id, %err, "a staged program was not removed");
                }
                self.plugins.stop(&id);
                self.load_extensions().await;
                self.publish_extension(&id).await;
                match import {
                    // its values, then its first connect (credentials.rs `source_changed`)
                    Some(imp) => self.import_mcp_values(&id, &imp).await,
                    None => self.pin_mcp(&id).await,
                }
            }
            AfterApply::Removed { id } => {
                self.plugins.stop(&id);
                let dir = self.cfg.ext_dir.join(&id);
                // the id was validated as a word when it was applied; a path that leaves the directory is never removed
                if dir.parent() == Some(self.cfg.ext_dir.as_path())
                    && !id.contains(['/', '\\', '.'])
                    && let Err(err) = std::fs::remove_dir_all(&dir)
                    && err.kind() != std::io::ErrorKind::NotFound
                {
                    tracing::error!(id, %err, "the program of a removed extension was not deleted");
                }
                self.load_extensions().await;
                self.publish_extension(&id).await;
            }
        }
    }

    /// Installs the program the owner read under the extensions directory, by the name its hash gives it; an API behind the
    /// proxy has none. The file is the staged one, checked against the manifest's pin as it is copied.
    fn install_extension_program(&self, p: &ExtensionProposal) -> Result<()> {
        if p.manifest.program.is_empty() {
            return Ok(());
        }
        let mut input = std::fs::File::open(&p.staged)
            .map_err(|e| Error::refused(format!("the staged program is gone ({e}): propose the extension again")))?;
        let dest = self.cfg.ext_dir.join(&p.manifest.program);
        let dir = dest
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.cfg.ext_dir.clone());
        std::fs::create_dir_all(&dir).map_err(|e| Error::refused(format!("the extension's directory: {e}")))?;
        let tmp = dir.join(format!("installing-{}", new_id("")));
        let installed = std::fs::File::create(&tmp)
            .and_then(|mut out| copy_hashed(&mut input, &mut out, MAX_EXTENSION_PROGRAM))
            .map_err(|e| e.to_string())
            .and_then(|(sum, _)| {
                if sum == p.manifest.sha256 {
                    Ok(())
                } else {
                    Err("the staged program is not the one the manifest is pinned to".to_owned())
                }
            })
            .and_then(|()| owner_only_executable(&tmp).map_err(|e| e.to_string()))
            .and_then(|()| std::fs::rename(&tmp, &dest).map_err(|e| e.to_string()));
        if let Err(e) = installed {
            // a partial copy that cannot be removed is a file no manifest names
            if let Err(err) = std::fs::remove_file(&tmp) {
                tracing::warn!(%err, "a partial copy of a program was not removed");
            }
            return Err(Error::refused(format!("the extension's program: {e}")));
        }
        Ok(())
    }

    /// Makes the extensions the server holds, but the ones the owner disabled, the sources' extensions, the plugin pipeline's
    /// and the plugins whose connections are accounts: at the start, and after the owner applied, removed, enabled or
    /// disabled one.
    pub async fn load_extensions(self: &Arc<Self>) {
        let installed = match self.installed_extensions().await {
            Ok(installed) => installed,
            Err(err) => {
                tracing::error!(%err, "the extensions were not read");
                return;
            }
        };
        let disabled = match self.store.read(|r| r.disabled_extensions()).await {
            Ok(d) => d,
            Err(err) => {
                tracing::error!(%err, "the disabled extensions were not read");
                return;
            }
        };
        let exts: Vec<Extension> = installed
            .into_iter()
            .map(|i| i.ext)
            .filter(|e| !disabled.iter().any(|d| d.id == e.id))
            .collect();
        self.sources.use_extensions(&self.cfg.ext_dir, &exts);
        if let Ok(d) = self.domains() {
            d.use_pipeline(&self.cfg.ext_dir, &exts);
        }
        self.use_account_plugins(&exts);
        self.core.use_plugin_accounts().await;
    }
}

impl Reach {
    /// Gives propose_extension, and the tools that bring in an MCP server, for the Chair of card card_id.
    pub fn add_extension_tool(self: &Arc<Self>, t: &mut Tools, card_id: &str) {
        self.add_mcp_server_tools(t, card_id); // an MCP server is an extension too (ADR-0077)
        let card_id = card_id.to_owned();
        t.serving(self).add(
            "propose_extension", chains::tools::Effect::Writes,
            "Add a source Remit does not ship (a payment system, a CRM, an internal API) as a plugin, as a change item the Assistant decides for the owner. Build a program that serves the plugin contract (crate plugin-sdk) and give its absolute path as binary: it says its label, capability, credential variables, tools, events and frontend itself, and the item shows all of it with the program's hash. For an API the platform can reach with a credential and no program, give auth instead. Nothing exists until it is applied; the credential values the owner types into his inbox, and you never see them.",
            move |o, input: ProposeExtensionInput| {
                let card_id = card_id.clone();
                async move {
                    let me = o.store.read(move |r| r.card(&card_id)).await?;
                    o.propose_extension(&me, input).await
                }
            },
        );
    }

    /// Gives remove_extension, for the Chair of card card_id.
    pub fn add_extension_removal_tool(self: &Arc<Self>, t: &mut Tools, card_id: &str) {
        let card_id = card_id.to_owned();
        t.serving(self).add(
            "remove_extension", chains::tools::Effect::Writes,
            "Ask to remove an applied extension (a source written as a program): its manifest, its program and the source go. Nothing changes until it is applied in the owner's inbox, where the Assistant decides it for him; you are told the answer.",
            move |o, input: RemoveExtensionInput| {
                let card_id = card_id.clone();
                async move {
                    let me = o.store.read(move |r| r.card(&card_id)).await?;
                    o.propose_extension_removal(&me, input).await
                }
            },
        );
    }
}
