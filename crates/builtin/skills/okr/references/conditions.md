# Writing the condition

A condition is an expression that gives true or false, on what the signal said. The dry run runs it on the newest reading, so write it on a reading you saw.

## What it can read

- `value`: the reading's JSON. A sensor's reading is read as `value.field`, `value.a.b`.
- `body` and `header`: for an event signal, the JSON of the event's body and the event's headers. `body.plays >= 3`. A body that is no JSON, or was cut, has no fields.
- `now`: the time of the check. It cannot be the whole condition: a condition that reads no `value` or `body` holds whatever the signal says and is refused.

## Patterns

| Measure | Condition |
|---|---|
| a count reaches a target | `value.solves >= 10` |
| a count stays at zero | `value.extra_requests == 0` |
| a rate stays under a bound | `value.errors / value.requests < 0.02` |
| a size stays under a bound | `value.gzip_kb < 100` |
| an event carries a field | `body.action == "opened" && body.issue.number > 0` |
| a text contains something | `value.status.contains("success")` |
| a value is one of several | `value.state in ["live", "healthy"]` |
| a field exists before it is compared | `has(value.p95) && value.p95 < 300` |

A division by zero gives no number and the condition reads false, so a ratio is guarded: `value.requests > 0 && value.errors / value.requests < 0.02`. `has(field)` is for a field that is sometimes absent; it also hides a typo (a misspelt field inside `has` reads as not met, not as an error), so write it only for a field you saw absent in a reading.

## What the dry run refuses

- an empty condition, one that does not compile, or one that gives a number or text;
- a condition that reads neither `value` nor `body` (a constant such as `1 < 2`, or only `now`);
- a field the reading lacks (`value.plays` when the reading has `value.solves`): a typo is an error, not "not met";
- a guardrail that the reading already fails, and a key result that can only be met by breaking a guardrail on the same signal.

A key result that is not met today is fine: its condition ran. A condition that cannot run is the problem.
