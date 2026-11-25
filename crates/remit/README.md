# remit (binary)

The `remit` binary (ADR-0072 decision 4): the core, which serves HTTP on TCP and the link (WebTransport, `/link`) on UDP at
the same `-addr`; the AI runtime (`remit runtime`, its child, ADR-0058); the jail's helper (`jail::init` runs first);
`remit emit` and `remit export`. The bundled plugins run in-process (decision 7).

## Shape

| Module | What it does |
|---|---|
| `main.rs` | subcommands, the environment check, signals (first stops in order, second exits), the log (JSON on stderr) |
| `dash_flags.rs` | the command line's flags (`-name value`, `-name=value`, bools, durations as `1h30m`) |
| `flags.rs` | defaults, absolute paths, what no session may read or change |
| `config.rs` | `orch::Config` and `ServerConfig`, the MCP URL, the address check |
| `serve.rs` | store, orchestrator and its ports, HTTP routes, shutdown order |
| `channels.rs` | the link's listener: each session to `rpc` or `runtimelink` by the contract its hello asks for |
| `agentmcp.rs` | `/mcp`, the agents' door, by run or seat token |
| `ports.rs` | orch's ports over the runtime's link and OpenRouter |
| `runtime.rs` | `remit runtime`, and how the core starts, waits for and kills it |
| `agent_home.rs` | the agents' Claude Code config dir |
| `memory.rs` | imports what older Remits kept of memory, once |
| `rotate.rs` | `agents.log`, rotated at 64 MiB |
| `health.rs` | `/healthz`'s verdict, the build stamp (`build.rs`) |
| `identity.rs` | the node key (`<data>/node.key`) and the start's self-signed certificate (ADR-0072 decisions 3, 12) |
| `web.rs` | `web/dist` embedded; every other path is `index.html` |
| `emit.rs`, `export.rs` | the two client subcommands |

## HTTP routes (TCP, `-addr`)

`/healthz`, `/link.json` (`{url, certificateHashes}`: where a page opens its link session and the hash to trust), `/mcp`
(agents), `/mcp/client` (`clientmcp`), `POST /hooks/{name}` (`door`), and the web app for everything else.

## Tests

`tests/binary.rs` starts the built binary on a temp data dir with a stand-in agent command: `/healthz` turns healthy (the
runtime connected, the scheduler ticked), `/link.json` names the link, a session with the owner key is welcomed, a forged one
refused, a session with an unknown mentor code is refused, an app route serves the page, `/mcp` refuses a request
without a token, and SIGTERM ends it with status 0.
