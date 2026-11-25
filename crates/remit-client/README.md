# remit-client

The native client of a Remit server (ADR-0072, decision 10). It is a binary over `link`, `wire` and `link-quic`. Every
command finds the server's link through `<address>/link.json` (`dial`), which says where the link is and which
certificate hash to trust; with no hash listed, it trusts the system's roots. `remit update` and `remit doctor` come in the
same way.

## `remit-client call|snapshot|watch`

The owner's contract (`wire::app`) from a shell, for a person or an agent that wants to call the server or watch it with
no browser in between:

```sh
remit-client snapshot                                   # the snapshot a channel starts from
remit-client call chain_get '{"id":"c_1"}'              # any call of wire::app, its args as one JSON object
remit-client call chain_pause '{"id":"c_1","why":"lunch"}' --request r_1   # a changing call; the same --request acts once
remit-client watch                                      # every push, one JSON line each, until the session ends
```

- `--server <address>` (default `http://127.0.0.1:4320`) and `--key-file <path>` (default `~/.remit/owner.key`): the
  owner key is read from a file, never from an argument, which every user of the machine could see.
- The calls are the contract's own: the method and args are decoded as `wire::app::Call` (`call::outgoing`), so a new call
  needs no code here. A changing call without `--request` gets a new one.
- An answer, the snapshot or each push is JSON on stdout. A failure is `{"code","message"}` on stderr with exit status 1:
  `INVALID_ARGUMENT` for a call or args the contract does not have, the server's code (`NOT_FOUND`, ...), `FORBIDDEN` when
  the key is refused, `DISCONNECTED` when the server cannot be reached or the watch's session ends.
