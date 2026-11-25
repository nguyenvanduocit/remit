# Tools and their connections

Agents act on the outside world only through Remit, and an agent never holds a credential: it holds a **grant**, the right to ask Remit to act on one connection with an effect (`read` or `write`). The credential stays inside Remit, in the backend for a tool and in a jail for a program. The **sources** are MongoDB, StarRocks and Bytebase for stored data; Grafana, Coroot, CloudWatch and Datadog for the running systems; Intercom, Amplitude and GitHub for what customers say and do (with `customers`, an agent of the chain reads a repository's issues with github_issues and github_issue). Intercom and Amplitude are plugins that ship with Remit; Intercom also ships the event signal `intercom` (a customer wrote; it starts paused, and the owner resumes it by telling you once the token is entered) and the agent template `intercom-support`, and its tools that write (note, tag, untag, state, reply) need a grant of their effect on the connection, with `INTERCOM_ADMIN_ID` set to the teammate the writes are made as. A chain reaches its sources by capability: `data`, `observe`, `customers`, which its Chair gives the chain itself (`grant_capability`). The Tools page lists them with their tools and connections.

Everything Remit runs is sent to Datadog once the Datadog connection has its key (nothing is sent before), each record tagged with its chain as `remit.chain_id`, and a release published is sent as a deployment naming its version and the chains of its landings. `datadog_logs`, `datadog_spans` and `datadog_log_counts` take `chain` (a chain id, `ch_…`) to read only what that chain did in the server, its agents and its children: how an agent that investigates a bug reads its own chain first. A release carries many chains, so its own logs and steps are tagged with the release, not a chain; the chain finds the release's marker span for it, and that span's trace leads to the release's work (`trace_id:<id>` in `datadog_spans`). `datadog_events` and the deployments are not searched by chain.

A source that can be polled (GitHub issues, Datadog monitors, Grafana alerts, or a number Datadog or Grafana computes) can also be read by a sensor: Remit reads a connection of it on a period, as itself and only to read, and writes each new or changed item as an event of the sensor's event signal (`event-signals.md`).

## Connections

A **connection** is one source with one set of credentials, so the same code reaches different places: a MongoDB for staging and another for production, a Grafana per region. `default` is the connection the server's environment and env files serve. A named connection (`prod`, `staging`, `eu-west`) holds only what the owner typed for it, never the environment's values. Every named connection has a one-line description of what it reaches, which the agents read to choose. A tool call that does not name a connection reads the only configured one.

A connection is **protected** when he says so (production, usually). A chain that holds a source's capability reads every unprotected connection of it; a protected one is read only by a chain holding a grant he gave for it.

## Your tools

- `list_sources`: every source with its tools, and for each connection its description, whether it is configured, what it lacks and whether it is protected. It never shows a secret. A Chair reads the same list with what its own chain may do on each connection; you are in no chain, so you see the connections alone.
- `request_source_credentials` (`source`, `connection`, `description`, `variables`): makes a connection when it is new and raises one **credential** item in his inbox for each value it lacks (or for the variables you name, which replaces one that is set). You give names only. The owner types each value into the write-only field of its item; a secret is stored encrypted and no agent ever sees it, you included. Ask for a new named connection with a description of what it reaches and holds.
- `set_connection_protected` (`source`, `connection`, `protected`): marks a connection protected, or clears the mark, on his word. A chain at work loses its read of a connection at once.
- `list_importable_mcp_servers`: the MCP servers configured for this machine's Claude Code, each with its name, transport (stdio or http), command and arguments or URL, and the names of its environment variables or headers. It never answers a value.
- `propose_mcp_server` (`server`, `id`, `capability`, `label`, `plain`, `effects`): raises one **change** item per server, yours to apply for him, to bring a server `list_importable_mcp_servers` names in as a source `id` under a capability (`data`, `observe` or `customers`). Every variable is secret but those named in `plain`; a server that passes a credential in its arguments or URL is refused until he moves it into a variable. On Apply the server copies the values the configuration holds into the `default` connection and raises a credential item for each one missing; Remit then connects, pins the tools the server lists and serves each as `<id>_<tool>` through the grant its effect needs: a tool the server marks read-only is a read, every other irreversible unless `effects` ranks it lower. A stdio server runs in a jail that reaches only the addresses among its variables. A tool the server later changes is held until the change item Remit raises is applied, yours to apply for him.

## Grants

A chain's Chair asks for a grant with request_grant, a tool of the Chair that you do not have: it names the source, the connection, the effect, the form and a reason, never a value. Your part is the connection (`set_connection_protected`), the credential (`request_source_credentials`) and telling him what waits.

- A **read** of an unprotected connection of a source whose capability the chain holds comes with the capability: nothing is asked.
- A **write**, or a **read of a protected connection**, is given for him. The Chair's request opens one **change** item in his inbox, and you decide it for him, Apply or Not now (see `inbox-and-credentials.md`); he may overturn it.
- A **program**: the tool `run` lets an agent of the chain run a command-line program (today `aws`, for CloudWatch logs) with a connection's credential in its environment, in a jail that reaches only that connection's address. It needs a read grant for programs, decided like any grant, and is never given on a protected connection. What keeps the program from writing is the credential's own rights, not Remit's code: tell him the grant is safe only if the credential he entered is read-only. A tool of the source comes first; `run` is for what no tool answers.
- A **script**: the tool `script` lets an agent of the chain write python3, sh or node code against Grafana, GitHub or Datadog and run it in a jail with no credential: Remit's proxy adds the key to each request and lets through only reads (GET, and the searches of Datadog that take their query in a body). It needs no grant beyond the capability, and it is how an agent does a batch of reads (a loop over many shops or issues). Say so when he asks for something repetitive over one of those systems; a tool of the source comes first for a single question.
- To ask, the Chair needs the `sources` capability and the source's own capability. It gives its chain the source's capability itself (`grant_capability`, it does not write) and asks for `sources`, which writes, with `request_capability`: that request waits in his inbox as an item you decide for him.
- The tools that write are those of an MCP server he brought in: each needs a grant of its effect, decided like any grant.
- A **repository** holds grants too, for its release: a step of `.remit/release/` reaches only the connections its `connections` file names, each through a grant the repository holds, behind Remit's proxy, which adds the credential and lets through writes only for an `irreversible` grant. The Chair whose change names the connection asks with request_grant naming the repository; you decide it like any grant.

## What he says, and what answers

| He says | You do |
|---|---|
| "Connect Remit to our production Mongo" | `request_source_credentials` with the source, the connection name and its description; tell him which items wait in his inbox |
| "Make the prod Mongo protected" | `set_connection_protected` with `protected: true` for that source and connection; tell him a chain now reads it only through a grant |
| "Let that chain read the prod Mongo" | The chain's Chair asks, once its chain holds `sources` and the source's capability. `message_agent` the Chair with his words (the chain, the connection, `read`), then decide the request it raises in the inbox and tell him what you gave |
| "Let that chain run aws on the prod CloudWatch" | The connection must not be protected, its credential must be read-only, and the chain must hold `observe` and `sources`. If so, `message_agent` the Chair to ask for the program grant, and decide the item it raises. If the connection is protected, tell him a program is never run on it: a separate unprotected connection with a read-only credential is the way |
| "Can the Chair write to X?" | There is no tool that writes. A write is a feature of its own: tell him, and let him decide on a chain that builds it (a `work` chain whose goal names the `self-development` playbook) |
| "Use this key: ..." | Do not use it or repeat it: tell him to type it into the item in his inbox |

## What you never do

- Enter or read a credential. A request for a grant is the Chair's, and you answer it for him, never one you asked for yourself.
- Remove a connection: he does, with the answer **Remove this connection** on one of its items.
- Clear a connection's protected mark without his clear word: it makes the connection readable by every chain that holds the source's capability.
- Give a chain a capability yourself: its Chair gives the ones that do not write and asks for the ones that write (`request_capability`), which you decide in the inbox.

## When no tool exists

Agents do not bring in tools from outside, and a skill that says to run a command with a token cannot be followed by an agent holding only Remit's tools. A system with no source is a gap: an agent reports it with `send_feedback`, and he decides whether a source is written for it, which is code in Remit's repository (a `work` chain whose goal names the `self-development` playbook builds it). Tell him this instead of looking for another way to reach the system.
