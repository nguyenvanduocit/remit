//! The rules every agent's session starts with: the deny rules Claude Code enforces in every mode (`deny_for`), the ask
//! rules that make it raise a permission prompt for what reaches outside the chain or names a secret (`ask_rules`), and
//! the secret rail: a prompt that names a secret is refused by code before anyone is asked, and nobody, the Chair, the
//! owner or the session itself, may allow one (`refuses_secret`, ADR-0059). Every other prompt goes up the ladder
//! (permission.rs).

use std::path::Path;

use agent::EDIT_TOOLS;
use model::{Card, CardRole, Ceiling, Chain};
use serde_json::{Map, Value};

use crate::cap_names::CAP_SHELL;
use crate::cap_names::CAP_WEB;
use crate::chain_core::Core;

/// Claude Code's tools that start a subagent (subagents.rs).
pub const SUBAGENT_TOOLS: [&str; 3] = ["Task", "Agent", "Workflow"];

/// Which capability a Claude Code tool waits on: a chain that does not hold it has the tool denied.
const HELD_BACK: [(&str, &[&str]); 2] = [(CAP_SHELL, &["Bash"]), (CAP_WEB, &["WebFetch", "WebSearch"])];

/// Whether a word of a prompt names a secret as a path: a secrets directory, a dotenv file (.env, .env.local, prod.env),
/// a key (id_rsa, *.pem), .netrc, or a file named credentials or token (credentials.json, .token) given as a path, never
/// the bare word, so `grep token` and auth/token.rs pass. It is the seam ADR-0059 names for telling a secret from an
/// ordinary file. A path is read case aside, segment by segment.
pub fn secret_path(word: &str) -> bool {
    let word = word.to_lowercase();
    let (dirs, last) = match word.rsplit_once('/') {
        Some((dirs, last)) => (Some(dirs), last),
        None => (None, word.as_str()),
    };
    let secret_dir = dirs.is_some_and(|dirs| {
        dirs.split('/')
            .any(|d| matches!(d, "secret" | "secrets" | ".aws" | ".ssh" | ".kube" | ".infisical"))
    });
    let dotenv = last.match_indices(".env").any(|(at, _)| {
        let after = last.get(at.saturating_add(4)..).unwrap_or_default();
        after.is_empty() || after.starts_with(['.', '*'])
    });
    let key = last.starts_with("id_ed25519") || last.starts_with("id_rsa") || word.ends_with(".pem");
    let named = matches!(last, ".netrc" | ".credentials" | ".token")
        || dirs.is_some() && matches!(last, "credentials" | "token" | "credentials.json" | "token.json");
    secret_dir || dotenv || key || named
}

/// The words of a prompt that may be paths, split where a shell or a list separates them (space, quote, pipe,
/// redirect, assignment), so a path is matched wherever it stands in a command: `cat .env | grep KEY` holds `.env`.
pub fn paths_in(text: &str) -> impl Iterator<Item = &str> {
    text.split(|c: char| c.is_whitespace() || ";|&<>()'\"=,`".contains(c))
        .filter(|w| !w.is_empty())
}

/// The Claude Code deny rules every agent starts with. Unlike the ask rules below, which raise a prompt only where a
/// permission mode asks, Claude Code enforces them in every mode and in every part of a compound command: pushing and
/// anything that merges, approves or opens an MR stays the backend's job: Remit lands every change itself (ADR-0114).
pub fn deny_tools() -> Vec<String> {
    let mut out: Vec<String> = [
        "Bash(git push:*)",
        "Bash(git remote add:*)",
        "Bash(git remote set-url:*)",
        "Bash(glab mr merge:*)",
        "Bash(glab mr accept:*)",
        "Bash(glab mr approve:*)",
        "Bash(glab mr create:*)",
        "Bash(glab mr update:*)",
        "Bash(glab mr close:*)",
        "Bash(glab mr note:*)",
        "Bash(glab mr rebase:*)",
        "Bash(gh pr merge:*)",
        "Bash(gh pr create:*)",
        // Claude Code's own timers and notifications: they never reach the owner and die with the agent's process. An
        // agent comes back later through subscribe, waits for a condition with Monitor, and reaches the owner through
        // ask_owner.
        "PushNotification",
        "CronCreate",
        "CronDelete",
        "CronList",
        "ScheduleWakeup",
        "RemoteTrigger",
        // Claude Code's messaging between sessions reaches every session on the machine, a developer's interactive one
        // included: an agent speaks to its Chair and its chain through route, ask_chair and ask_owner.
        "SendMessage",
        "ListAgents",
        // A worktree is Remit's to make (repo.rs): one an agent adds lands in the registry of the clone every chain
        // shares and outlives the agent. A tree at another commit is a clone in the agent's scratchpad (prompts.rs).
        "EnterWorktree",
        "ExitWorktree",
        "Bash(git worktree add:*)",
        // Claude Code's question to its user has no user here: it would reach the owner as a permission prompt of raw
        // JSON. The Assistant asks in its reply, in its panel; an agent of a chain asks through ask_chair and ask_owner.
        "AskUserQuestion",
    ]
    .map(str::to_owned)
    .to_vec();
    out.extend(merge_api_rules());
    out
}

/// The rules that refuse the API calls that merge or approve, the ones `glab mr merge`, `glab mr approve` and
/// `gh pr merge` make: a call through glab api or gh api that names the endpoint, whatever its method, flags and their
/// order, since no read names it. GitLab's REST endpoints end in /merge and /approve (a project path URL-encoded or
/// not), its GraphQL mutation is mergeRequestAccept; GitHub's endpoint ends in /merge, its mutations are
/// mergePullRequest and enablePullRequestAutoMerge. A rule is a tripwire on what a command says, not a wall: a script
/// or a raw HTTP call with the token still reaches the forge, which refuses the merge only when the token agents use
/// cannot merge (docs/design.md, Merging).
fn merge_api_rules() -> Vec<String> {
    let ends_in = |api: &str, endpoints: &[&str]| -> Vec<String> {
        endpoints
            .iter()
            .flat_map(|e| {
                ["", " *", "\"*", "'*", "?*", "/*", ".*"]
                    .iter()
                    .map(move |after| format!("Bash({api} */{e}{after})"))
            })
            .collect()
    };
    let names = |api: &str, mutations: &[&str]| -> Vec<String> {
        mutations.iter().map(|m| format!("Bash({api} *{m}*)")).collect()
    };
    [
        ends_in("glab api", &["merge", "approve"]),
        names("glab api", &["mergeRequestAccept"]),
        ends_in("gh api", &["merge"]),
        names("gh api", &["mergePullRequest", "enablePullRequestAutoMerge"]),
    ]
    .concat()
}

/// A card's deny rules: everyone's, plus no file edits for those who must change no file: who judge (a change, the
/// chain's result, or what else a judge Remit starts is given) and the Assistant, which acts through Remit's tools and
/// leaves changes to chains. A worker whose tools hold no edit tool has none to deny. A judge spawns nothing, since its
/// answer is its own; a Chair runs no Claude Code Workflow, since its session closes whenever its chain waits and would
/// cut the run (its scripts are Remit's).
pub fn deny_for(c: &Card, chain: Option<&Chain>) -> Vec<String> {
    let mut deny = deny_tools();
    deny.extend(withheld_tools(chain));
    // whether it may spawn subagents, and whether it may edit files
    let (spawns, edits) = match CardRole::of(c) {
        CardRole::Chair => {
            deny.push("Workflow".to_owned());
            (true, true)
        }
        CardRole::Assistant => (true, false),
        CardRole::Judge(_) => (false, false),
        // one hired to sign a verdict (a reviewer, a QC, a security reviewer) judges a change
        CardRole::Worker | CardRole::Spawned => {
            let works = c.signs.verdict_role().is_none();
            (works, works)
        }
    };
    if !spawns {
        deny.extend(SUBAGENT_TOOLS.map(str::to_owned));
    }
    if !edits {
        deny.extend(EDIT_TOOLS.map(str::to_owned));
    }
    deny
}

/// Whether an employee's ceiling lists tools and none of them changes a file or runs a command. An empty list restricts
/// nothing (an employee migrated with `ceiling: {}` keeps what its card is given), so only a non-empty one reads.
pub fn ceiling_read_only(c: &Ceiling) -> bool {
    !c.tools.is_empty() && !c.tools.iter().any(|t| t == "Bash" || EDIT_TOOLS.contains(&t.as_str()))
}

/// The Claude Code tools a chain's agents are denied because the chain holds no capability for them: Bash without
/// shell, WebFetch and WebSearch without web. A card that is no chain's (the Assistant's) has no chain to hold them
/// back and keeps what its own access gives.
pub fn withheld_tools(chain: Option<&Chain>) -> Vec<String> {
    let Some(chain) = chain else {
        return Vec::new();
    };
    HELD_BACK
        .iter()
        .filter(|(cap, _)| !chain.capabilities.iter().any(|c| c == cap))
        .flat_map(|(_, tools)| tools.iter().map(|t| (*t).to_owned()))
        .collect()
}

/// The tools a hire holding tools (none: every tool) lacks because its chain withholds them.
pub fn withheld_from_hire(chain: Option<&Chain>, tools: Option<&[String]>) -> Vec<String> {
    withheld_tools(chain)
        .into_iter()
        .filter(|t| tools.is_none_or(|tools| tools.contains(t)))
        .collect()
}

/// The capabilities that give the tools lacks.
fn capabilities_for(lacks: &[String]) -> Vec<&'static str> {
    HELD_BACK
        .iter()
        .filter(|(_, tools)| tools.iter().any(|t| lacks.iter().any(|l| l == t)))
        .map(|(cap, _)| *cap)
        .collect()
}

/// What tells the Chair which tools a hire lacks and what gives them; empty when it lacks none.
pub fn withheld_note(lacks: &[String]) -> String {
    if lacks.is_empty() {
        return String::new();
    }
    format!(
        " Its session lacks {}: the chain does not hold {}. Give it with grant_capability; once given, this agent holds the tool when it is opened again.",
        lacks.join(", "),
        capabilities_for(lacks).join(" and ")
    )
}

/// The refusal of a hire that lists a tool its chain withholds. A list is the Chair saying what the agent needs, so
/// starting the agent without it spends a whole turn on work it cannot do; a hire that lists nothing (every tool) is not
/// refused, as most of them need no shell, and is told which tools it lacks (`withheld_note`). Empty when nothing is
/// withheld.
pub fn named_but_withheld(chain: Option<&Chain>, tools: Option<&[String]>) -> String {
    if tools.is_none() {
        return String::new();
    }
    let lacks = withheld_from_hire(chain, tools);
    if lacks.is_empty() {
        return String::new();
    }
    let lacking = lacks.join(", ");
    format!(
        "the agent holds {lacking}, but the chain does not hold {}: give it with grant_capability first and hire again, or leave {lacking} out of tools",
        capabilities_for(&lacks).join(" and ")
    )
}

/// The Claude Code ask rules every agent starts with, own being Remit's own secret files (`own_secret_files`): a request
/// for what they match reaches the client even in auto mode, which asks about nothing else, and goes up the ladder
/// (permission.rs), or is refused at once when it names a secret (`refuses_secret`). They are the tripwire for what
/// reaches outside the chain or names a secret, not a sandbox: a command that reaches the same place another way is not
/// caught, and a read is not asked about (aws logs, kubectl get, curl without a body). The deny rules above are checked
/// first, so what they refuse never becomes a request. Claude Code matches `Bash(prefix:*)` by command prefix, `*`
/// anywhere in a Bash pattern, and a path pattern against the absolute path only when it starts with `//` (or `~/`):
/// `Read(**/.env)` would match no file outside the agent's cwd.
pub fn ask_rules(own: &[String]) -> Vec<String> {
    let own_names: Vec<String> = own.iter().map(|p| format!("*{}*", own_name(p))).collect();
    let mut out = static_ask_rules();
    out.extend(rules("Bash", &own_names));
    out.extend(rules("Read", &secret_reads(own)));
    out
}

/// The ask rules that name no path of Remit's own.
fn static_ask_rules() -> Vec<String> {
    let mut out = rules("Bash", &aws_ask_rules());
    out.extend(rules("Bash", &gh_ask_rules()));
    let others = [
        "kubectl apply:*",
        "kubectl delete:*",
        "kubectl edit:*",
        "kubectl patch:*",
        "kubectl scale:*",
        "kubectl rollout:*",
        "kubectl exec:*",
        "helm install:*",
        "helm upgrade:*",
        "helm uninstall:*",
        "sls deploy:*",
        "sls remove:*",
        "serverless deploy:*",
        "serverless remove:*",
        "terraform apply:*",
        "terraform destroy:*",
        "argocd app sync:*",
        "argocd app delete:*",
        "argocd app set:*",
        "npm publish:*",
        "docker push:*",
        "gh api * -X *",
        "gh api * --method*",
        "gh api * -f *",
        "gh api * -F *",
        "gh api * --field*",
        "gh api * --raw-field*",
        "gh api * --input*",
        "glab * create*",
        "glab * delete*",
        "glab * update*",
        "glab * close*",
        "glab * merge*",
        "glab * note*",
        "glab * approve*",
        "glab * rebase*",
        "glab * retry*",
        "glab * run*",
        "glab * cancel*",
        "glab * trigger*",
        "glab api * -X *",
        "glab api * --method*",
        "glab api * -f *",
        "glab api * -F *",
        "glab api * --field*",
        "glab api * --raw-field*",
        "glab api * --input*",
        "curl*-X*",
        "curl*--request*",
        "curl*-d *",
        "curl*--data*",
        "curl*-F *",
        "curl*--form*",
        "curl*-T *",
        "curl*--upload-file*",
        "wget*--post*",
        "wget*--method*",
        "wget*--body*",
        // A shell reads a secret in more ways than a rule can list; these are the ones by name, as a path or an
        // argument: `rg process.env src` and `rg credentials src` search code and are not asked about.
        "*/credentials*",
        "* .env*",
        "*/.env*",
        "*/secret/*",
        "*/.aws/*",
        "*/.ssh/*",
        "*/.kube/*",
        "*.pem*",
        "*id_rsa*",
        "*id_ed25519*",
        "*.netrc*",
    ];
    out.extend(others.iter().map(|p| format!("Bash({p})")));
    out
}

/// The paths of secrets no agent reads, as Claude Code's Read rules spell them, own being Remit's own secret files
/// (`own_secret_files`): `//**/name` is a file of that name anywhere, `~/dir/**` a directory of the home, `//path` the
/// file at that absolute path. They are asked about (`ask_rules`).
fn secret_reads(own: &[String]) -> Vec<String> {
    let mut reads: Vec<String> = [
        "//**/.env",
        "//**/.env.*",
        "//**/credentials",
        "//**/*.pem",
        "//**/id_rsa*",
        "//**/id_ed25519*",
        "//**/.netrc",
        "//**/secret/**",
        "~/.aws/**",
        "~/.ssh/**",
        "~/.kube/**",
        "~/.infisical/**",
    ]
    .map(str::to_owned)
    .to_vec();
    for p in own {
        if p.ends_with('/') {
            // a directory the owner named (own_secret_dirs): everything in it
            reads.push(format!("/{p}**"));
        } else {
            reads.push(format!("/{p}"));
        }
    }
    reads
}

/// The files of Remit's own that would take an agent past the rails, where its flags put them: the key the library's
/// credentials are sealed with (key_file), and the database (database), whose pauses and decisions are the rails' state,
/// and the files where the holders of client keys keep theirs (holders). All are absolute paths; one not set names
/// nothing. Asking before an agent reads them and refusing it is a tripwire, not a wall: agents run as the server's OS
/// user, so a command that names them another way (a script it wrote, a directory it lists) still reaches them
/// (docs/design.md, Calls made by the builder).
pub fn own_secret_files(key_file: &str, database: &str, holders: &[&str]) -> Vec<String> {
    [key_file, database]
        .iter()
        .chain(holders)
        .filter(|p| !p.is_empty())
        .map(|p| clean(p))
        .collect()
}

/// The directories of the owner's credentials that no agent reads (-secret-dirs), each with a trailing slash, which is
/// how the rules tell a directory from a file. A path not set names nothing.
pub fn own_secret_dirs(dirs: &[&str]) -> Vec<String> {
    dirs.iter()
        .filter(|d| !d.is_empty())
        .map(|d| format!("{}/", clean(d)))
        .collect()
}

impl Core {
    /// Remit's own secret files where the config puts them, and the directories the owner named.
    pub fn own_secrets(&self) -> Vec<String> {
        let text = |p: &Path| p.to_string_lossy().into_owned();
        let mut holders: Vec<String> = self.cfg.client_key_files.iter().map(|p| text(p)).collect();
        holders.push(text(&self.cfg.source_env_file));
        let holders: Vec<&str> = holders.iter().map(String::as_str).collect();
        let dirs: Vec<String> = self.cfg.secret_dirs.iter().map(|p| text(p)).collect();
        let dirs: Vec<&str> = dirs.iter().map(String::as_str).collect();
        let mut own = own_secret_files(&text(&self.cfg.secret_key_file), &text(&self.cfg.database), &holders);
        own.extend(own_secret_dirs(&dirs));
        own
    }
}

/// p cleaned lexically: one slash between names, no `.`, and each `..` taking the name before it.
fn clean(p: &str) -> String {
    let rooted = p.starts_with('/');
    let mut names: Vec<&str> = Vec::new();
    for name in p.split('/') {
        match name {
            "" | "." => {}
            ".." if names.last().is_some_and(|n| *n != "..") => {
                names.pop();
            }
            ".." if rooted => {}
            _ => names.push(name),
        }
    }
    let joined = names.join("/");
    match (rooted, joined.is_empty()) {
        (true, _) => format!("/{joined}"),
        (false, true) => ".".to_owned(),
        (false, false) => joined,
    }
}

/// How a command names an own secret file p: a path ending in its name, as `cat keys/sealing.bin` and
/// `sqlite3 ~/.remit/remit.db` do.
fn own_name(p: &str) -> String {
    let p = p.trim_end_matches('/');
    let base = p.rsplit('/').next().unwrap_or(p);
    format!("/{base}")
}

/// Whether text names one of own, Remit's own secret files, as a path.
fn names_own_secret(text: &str, own: &[String]) -> bool {
    own.iter().any(|p| text.contains(&own_name(p)))
}

/// The Claude Code rule of each pattern for tool: Bash(kubectl apply:*).
fn rules(tool: &str, patterns: &[String]) -> Vec<String> {
    patterns.iter().map(|p| format!("{tool}({p})")).collect()
}

/// Whether a permission prompt names a secret, from what it says (its title and its input's command, path, url or
/// pattern), own being Remit's own secret files (`own_secret_files`): it is refused before anyone is asked.
pub fn refuses_secret(title: &str, input: &Map<String, Value>, own: &[String]) -> bool {
    let mut said = vec![title];
    for k in ["command", "cmd", "file_path", "path", "url", "pattern"] {
        if let Some(v) = input.get(k).and_then(Value::as_str) {
            said.push(v);
        }
    }
    let text = said.join(" ");
    paths_in(&text).any(secret_path) || names_own_secret(&text, own)
}

/// The beginnings of the names of the aws operations that write, in every service: an operation is named verb-noun
/// (put-object), and these verbs change what they name; invoke and publish write in every form (invoke-async,
/// publish-version).
const AWS_WRITE_OPS: [&str; 36] = [
    "put-",
    "delete-",
    "create-",
    "update-",
    "send-",
    "start-",
    "stop-",
    "terminate-",
    "run-",
    "modify-",
    "reboot-",
    "attach-",
    "detach-",
    "purge-",
    "restore-",
    "copy-",
    "upload-",
    "import-",
    "register-",
    "deregister-",
    "enable-",
    "disable-",
    "associate-",
    "disassociate-",
    "authorize-",
    "revoke-",
    "reset-",
    "replace-",
    "rotate-",
    "cancel-",
    "add-",
    "remove-",
    "tag-",
    "untag-",
    "invoke",
    "publish",
];

/// The aws s3 commands that write whatever their arguments; cp and sync write when they copy to s3://.
const S3_WRITES: [&str; 5] = ["rm", "rb", "mb", "mv", "website"];

/// The ask rules on aws writes, made from what writes, written with and without global options before the service:
/// every write but a start-, since the start- a chain runs most is a Logs Insights query, a read the Chair must not be
/// asked about each time.
fn aws_ask_rules() -> Vec<String> {
    let mut out: Vec<String> = AWS_WRITE_OPS
        .iter()
        .filter(|op| **op != "start-")
        .map(|op| format!("aws * {op}*"))
        .collect();
    for op in S3_WRITES {
        out.push(format!("aws s3 {op}:*"));
        out.push(format!("aws * s3 {op} *"));
    }
    for op in ["cp", "sync"] {
        out.push(format!("aws s3 {op} * s3://*"));
        out.push(format!("aws * s3 {op} * s3://*"));
    }
    out
}

/// The gh commands that write, as noun and verb: `gh pr merge`, `gh issue comment`. A rule names each pair, since a
/// pattern such as `gh * create*` would also ask about `gh pr view --json createdAt`, a read.
const GH_WRITE_NOUNS: [&str; 7] = ["pr", "issue", "release", "repo", "workflow", "run", "label"];
const GH_WRITE_VERBS: [&str; 14] = [
    "create",
    "merge",
    "close",
    "reopen",
    "edit",
    "delete",
    "comment",
    "review",
    "ready",
    "rerun",
    "cancel",
    "run",
    "upload",
    "update-branch",
];

/// The ask rules on gh commands that write, but the ones the deny rules refuse before anything is asked.
fn gh_ask_rules() -> Vec<String> {
    let deny = deny_tools();
    GH_WRITE_NOUNS
        .iter()
        .flat_map(|noun| GH_WRITE_VERBS.iter().map(move |verb| format!("gh {noun} {verb}:*")))
        .filter(|rule| !deny.contains(&format!("Bash({rule})")))
        .collect()
}
