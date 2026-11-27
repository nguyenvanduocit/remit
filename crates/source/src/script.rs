//! Scripts, the form T1 of ADR-0018: an agent runs a script for batch or repeated work on a system and never holds its
//! credential. The script runs (the Runner, as for a program) with its HTTP clients sent to a proxy that checks each request
//! and adds the connection's credential before it goes on: the script cannot print what it never had. What the proxy lets
//! through is the source's statement (`InjectSpec`): GET anywhere under the connection's address, POST only where the source
//! says a POST reads. That the script only reads is enforced by the proxy's method and path rules, and is said so wherever it
//! is shown (`SCRIPT_ENFORCED`); the credential's own rights are not what keeps it read-only.
//!
//! This file is the form's whole contract on the source side: `InjectSpec::check` is pure, the run is the shell around it
//! (`script_run`), and a source is reached this way by answering `Provider::inject`, as Grafana, GitHub and Datadog do.

use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use grant::Effect;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::connection::{Resolved, ServedTool, Set, asked_connection, key};
use crate::error::{BoxError, Error, Result, err, fail};
use crate::gate::{Caller, Form};
use crate::program::{Job, Output, PROGRAM_PATH, finished};
use crate::redact::{mask_emails, scrub};
use crate::source::{FROM_OUTSIDE, Kind, Provider, schema_of, with_connection};
use crate::text::{OUTPUT_MAX, limit};
use crate::when::duration_text;

/// The one tool through which an agent runs a script: it takes the source, so the grammar is one for every source.
pub const SCRIPT_TOOL: &str = "script";

/// How this form keeps a script from writing, in the words every surface that shows the form uses: the tool's description,
/// list_sources and the head of each answer. A source's spec is checked by code, the credential's rights are not what is
/// relied on, and that is a weaker claim than a typed tool's, so it is said as it is.
pub const SCRIPT_ENFORCED: &str =
    "Enforced: proxy (method and path allow-list; read-only is not the credential's rights)";

/// Under the call's deadline, so the runner ends the script and says so before the gate's deadline does.
pub(crate) const SCRIPT_TIMEOUT: Duration = Duration::from_secs(25);
const SCRIPT_MAX_CODE: usize = 64 << 10;
/// Requests one run may make, refused ones included.
pub(crate) const SCRIPT_REQUESTS: u64 = 500;
/// Bytes of one request body.
pub(crate) const SCRIPT_BODY: u64 = 1 << 20;
/// Bytes of one answer the script reads.
pub(crate) const SCRIPT_ANSWER: u64 = 8 << 20;

/// Makes the credential from the request the upstream receives (a signature over its method, path, query, headers and
/// body), as the injecting proxy calls it: the request's head with its absolute address, its body and the time.
pub type Sign =
    Arc<dyn Fn(&mut http::request::Parts, &[u8], SystemTime) -> std::result::Result<(), BoxError> + Send + Sync>;

/// Removes the credential from text the upstream answered with.
pub type Scrub = Arc<dyn Fn(&str) -> String + Send + Sync>;

/// Everything about a script's way out except the script, which is the agent's. A source builds it from its own
/// connection; an agent never sees `headers` or `secrets`.
#[derive(Clone, Default)]
pub struct InjectSpec {
    /// The connection's address: scheme://host[:port][/prefix]. It is the only host the script reaches.
    pub base: String,
    /// The credential: added to every request, replacing the script's own.
    pub headers: BTreeMap<String, String>,
    /// The paths where POST only reads (a search that takes its query in a body). Everywhere else only GET passes.
    pub read_posts: Vec<String>,
    /// When set, makes the credential from the request instead of, or besides, `headers`. It is the proxy's: the script
    /// never sees what it signs with.
    pub sign: Option<Sign>,
    /// The values of the credential, each in every spelling an upstream may echo it in: removed from everything the script
    /// is answered and from everything it wrote, before an agent reads it.
    pub secrets: Vec<String>,
}

impl InjectSpec {
    /// Whether the spec can be run: an http or https address with a host and no login, and a credential to add. It is pure;
    /// the proxy checks the same again, because it is the one that enforces.
    pub fn check(&self) -> Result<()> {
        let Some(base) = url::Url::parse(&self.base)
            .ok()
            .filter(|b| matches!(b.scheme(), "http" | "https") && b.host_str().is_some_and(|h| !h.is_empty()))
        else {
            return fail!(
                "the address {:?} is not an http or https address",
                limit(&self.base, 80)
            );
        };
        if !base.username().is_empty() || base.password().is_some() {
            return fail!(
                "the address holds a login: the credential is the connection's variables, not part of the address"
            );
        }
        if self.headers.is_empty() && self.sign.is_none() {
            return fail!("the connection has no credential to add");
        }
        Ok(())
    }

    /// What removes the credential from text the upstream answers with, or a program printed: every spelling of
    /// `secrets`.
    #[must_use]
    pub fn scrubber(&self) -> Scrub {
        let secrets = self.secrets.clone();
        Arc::new(move |text: &str| scrub(text, &secrets))
    }

    /// What the proxy lets a script do, in words, for the description and list_sources.
    fn reads(&self) -> String {
        if self.read_posts.is_empty() {
            return "GET anywhere under its API; POST nowhere".to_owned();
        }
        format!(
            "GET anywhere under its API; POST only at {}",
            self.read_posts.join(", ")
        )
    }
}

/// The spellings of an Authorization value an upstream may echo back: the whole value, its credential without the scheme
/// ("Bearer abc" echoes as "abc" in a log line), and any further parts the value is made of (a password).
pub(crate) fn auth_secrets(authorization: &str, parts: &[String]) -> Vec<String> {
    let mut out = vec![authorization.to_owned()];
    if let Some((_, credential)) = authorization.split_once(' ') {
        out.push(credential.to_owned());
    }
    out.extend(parts.iter().cloned());
    out
}

/// A way to run a script: the program that reads it and the arguments before the script's path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Language {
    pub name: &'static str,
    pub binary: &'static str,
    pub ext: &'static str,
    pub args: &'static [&'static str],
}

/// The languages a script may be written in. The host says which of them it has.
pub(crate) fn script_languages() -> Vec<Language> {
    vec![
        // -I: no PYTHON* variable and no user site decides what the script imports.
        Language {
            name: "python3",
            binary: "python3",
            ext: ".py",
            args: &["-I"],
        },
        Language {
            name: "sh",
            binary: "sh",
            ext: ".sh",
            args: &[],
        },
        Language {
            name: "node",
            binary: "node",
            ext: ".js",
            args: &[],
        },
    ]
}

/// What an agent gives the script tool; the kernel adds `connection`.
#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
pub struct ScriptInput {
    /// the source whose API the script calls, as list_sources names it
    pub source: String,
    /// the language of the script
    pub language: String,
    /// the whole script, as text; it reads the connection's address from the API_BASE environment variable and prints its result to stdout
    pub code: String,
}

/// The language, size and hash of a script: enough to tell which script a record is about, without the script.
fn script_id(language: &str, code: &str) -> String {
    format!(
        "{language}, {} bytes, sha256 {}",
        code.len(),
        hex::encode(Sha256::digest(code.as_bytes()))
    )
}

/// What the recorder keeps of a call's input.
fn script_detail(input: &Value) -> String {
    match serde_json::from_value::<ScriptInput>(input.clone()) {
        Ok(typed) => format!("script {}", script_id(&typed.language, &typed.code)),
        Err(_) => String::new(),
    }
}

/// How a Job reaches its source through the injecting proxy instead of a gate: the connection's address and credential,
/// what a POST may read, and the caps of one run. The runner puts the proxy, its certificate authority and the address
/// (API_BASE) into the script's environment; the job's own environment holds none of them.
#[derive(Clone)]
pub struct InjectJob {
    pub base: String,
    pub headers: BTreeMap<String, String>,
    pub sign: Option<Sign>,
    pub read_posts: Vec<String>,
    /// Removes the credential from what the upstream answers.
    pub scrub: Scrub,
    /// How many requests the script may make.
    pub max_requests: u64,
    /// The bytes of one request body.
    pub max_body: u64,
    /// The bytes of one answer.
    pub max_answer: u64,
}

impl Set {
    /// How a step of a repository's release reaches connection of source (ADR-0116): its address, credential and
    /// scrub, for the injecting proxy the release's runner makes in this process. The step never sees it; whether the
    /// repository may reach the connection, and with which effect, is its grant's, asked before this. Refused for a
    /// source that reaches nothing through a proxy and for a connection with no credential yet, which the owner types.
    pub async fn injection(&self, source: &str, connection: &str) -> Result<InjectSpec> {
        let Some(k) = self.kind(source) else {
            return Err(crate::connection::no_source(source));
        };
        let name = self.pick(&k, connection).await?;
        let p = self.provider(&k, &name).await?;
        let m = p.missing();
        if !m.is_empty() {
            return fail!(
                "{} has no credential on connection {name} yet: set {}; the owner types it in the connection's item",
                p.name(),
                m.join(", ")
            );
        }
        let Some(spec) = p.inject() else {
            return fail!("{} is reached by no proxy, so no release step reaches it", p.name());
        };
        spec.check()?;
        Ok(spec)
    }

    /// Whether this Set offers the script tool: it has a place to run one and a language to write it in.
    pub(crate) fn can_script(&self) -> bool {
        self.inner.runner.is_some() && !self.inner.languages.is_empty()
    }

    fn language_names(&self) -> Vec<&'static str> {
        self.inner.languages.iter().map(|l| l.name).collect()
    }

    /// Whether `input` may run: the language is one of this host's and the code is text of a size the tool takes.
    fn check_script(&self, input: &ScriptInput) -> Result<Language> {
        let Some(lang) = self.inner.languages.iter().find(|l| l.name == input.language) else {
            return fail!(
                "language {:?} is not one this server runs: the ones that run are {}",
                limit(&input.language, 40),
                self.language_names().join(", ")
            );
        };
        if input.code.trim().is_empty() {
            return fail!("code is empty");
        }
        if input.code.len() > SCRIPT_MAX_CODE {
            return fail!("the script is {} bytes: at most {SCRIPT_MAX_CODE}", input.code.len());
        }
        if input.code.contains('\0') {
            return fail!("the script holds a NUL byte");
        }
        Ok(lang.clone())
    }

    /// Runs the script behind the proxy of `spec`: it checks the script, writes it into a scratch directory, has the Runner
    /// run it and answers what it left behind, email addresses masked. A script that could not be run is an error.
    async fn script_run(&self, spec: &InjectSpec, input: &ScriptInput) -> Result<(Language, Output)> {
        let lang = self.check_script(input).map_err(|e| e.context("script"))?;
        spec.check().map_err(|e| e.context("script"))?;
        let Some(runner) = self.inner.runner.clone() else {
            return fail!("script: no script runs on this server");
        };
        let dir = tempfile::Builder::new()
            .prefix("remit-script-")
            .tempdir()
            .map_err(|e| err!("script: scratch directory: {e}"))?;
        let file = dir.path().join(format!("script{}", lang.ext));
        std::fs::write(&file, &input.code).map_err(|e| err!("script: {e}"))?;
        let mut argv = vec![lang.binary.to_owned()];
        argv.extend(lang.args.iter().map(|a| (*a).to_owned()));
        argv.push(file.to_string_lossy().into_owned());
        let d = dir.path().to_string_lossy().into_owned();
        let secrets = spec.secrets.clone();
        let job = Job {
            argv,
            env: vec![
                format!("PATH={PROGRAM_PATH}"),
                format!("HOME={d}"),
                format!("TMPDIR={d}"),
                "LANG=C.UTF-8".to_owned(),
            ],
            dir: dir.path().to_path_buf(),
            egress: Vec::new(),
            inject: Some(InjectJob {
                base: spec.base.clone(),
                headers: spec.headers.clone(),
                sign: spec.sign.clone(),
                read_posts: spec.read_posts.clone(),
                scrub: Arc::new(move |text: &str| scrub(text, &secrets)),
                max_requests: SCRIPT_REQUESTS,
                max_body: SCRIPT_BODY,
                max_answer: SCRIPT_ANSWER,
            }),
            timeout: SCRIPT_TIMEOUT,
            max_output: OUTPUT_MAX,
        };
        let mut out = runner
            .run(&job)
            .await
            .map_err(|e| Error::msg(format!("script: {}", scrub(&e.to_string(), &spec.secrets))))?;
        out.stdout = mask_emails(&out.stdout);
        out.stderr = mask_emails(&out.stderr);
        Ok((lang, out))
    }

    /// The script tool on one connection: the act of a script behind the proxy (form proxy), a read as far as the proxy's
    /// rules go.
    pub(crate) fn script_resolved(&self, p: &Arc<dyn Provider>, k: &Kind, connection: &str) -> Result<Resolved> {
        let Some(spec) = p.inject().filter(|_| self.can_script()) else {
            return fail!("{} runs no script on this server", p.name());
        };
        let mut res = Resolved::new(key(&k.id, connection), connection, Effect::Read, Form::Proxy);
        res.detail = Some(Arc::new(script_detail));
        let (set, tool_spec) = (self.clone(), spec.clone());
        res.exec = Some(Arc::new(move |args: Value| {
            let (set, spec) = (set.clone(), tool_spec.clone());
            Box::pin(async move {
                let typed: ScriptInput =
                    serde_json::from_value(args).map_err(|e| err!("script: the input does not fit: {e}"))?;
                let (lang, out) = set.script_run(&spec, &typed).await?;
                let head = format!(
                    "form: script, the credential is added by a proxy and the script holds none. {SCRIPT_ENFORCED}\n{}\n",
                    script_id(lang.name, &typed.code)
                );
                finished(SCRIPT_TOOL, lang.name, &head, &spec.secrets, &out, SCRIPT_TIMEOUT)
            })
        }));
        let set = self.clone();
        res.script = Some(Arc::new(move |input: ScriptInput| {
            let (set, spec) = (set.clone(), spec.clone());
            Box::pin(async move {
                let (lang, out) = set.script_run(&spec, &input).await?;
                finished(SCRIPT_TOOL, lang.name, "", &spec.secrets, &out, SCRIPT_TIMEOUT)?;
                Ok(scrub(&out.stdout, &spec.secrets))
            })
        }));
        Ok(res)
    }

    /// Runs a script of a source's connection for Remit itself and answers what it printed, with no head and no stderr: a
    /// sensor (ADR-0043) reads the answer as one JSON value. It passes the gate as the script tool does: the policy decides it
    /// for `caller`, a read behind the proxy, and the recorder is told of it once. A returned error is safe to show.
    pub async fn run_script(
        &self,
        caller: &Caller,
        source: &str,
        connection: &str,
        input: ScriptInput,
    ) -> Result<String> {
        let gate = self.script_gate(source);
        let detail_input = serde_json::json!({"source": source, "language": input.language, "code": input.code});
        let source = source.to_owned();
        gate.pass(caller, SCRIPT_TOOL, connection, &detail_input, move |res| async move {
            match res.script {
                Some(script) => script(input).await,
                None => fail!("{source} runs no script"),
            }
        })
        .await
    }

    /// The gate of a source that runs scripts; for any other name, a gate whose resolution fails with the reason, so the
    /// call is still recorded once.
    fn script_gate(&self, source: &str) -> crate::gate::Gate {
        let mut can = Vec::new();
        for k in self.all() {
            if self.template(&k).inject().is_none() {
                continue;
            }
            if k.id == source {
                return self.gate(&k);
            }
            can.push(k.id);
        }
        self.refusing_gate(
            source,
            format!(
                "script: no source {:?} runs scripts here: the ones that do are {}",
                limit(source, 40),
                can.join(", ")
            ),
        )
    }

    /// The script tool, once, for the injected sources among `held`. Nothing is served without a Runner, a language or a
    /// source to run.
    pub(crate) fn script_tool(&self, held: &[Kind]) -> Option<ServedTool> {
        let ks: Vec<Kind> = held
            .iter()
            .filter(|k| self.template(k).inject().is_some())
            .cloned()
            .collect();
        if !self.can_script() || ks.is_empty() {
            return None;
        }
        let languages = self.language_names();
        let mut schema = schema_of::<ScriptInput>();
        for (property, values) in [
            ("source", ks.iter().map(|k| k.id.clone()).collect::<Vec<_>>()),
            ("language", languages.iter().map(|l| (*l).to_owned()).collect()),
        ] {
            if let Some(prop) = schema
                .pointer_mut(&format!("/properties/{property}"))
                .and_then(Value::as_object_mut)
            {
                prop.insert("enum".to_owned(), values.into_iter().map(Value::String).collect());
            }
        }
        let set = self.clone();
        let description = script_description(self, &ks, &languages);
        Some(ServedTool::new(
            SCRIPT_TOOL.to_owned(),
            description,
            with_connection(schema),
            false, // a program's or script's effect is its form's, declared per call: it may write
            Arc::new(move |caller: Caller, args: Value| {
                let (set, ks) = (set.clone(), ks.clone());
                Box::pin(async move {
                    let source = args
                        .get("source")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    let Some(k) = ks.iter().find(|k| k.id == source) else {
                        let ids: Vec<&str> = ks.iter().map(|k| k.id.as_str()).collect();
                        return fail!(
                            "script: no source {:?} runs scripts here: the ones that do are {}",
                            limit(&source, 40),
                            ids.join(", ")
                        );
                    };
                    let connection = asked_connection(&args);
                    set.gate(k).run(&caller, SCRIPT_TOOL, &connection, args).await
                })
            }),
        ))
    }
}

/// What the script tool does for one source: its spec's own statement, so what is allowed is said once.
pub(crate) fn script_what(name: &str, spec: &InjectSpec) -> String {
    format!("{name}: a script may {}", spec.reads())
}

/// The script tool's description over the injected sources `ks` and this host's languages.
fn script_description(set: &Set, ks: &[Kind], languages: &[&str]) -> String {
    let mut sources = String::new();
    for k in ks {
        let p = set.template(k);
        if let Some(spec) = p.inject() {
            sources.push_str(&format!("\n- {}: {}", k.id, script_what(&p.name(), &spec)));
        }
    }
    format!(
        "Run a script for batch or repeated work against the API of a source, without ever holding its credential: its HTTP clients go through a proxy that checks each request and adds the connection's credential, \
so the credential is not in its environment, its files or its output. Write it in one of: {}. The connection's address is in the environment variable API_BASE: call it with the language's own HTTP client \
(HTTPS_PROXY and the proxy's certificate authority are already set; Authorization is added for you, and one you send is replaced) and print the result to stdout. \
Use it when no typed tool answers or the work is a loop over many calls; a typed tool comes first. Sources and what each allows:{sources}\
\nRefused, and named in the answer: any other method, a path with .. or an encoded slash or dot, another host, a redirect to another host (never followed), more than {SCRIPT_REQUESTS} requests, a request body over {} MiB, an answer over {} MiB. \
The script starts in a scratch directory of its own and ends after {}. stdout and stderr are cut at {OUTPUT_MAX} bytes, tokens are removed and email addresses are masked. \
{SCRIPT_ENFORCED}. Needs a read on the connection (the chain's capability gives it, except on a protected connection, which needs a read grant the owner gave). {FROM_OUTSIDE}",
        languages.join(", "),
        SCRIPT_BODY >> 20,
        SCRIPT_ANSWER >> 20,
        duration_text(SCRIPT_TIMEOUT),
    )
}
