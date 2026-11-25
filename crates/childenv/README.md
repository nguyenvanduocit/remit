# childenv

Decides what a child process inherits of the server's environment when an agent can steer what that process does (ADR-0018,
invariant 1): `of` keeps the names a child needs and adds the caller's own, `secret_names` lists the names that look like
credentials. Pure: entries are `NAME=value` strings, as the process environment holds them.

`trace` tells a child the work it runs for (ADR-0119): `TRACEPARENT`, and the work's attributes appended, escaped, to the
job's own `OTEL_RESOURCE_ATTRIBUTES`; its service name stays the job's. Ids only, never a credential; adding a name here is a rail change
that needs a `security` verdict (ADR-0119, *Security verdict*).
