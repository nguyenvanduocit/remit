# 0119. Everything Remit runs is traced and logged to Datadog, and every record names its chain

Status: accepted, 2026-11-19; D1 built (`crates/datadog`); D2 built 2026-11-20 (*As built: D2*, below); D3 built (*As built: D3*). Source: the owner, 2026-11-19 (translated): "integrate fully and comprehensively
with Datadog, so we can follow bugs and log everything of it on every system and every platform; but how does it follow
the right chain, or have a trace?". Extends ADR-0085 (the agents' telemetry), ADR-0095 (a page's link report), ADR-0114
(a landing's trailers), ADR-0116 (a release) and ADR-0018 (what a child inherits).

## Context

What Remit observes of itself today, read on the owner's instance on 2026-11-19:

- **The agents.** Claude Code sessions export OTLP to Remit's `/otlp` door, and the card is named by the run's token,
  never by the payload (`crates/remit/src/otlp.rs`, `crates/agent/src/options.rs:85-104`). The records are kept in
  `telemetry.db`: 21,060 of them over 2.5 days from 22 cards, including `claude_code.cost.usage` adding to $225.63. Each
  card's spans carry their own trace ids, not tied to anything of Remit's.
- **The server.** `tracing` writes JSON lines to `~/.remit/remit.log` (`crates/remit/src/main.rs:146-154`). A chain step
  is a span with `chain` and `step` fields. There are 1,794 WARN or ERROR lines, and nothing reads them. There are no
  metrics, no exported traces and no alerts.
- **A page.** Its link report becomes wide events in `telemetry.db` (`crates/rpc/src/link_report.rs`). A page's errors and
  slowness are not reported.
- **Children** (the checks and release steps in the jail, plugins, sensors' scripts, git) report nothing, and
  `crates/childenv` gives them only the names they need.
- **Products.** A landed commit names its chain (`Remit-Chain:` trailer, `crates/shipping/src/landing.rs:206`). An error in
  production does not lead back to it.

Datadog takes OTLP directly, with no agent: `/v1/logs`, `/v1/metrics` and `/v1/traces` on the site's OTLP intake
(`https://otlp.<site>`, the metrics page's example `https://otlp.datadoghq.com/v1/metrics`), with a `dd-api-key` header,
over `http/protobuf`, up to 5.1 MiB per logs or metrics request and 15 MiB per traces request, a larger one refused with
413 (`docs.datadoghq.com/opentelemetry/setup/intake_endpoint/`). Traces get APM's trace metrics only with the header
`compute_stats: true`; metrics must be delta, a cumulative one is refused; no `dd-protocol` or `dd-otlp-source` header is
asked for (`docs.datadoghq.com/opentelemetry/setup/otlp_ingest/{traces,metrics,logs}.md`, all read 2026-11-20). Datadog
recommends its agent or a Collector for production, for enrichment and sampling. Remit sits where a Collector would.

The owner's question is the heart of this: **how does a log, an error or a span lead to the right chain?** A chain lives
for hours or days, and a trace that long is unreadable. A chain is not a trace. It is a key every record carries.

## Options

1. **Install the Datadog Agent and its tracer libraries on each host and in each child.** This is the most "native"
   option. But it puts a Datadog key in every process an agent steers (the credential rail), splits Remit's own records
   from what goes out, and leaves the correlation to each process.
2. **Remit is the Collector.** Every process exports OTLP to Remit, as Claude Code does today. Remit stamps every record
   with the chain it belongs to (it knows that from the token, never from the payload), keeps what it keeps today, and
   forwards everything to Datadog with the one key only it holds. **Chosen.**
3. **Run an OpenTelemetry Collector beside Remit.** It is a standard part. But it knows nothing about chains: the stamping
   would still be Remit's, and it would be one more process to run on every platform. Kept as a seam (below).

## Decision

### One key in every record: the chain's

- **Correlation attributes.** Every log, span and metric about a chain carries these as Datadog tags:
  - always `remit.chain_id`;
  - with them when known: `remit.card_id`, `remit.session_id`, `remit.turn`, `remit.key_result`, `remit.landing_id`,
    `remit.release_id`, `remit.chain_kind`.
  - Every record also carries the resource tags `service` (`remit`, `remit-agent`, `remit-web`), `env`
    (`-datadog-env`, default the host's name) and `version` (the release tag `<yyyymmdd>-<sha>`): Datadog's unified
    service tagging.
- **What a trace is.** A trace is one unit of work, never a chain. The units are:
  - a turn of a card: root span `remit.turn`;
  - a step of a chain: `remit.step`;
  - a scheduler Tick;
  - an RPC;
  - a landing's batch;
  - a release.

  Finding a chain is a search on `@remit.chain_id`. A chain's screen links to it in Datadog (APM traces, logs and errors,
  each filtered to that chain), so the owner jumps from where he looks at the agent.
- **Agents join the turn's trace.** The `/otlp` door already knows the card from the token. It also knows the turns the
  card ran, and so their `remit.turn` spans. It places each trace a session exports once, by its earliest span, in the
  turn that span began in, and every later span and log of that trace goes where it went, whatever its own clock says.
  The trace becomes the turn's and its root spans the children of the turn's span. A trace that began in no turn goes in
  a trace of the card's own, derived from the card and its id, so a payload's ids never reach a trace that is not its
  card's. Every attribute under `remit`, at any depth (Datadog flattens a map `remit: {chain_id}` to `remit.chain_id`),
  is dropped and the token's stamped. This does not depend on Claude Code propagating context, and a payload cannot claim
  another chain. The door sends a request on only once `telemetry.db` kept it, so a session's retry is not sent twice.
- **The server's logs join their span.** The `tracing` subscriber gains an OpenTelemetry layer. Every event inside a span
  carries its `trace_id` and `span_id`, so Datadog shows a span's logs and a log's trace. The chain attributes come from
  the span's fields (`chain`, `card`), set once where the span opens.
- **Errors are errors.** An ERROR event is exported with `error.kind` and `error.message`, so Datadog Error Tracking
  groups it as an issue tagged with its chain. It carries no `error.stack`: the stack a log line can take is where it was
  logged, not where it failed, and taking one costs every error its time on the worker that logged it.
- **Children join through Remit.** A child an agent steers is given `TRACEPARENT`, `OTEL_RESOURCE_ATTRIBUTES` with its
  chain's attributes, and an OTLP endpoint on Remit's door with its run's token. It is never given the Datadog key, which
  only the server holds. A new name for `childenv` is a rail change and needs a `security` verdict.
- **A page reports through its link.** Its errors (uncaught, unhandled rejections, failed RPCs) and its timings (load, the
  link's frames and delays, as today) go up the link as one more report. They are tagged with the chain it shows and the
  owner's session, and leave as `service:remit-web`. There is no third-party script in the page and no client token, and
  the page holds no state (Server-driven).
- **Products lead back.** A release (ADR-0116) sends a Datadog deployment event naming its version and the chains of its
  landings. An error in a product that runs that version then leads to the landings and their chains (Datadog's suspect
  commits read the `Remit-Chain:` trailer).

### Remit forwards, and keeps what it keeps

- **One exporter in the server.** It takes the records already stamped (the agents', the server's, the children's, the
  pages') and sends them to the site's intake over `http/protobuf`, batched under the limits and retried with backoff.
  It is fed by a queue bounded in records and in bytes that drops the oldest when Datadog is unreachable, and counts
  what it dropped. It never blocks the work. A request is measured as an upper bound of its encoded size, so none passes
  the intake's limit. At stop it sends what waits once more, each request tried once, and ends a few seconds after stop
  whatever is left. It is built on `opentelemetry-proto` and an HTTP client, not on `opentelemetry-otlp`: the SDK's batch
  processor drops the newest when full and holds its headers from the start, and the agents' records arrive as OTLP
  JSON that is forwarded, not re-made as SDK spans.
- **`telemetry.db` stays the one source Outcomes and the card's screen read.** Datadog is a copy for searching, alerting
  and comparing hosts. Remit never reads its own state back from Datadog.
- **The key is a credential.** It is the Datadog source's own key, `DATADOG_API_KEY` on its default connection, which
  the owner types into a write-only field of an inbox item; no agent sees it. The site is that connection's
  `DATADOG_SITE` (`datadoghq.com` when empty, read by the source's own rule, `source::datadog_site`). One account has one
  owner: the source that reads Datadog and the exporter that writes to it use the same key and site, read at each send,
  so a key entered or replaced is used at once. `-datadog-env` names the env every record is tagged with (the host's
  name when empty). No key means nothing is kept or exported, and nothing else changes; a chain's screen links to
  Datadog only while what is sent goes through.
- **Nothing that leaves carries a credential.** Every record is redacted by the sources' rules before it is sent: the
  value of an attribute whose key names a credential is removed (`sensitive_key`), and every text is cleaned of token
  shapes and addresses (`clean_text`). The exporter redacts each record as it lays it out, the door the resource and
  scope a session brings (`crates/datadog/src/redact.rs`).
- **Every signal, no sampling.** The cost is not capped (Where we are). The exporter is the seam where head or tail
  sampling would go.

### Datadog speaks back

- **A Datadog monitor that fires posts to Remit's event door** (`event:datadog`, signed as event signals are). A
  subscription the Assistant writes turns it into a chain that investigates. The chain's tags name the chain the error
  came from, so the investigation starts with its trace.
- **Agents read Datadog through the source they already have** (`crates/source/src/datadog`), now able to search by
  `remit.chain_id`. An agent investigating a bug reads its own chain's logs and traces.
- **Remit's own errors become a fact** (`fact:remit.error`), grouped by Remit's own fingerprint (where it was logged and
  its message with the numbers taken out) so one issue raises one, not one per line, with Datadog or without. A shipped
  subscription gives it to a chain that investigates. That chain's origin is `event`: the error's text is what the server
  logged, often quoting a system outside Remit, so its Chair reads it as evidence, never as the owner's words. One ERROR
  is not an issue (`errors::client_left`, added 2026-11-20 after it raised two): rmcp's `channel closed`, logged at
  every session end for the answer to a client of `/mcp` that had closed its connection. Nothing in the server failed.

### Built in slices, each whole

- **D1:** the attributes; the server's OpenTelemetry layer with logs joined to spans; the exporter and the key; the turn
  re-parenting at `/otlp`; the chain screen's links; `fact:remit.error`.
- **D2:** the children (`childenv` with a `security` verdict) and the page's error report.
- **D3:** the release's deployment events, `event:datadog` from monitors, and the Datadog source's search by chain.

### As built: D3

- **The deployment event.** A release that published (version set, installed here when it is this server's repository, before
  its bake) is told to the exporter through shipping's port `release_run::Deployments`, filled in
  `crates/remit/src/datadog_ports.rs`. Shipping chooses what the release carries of the chains' own work
  (`release::own_of`: its landings but the reverts, and their chains), the one rule that also says whom it tells, marks
  live and traces; the port receives that and the time, and re-derives nothing. The exporter sends it with the records, from a
  bounded list that counts what it drops (`Datadog::deployed`, `crates/datadog/src/deploy.rs`), as Datadog's deployment event,
  `POST https://api.<site>/api/v2/dora/deployment` with `DD-API-KEY`: `id` (the release's), `service`, `version` (left out
  while empty), `started_at` and `finished_at` in epoch nanoseconds (its steps' first start and last end), `git.repository_url`
  and `git.commit_sha`, and `custom_tags` `remit.release_id`, then each `remit.chain_id`, then each `remit.landing_id`
  (docs.datadoghq.com/dora_metrics/setup/deployments, read 2026-11-20). The page gives no limit on tags: Remit sends at most
  100, the most the events API takes, and cuts the landings first. The v2 events API was read first
  (docs.datadoghq.com/api/latest/events/post-an-event.md, read 2026-11-20): its `change` category takes only `feature_flag`
  and `configuration` as the changed resource, so a deployment is not expressible there. The same retry and the same key and
  site as the records (`export::post`). With no key nothing is kept; an account that cannot be read keeps what waits for the
  next send. The deployments go to the site's API host, not the intake: a Collector in the intake's place (see *Revisit
  when*) has no deployment endpoint, so a test's sink overrides the two hosts separately (`Inner.api`, `Inner.intake`).
- **A release is traced as the work it does.** The run of its procedure, its going live and its taking back are each a trace
  of its own, rooted at `remit.release` (`remit.release_id`, `repo`, `action`, `version`) with no parent and a link
  (`follows_from`, exported as a span link) to the span it was started from, in `crates/shipping/src/release_trace.rs`.
  The brief asked for span links to each landing's chain. A link needs the span it points at, and a chain, which lives for
  days, has none; so the release names its chains by a child span `remit.release.landing` for each landing it carries,
  tagged `remit.chain_id` and `remit.landing_id`. `@remit.chain_id:ch_x` finds that marker span, whose trace is the release's.
  The release's own logs, its steps and their `remit.child` spans carry `remit.release_id` and no chain: a release is of
  many, so a search on a chain finds the release through its marker and goes on by `trace_id`.
- **`event:datadog`.** Needs no new door and no new rail. Datadog's webhook cannot sign (docs.datadoghq.com/integrations/webhooks,
  read 2026-11-20: custom headers in JSON, with variables, HTTP basic in the URL, OAuth bearer; no HMAC), and the door
  already takes the key a sender can only set as a bearer token or a token header (`presented_key`,
  `crates/door/src/lib.rs`), the same key and the same pause as every event signal. The manual
  (`references/event-signals.md`) has the Assistant set the key in a hidden custom header and never in the address
  (`?token=`, which the door also takes, would put the key where Datadog shows the webhook's URL). Datadog sends no
  delivery id, so a retry is the same event only by its identical body within the door's ten minutes
  (`dedupes_of`); the payload carries `$ID` so two alerts are not taken for one. The subscription that starts the
  investigating chain is the Assistant's, from a real event.
- **Search by chain.** `datadog_logs`, `datadog_spans` and `datadog_log_counts` take `chain` (`ch_…`), which narrows the search
  to `@remit.chain_id:<chain> (<the search>)`, so an OR inside the agent's search stays inside the chain; an id with any
  other character is refused (`source::datadog::of_chain`). `datadog_events` has its own input and takes no chain, and the
  deployments are not searched by chain.
- **Assumptions.** The service of a deployment is its repository's project name, and no `env` is sent: Remit does not know
  where a procedure publishes to. The organisation has Datadog's DORA Metrics on; if not, Datadog refuses the event, the
  exporter says so once a minute at most and lets it go. That Datadog takes a second send of one `id` as one deployment is
  not in the page read: a deployment that went out twice (a restart between its send and its record) may show twice.

## Consequences

- From a chain's screen the owner opens everything that chain did, in every process, in one search. From any error in
  Datadog, one tag leads to the chain, and from there to its card, its turn and its landing.
- What runs on any host (macOS, Linux) and in any page reports the same way, because the one door and the one exporter
  are Remit's.
- Data leaves for Datadog: tool details, paths, error messages, and the names of chains and cards. Prompt and answer
  texts stay out, as today (`OTEL_LOG_USER_PROMPTS` is not set). Credentials are redacted before export by the sources' own rules (Decision).
- One more dependency on the outside. Losing it changes nothing in Remit but the copy.

## As built: D2

What was built, where the build differs from the decision above, and the seams left.

- **A jailed child runs in a span of its own and is told it, and exports nothing yet.** Every child Remit runs in the jail
  for a piece of work (a repository's checks, `crates/shipping/src/repochecks.rs`; a release's steps, `release_run.rs`; a
  source's programs and scripts, a sensor's included, `JailRunner` in `crates/reach/src/program.rs`) runs in a span
  `remit.child` (`telemetry::Child`), under the span of the work that started it. The span records how the child ended
  (`exit_code`, `timed_out`) and is an error when it exited non-zero, ran out of time or could not be started. The child
  is given what `childenv::trace` makes of that span: `TRACEPARENT` (while the server traces, in place of any the job set)
  and the correlation attributes of `attr::ALL` its spans set, appended to the job's own `OTEL_RESOURCE_ATTRIBUTES`, so
  Remit's names are the ones an SDK reads (the last of a name wins) and the job's stay. Its service name is not set: a check
  or a release step may run the customer's product, whose telemetry keeps its own name. It is **not** given an OTLP
  endpoint or a token, unlike the decision: a check has no network, and a step or a script reaches only its gate or its
  proxy, so no jailed child can reach the `/otlp` door, and an endpoint it cannot reach would be a setting that does nothing.
- **Which chain a child names.** A chain's checks outside its steps run in `remit.checks`, which names the chain (a submit's
  checks, `land.rs`; a finish's command check, `check_rail.rs`). Every source call runs in `remit.source`, which names the
  chain of its `Caller` (`source`'s gate): an agent's program or script and a chain's sensor script name their chain; Remit's
  own polls name none. A release's steps run in its `remit.release` trace (D3), which names the chains it
  carries by a child span for each landing, so a step's `remit.child` is in the trace a search on any of them finds. A landing batch's checks
  (`queue.rs`) name none, for the same reason.
- **Not told:** git in an agent's clone (`repo_command`), which exports nothing and runs under no span that names its chain;
  a plugin, which serves every chain; an agent's own shell, which inherits its Claude Code session's OTLP settings already
  (ADR-0085).
- **Where the span is read.** The correlation attributes and the layer that keeps them on each span (`Correlation`,
  `Correlate`) and the reading of a child's span (`Child`) are `telemetry`'s, beside the attribute names, so the crates
  that run children depend on no exporter. `Correlation` is kept on a `Registry`-based subscriber, the server's; the server
  checks that once where it assembles its subscriber (`telemetry::correlates`, in `remit`'s `logging`) and logs an error
  when it is not, and a span whose attributes cannot be read gives the child none.
- **A page's faults go up its link report** (`wire::app::LinkReport.faults`). The core counts what failed over the minute,
  by kind, message, place and the chain the tab showed (`crates/replica/src/tally.rs`): an error nothing caught and a
  rejection nothing handled, which the tab hands it (`Ask::Fault`, from `window`'s `error` and `unhandledrejection` in
  `web/src/core/client.ts`); and a call that failed (`FaultKind::Call`, its method as the place), but for the link being
  down, a call the page let go of, or a conflict the caller goes on from. A rejection that is a call's failure is not
  counted twice. The core sends at most `PAGE_FAULTS_MAX` (32) a minute, each text cut at `PAGE_FAULT_TEXT_MAX` (500).
- **The server trusts none of it** (`crates/rpc/src/link_report.rs`). It cuts the report to those bounds again, the page's
  name to 64 characters and each count to between 1 and 100,000 (`bounded`), then lets a session send at most 32 faults a
  minute of the server's clock, counted as they are taken, however the page names its reports (`FaultBudget`, per
  session), then decides the chain each fault names from its own records, never from what the page said it shows: the
  owner's session keeps a chain the store holds (`Orchestrator::owner_names`, a store read that loads no chain), a mentor's
  a chain one of his helps is on (`mentor_names`, his helps as the store holds them, read through his key, so a revoked key
  names none), and any other chain is dropped. Both entrances run that one sequence (`link_report::keep`, told the
  `Sender`). The faults are kept in `telemetry.db` as `remit.page.fault` and, the first
  time the report is kept there, sent to Datadog (`orch::DatadogExport::page_faults`, `datadog::Datadog::page_faults`) as
  ERROR logs of `service:remit-web` with `error.kind` (the contract's name of the kind), `error.message`, `remit.chain_id`,
  `remit.page.fault_count` (how often it happened in the minute) and `session.id` (the page's core, a random name of 32 hex
  digits; a page name of another shape is not sent). The core counts a minute's faults without their times, so a record's
  time is when the server took the report. Nothing names a person: who signed the session in is not sent. A
  server that keeps no telemetry cannot tell a report sent again, and sends none. The page's timings stay in `telemetry.db`
  as today; sending them to Datadog is not built.

### Security verdict: what a child is told (`childenv::trace`)

A new name for `childenv` is a rail change. Verdict by `rails-and-sandbox`, 2026-11-20: **accepted, bounded.**

- **What a jailed child can do now that it could not:** read two names: `TRACEPARENT` (a trace id and span id the
  server made for it) and `OTEL_RESOURCE_ATTRIBUTES` (the correlation attributes of its work, the chain's id, a card's, a
  release's, after the job's own). Its service name is left as the job set it, so the customer's product keeps its own
  telemetry's name. It can print them, or send them where it could already send: a release
  step to the connections its repository's grants name, a script to its source's upstream, a check nowhere.
- **Why that is bounded:** the values are ids, and no door of Remit's opens with an id: the owner key, a run's token, a
  client key or a worker key does. No token is given, so a child gains no way into `/otlp` or `/mcp`, and the Datadog key
  never leaves the server. A value cannot make an attribute of its own: each name and value is escaped
  (`urlescape::component`). What a child reports through Remit is stamped from Remit's own state, never from the payload
  (`forward.rs`); a child that reaches Datadog's own API (a script of the Datadog source, through its read-only proxy) is
  not Remit's to stamp, and reads there only. An id sent on to a granted upstream tells that system which chain asked, and
  nothing more.
- **The rail tests:** `childenv`'s `a_value_cannot_add_an_attribute_of_its_own` holds the exact entries a child is given,
  and a release step's environment is listed name by name (`orch` test
  `a_breached_release_runs_back_reverts_and_releases_forward_with_no_turn`): a name added to either fails its test until it
  is listed there and reviewed.

## Revisit when

- **Sampling or the drop rate matter** (Datadog's bill or the queue's counter): the exporter is the seam; tail sampling
  that keeps every error trace comes first.
- **More than one Remit host, or a Collector already runs where Remit does:** point the exporter at the Collector instead
  of the intake (`Inner.intake` in `crates/datadog`, the site's intake today, a local sink in its test). Nothing else
  changes: the deployments go to the site's API (`Inner.api`), which a Collector does not stand in for.
- **What may leave the machine:** the same `may_leave` seam as ADR-0118. Redaction beyond secrets (paths, customer data
  in tool details) waits for the owner's hardening, with `aio-threat-models`.
- **Claude Code propagates `TRACEPARENT` itself:** the re-parenting at `/otlp` becomes a check that the parent is the turn.
- **rmcp logs an answer whose client left below ERROR:** delete `errors::client_left` and its test.
- **A jailed child should export its own spans and logs** (a product's test instrumented with OpenTelemetry): the control not
  built is the way to the door. The jail relays a second loopback port to a unix socket made for that one child, which answers
  only OTLP and stamps what it takes from the socket, not from a token, so no credential enters the jail (`jail::Spec`
  gains it on both adapters, with `crates/jail/tests/jail.rs`). A child outside the jail that is given a token is given a
  pass only `/otlp` takes, naming its own run and revoked when the child ends, never the run's token, which `/mcp` also
  opens. The seam: `childenv::Trace` gains the endpoint, `/otlp`'s `export` resolves the pass beside `caller_for_token`.
- **The run's token in a session's environment** (`OTEL_EXPORTER_OTLP_HEADERS`, ADR-0085) is read by every process the
  agent's shell starts, and it opens `/mcp` as well as `/otlp`: an OTLP-only pass per run would narrow it. Found while building
  D2; left for the owner's hardening.
- **A page's or a mentor's client floods its faults** (the entrance is any signed-in session, the owner's or a mentor's
  device): the control built is a budget per session (32 faults a minute of the server's clock), and the bounds of each
  report. Not built: a budget per key across its sessions (a client that reconnects starts a new session, and a new
  budget), and sampling of faults at the exporter. Both go in `rpc::link_report` and the exporter's queue. A mentor's
  report reads his helps from the store once per fault that names a chain, at most 32 a minute; reading them once per
  report, or from his session's seat, is the change if that read shows up.
- **`event:datadog` is reachable by anyone who holds its key:** the door's key is the only control, as for every event signal. Not built: an allow-list of Datadog's source addresses, a rate limit on the door, and a Datadog payload schema the door checks before an event is kept.
- **A release is taken back, or an organisation has no DORA Metrics:** a `rollback` deployment status and a plain event as the fallback for an organisation without DORA Metrics are not built; `release_run::Deployments` is the seam.
- **A chain gets a span of its own** (a long-lived root): the release's marker spans become links to it.
- **A deployment sent twice** (a restart between its send and its record): Datadog's handling of a repeated `id` is not documented on the page read; a record of what was sent is the change if it shows twice.
