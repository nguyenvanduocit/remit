# Grafana Alerting and Prometheus Alertmanager: evaluation, state, notification routing, and lessons for the Remit signal pipeline

Scope: official docs as served at `grafana.com/docs/grafana/latest/` and `prometheus.io/docs/.../latest/` on 2026-11-05. The `latest` pages carry almost no version tags, so a behaviour is dated only where a source gives the version. Remit open problems are referenced as (a)-(g) from the brief: (a) silent failure or skip, (b) no start position, backfill or dry run, (c) retention ignores cursors, (d) "signal" mixes state and stream, (e) time split between clock sensors and windows with an implicit overlap policy, (f) a plugin processor runs under the global lock with no timeout, (g) AI agents author and debug subscriptions.

## Q1. Grafana alert rule evaluation: instance state machine, pending period, keep firing for, NoData/Error, multi-dimensional instances, evaluation groups

### Takeaway
A Grafana rule is evaluated per group interval. Each evaluation yields many *alert instances*, one per label set, and each instance runs its own state machine: Normal → Pending → Alerting → Recovering → Normal, plus NoData and Error. The two time filters are the pending period (debounce on entry) and keep firing for (debounce on exit). NoData and Error count as states, and the rule author chooses per rule what each one maps to. Every transition carries a `grafana_state_reason`.

### Cited Findings
- Three settings decide when a rule fires: the evaluation group ("how often the rule runs"), the pending period ("how long the condition must hold before firing") and keep firing for ("how long the alert keeps firing after the condition clears"). Every alert rule and every recording rule belongs to a group. Example intervals: 10s, 30s, 1m, 10m. — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)
- Pending period: a breached condition moves an instance from Normal to Pending, and to Alerting once the period has elapsed. It "ensures the condition breach is stable" before anyone is notified. A value of 0 skips Pending. — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)
- Keep firing for: when the condition clears, the instance goes from Alerting to Recovering and then to Normal (Resolved) after the period. If the condition returns during Recovering, the instance goes back to Alerting "without sending new notifications". The stated purpose is to avoid noise "caused by flapping conditions". 0 skips Recovering. — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/); [Grafana: Create Grafana-managed rule](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)
- Lifecycle states: Normal, Pending, Alerting, Recovering, No Data, Error. No Data and Error exist only for Grafana-managed rules. — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)
- Multi-dimensional: "One alert rule can generate multiple alert instances—one for each series or dimension produced by the rule's query." Instances of one rule can be in different states at the same moment. "Only Alerting and Normal (Resolved) instances are routed for notifications." — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)
- Editing a rule resets its instances to Normal. Edits to annotations, the evaluation interval or other internal fields are the exception. — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)
- No Data: "the query runs successfully but returns no data points". Error: the rule "fails to evaluate its query successfully", for example on timeouts or repeated query failures. Both honour the pending period (Normal → Pending → No Data/Error). — [Grafana: No Data and Error states](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)
- Configurable mapping per rule. No Data: Set No Data state (default), Set Alerting, Set Normal, or Keep last state. Error: Set Error state (default), Set Alerting, Set Normal, or Keep last state. When the mapping is Alerting or Normal, Grafana reuses the last known `Values` and puts `-1` for the measured value. — [Grafana: No Data and Error states](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)
- Keep last state "helps mitigate temporary data source issues" but "may not suit strict monitoring". For long outages the docs say to add a separate rule that detects them. — [Grafana: No Data and Error states](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)
- By default, No Data creates a separate `DatasourceNoData` alert instance and Error creates `DatasourceError`. Both carry the labels `alertname`, `datasource_uid` and `rulename` (the rule UID per the create-rule page). They are "independent of the original alert, so existing silences, mute timings, and notification policies may not apply to them." — [Grafana: State and health](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/state-and-health/); [Grafana: Create Grafana-managed rule](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)
- Evaluation timeout defaults to `30s` (`evaluation_timeout`) and attempts default to `3` (`max_attempts`). The settings are global, and changing them is "a last resort": a timeout set too long "can cause missed evaluations". — [Grafana: No Data and Error states](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)
- Stale instances: the query still returns data, but one series has been missing for "a number of consecutive evaluation intervals (2 by default)". The count is set by "Missing series evaluations to resolve". After that the instance goes to Normal with `grafana_state_reason=MissingSeries`, is notified as Resolved, and is then evicted from the UI. This is separate from No Data, which applies when the query returns no series at all. — [Grafana: Stale alert instances](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/stale-alert-instances/)
- `grafana_state_reason` annotation values: `No Data` or `Error` (the NoData/Error mapping set Normal), `Paused`, `RuleDeleted`, `Updated` (some rule edits), `MissingSeries`. — [Grafana: State and health](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/state-and-health/)
- Pausing a rule stops evaluation and creates no instances. The docs call this "different from mute timings", which block delivery while evaluation and instance creation continue. — [Grafana: Create Grafana-managed rule](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)
- Within a group: Grafana-managed rules "are evaluated at different times over the same evaluation interval but display the same evaluation timestamp" (concurrent). Data-source-managed rules are "evaluated sequentially, one after the other", so recording rules finish before alert rules. Grafana-managed rules can declare an explicit *rule sequence* (recording rules first, then alert rules). Rules in different groups may run at the same time. — [Grafana: Evaluation within a group](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/evaluation-within-a-group/)
- Prometheus equivalent: `for` ("Elements that are active, but not firing yet, are in the pending state"). `keep_firing_for` "tells Prometheus to keep this alert firing for the specified duration after the firing condition was last met". Without it, an alert deactivates "on the first evaluation where the condition is not met". Prometheus writes an `ALERTS{alertname, alertstate}` series for pending and firing alerts. — [Prometheus: Alerting rules](https://prometheus.io/docs/prometheus/latest/configuration/alerting_rules/)
- `keep_firing_for` arrived in Prometheus 2.42.0 to address flapping. — [docs commit](https://forger.sitiv.fr/apitech/prometheus/commit/3afcda82befc41b2a7834069593c2a6dc24a6e2c); [prometheus-users thread](https://groups.google.com/g/prometheus-users/c/nPmjW2gEvqg/m/EaD80NNJBgAJ) (secondary sources; release notes not read directly)
- Prometheus rule-engine limits: `--rules.max-concurrent-evals` default 4, "Global concurrency limit for independent rules that can run concurrently". `--rules.alert.resend-delay` default 1m. — [Prometheus CLI flags](https://prometheus.io/docs/prometheus/latest/command-line/prometheus/)

### Inferences
- **(d) state vs stream.** An alert instance is *state keyed by (rule, label set)* and derived from a stream of evaluations. Grafana keeps the input series (stream), the instance state (current value) and the transitions (history, Q2) as three separate things. For Remit: a subscription should hold one explicit state per key (Normal/Pending/Firing/Recovering/Error/NoData) beside its cursor. A firing is then a *transition*, not a raw match. `(subscription, key, revision)` maps onto Grafana's (rule, label set) instance. The missing piece is a state column.
- **(a) silent failure.** In Grafana an evaluation error is a *state* (Error) that is notified (`DatasourceError`), not only a log line. Remit's CEL errors, processor errors and timeouts could likewise put the subscription/key into an Error state that is visible and routable to an agent. The author would pick Error → {Error, Firing, Normal, KeepLast}.
- **(e) time.** Pending period and keep firing for are the debounce windows, expressed *in the rule* and measured against the evaluation clock. They are not a separate clock sensor. A cooldown/deadline window plays the same role. Grafana defines it as a state duration, which makes the overlap policy explicit (re-entry during Recovering does not re-notify).
- **(f) timeouts.** Grafana bounds each evaluation (30s, 3 attempts) and turns an exhausted budget into the Error state. A plugin processor needs the same: a deadline per call, with the result recorded as an error state, not a held lock.
- Editing a rule resets its state. This is a cheap idempotency choice that Remit could copy for subscription edits that change the condition (new revision → fresh state).

### Gaps
- The Recovering state / keep firing for on Grafana-managed rules was announced in May 2025 for self-managed Grafana v12.0.0 ([Grafana What's new](https://grafana.com/whats-new/2025-05-05-grafana-managed-alert-rule--recovering--state/), via search snippet). No later change to it was found.
- Slow evaluations are closed from Grafana source (the docs are silent). Each rule has its own goroutine fed by an unbuffered `evalCh`. When a new tick arrives while one is still waiting, the scheduler takes the waiting tick out and sends the newer one, so the newest tick wins and the older is dropped. It logs `"Tick dropped because alert rule evaluation is too slow"` and increments `grafana_alerting_schedule_rule_evaluations_missed_total{org,name,reason="slow_evaluation"}`. A second reason, `state_not_warmed`, covers results thrown away because the state cache is not loaded after a start ("Skip processing evaluation results because the state cache is not ready"). Sources: [schedule.go](https://github.com/grafana/grafana/blob/main/pkg/services/ngalert/schedule/schedule.go) (lines 438-446, main at b03e984d74, 2026-10-01); [alert_rule.go](https://github.com/grafana/grafana/blob/main/pkg/services/ngalert/schedule/alert_rule.go) (lines 206-222, 491-494); [metrics/scheduler.go](https://github.com/grafana/grafana/blob/main/pkg/services/ngalert/metrics/scheduler.go) (lines 179-186). Lesson for (e)/(f): the overlap policy for a recurring run is explicit (skip, keep the latest) and every skip is counted with a reason, never silent. Prometheus behaviour on missed iterations was not checked.

## Q2. State history: how transitions are recorded, and how users debug "why did/didn't this fire"

### Takeaway
Grafana writes one record per *state transition* (not per evaluation) to a pluggable history backend: Loki for the History UI, Prometheus as a `GRAFANA_ALERTS` series, or both. Each record holds the previous state, the new state, the labels and the query value at the transition. The history ignores silences and mute timings, so it shows what the rule decided apart from what was delivered.

### Cited Findings
- An event is logged "each time an alert instance changes its state". All events appear "regardless of whether silences or mute timings are set". — [Grafana: View alert state history](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/)
- The State history view shows when the change happened, the previous state, the current state, other instances that changed at the same time, and "the query value that triggered the change". Instances are identified by their unique label set. — [Grafana: View alert state history](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/)
- Limits: the History page has a "5000 alerts limit". The timeline defaults to the past hour. RBAC applies: "Users can only view the history of alert rules they have permission to access." On Grafana Cloud, "Analyze with Assistant" runs an LLM over the filtered events to find patterns and root causes. — [Grafana: View alert state history](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/)
- OSS and Enterprise must "configure alert state history in Loki" to get the History page. — [Grafana: View alert state history](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/)
- Backends under `[unified_alerting.state_history]` (`enabled`, `backend`):
  - Loki (`loki_remote_url`): powers the History views and Explore.
  - Prometheus: writes `GRAFANA_ALERTS{alertname, alertstate, grafana_alertstate, grafana_rule_uid, <labels>}` by remote write. Keys are `prometheus_target_datasource_uid` and `prometheus_metric_name`, plus `prometheus_write_timeout` (default 10s). It does *not* power the History views.
  - Multiple: `primary` plus `secondaries`.
  - The docs suggest a separate Loki instance for history.

  — [Grafana: Configure alert state history](https://grafana.com/docs/grafana/latest/alerting/set-up/configure-alert-state-history/)
- `grafana_state_reason` explains why an instance is in its state (MissingSeries, Paused, RuleDeleted, Updated, No Data, Error). — [Grafana: No Data and Error states](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)
- In single-node evaluation off (the default HA mode), every instance evaluates every rule, and "the duplication is visible in state history". — [Grafana: Configure high availability](https://grafana.com/docs/grafana/latest/alerting/set-up/configure-high-availability/)
- Prometheus has no transition log. It keeps the `ALERTS` series, which "is marked stale" when the alert is no longer active. — [Prometheus: Alerting rules](https://prometheus.io/docs/prometheus/latest/configuration/alerting_rules/)
- Alertmanager has an Event Recorder behind `--enable-feature=event-recorder`. It "captures significant Alertmanager events" and "fans them out to one or more destinations" ("file, webhook, Kafka, stdout"). Its events include process start and shutdown, alert lifecycle transitions, silence creation, notification delivery, and mute/inhibit suppressions. Without the flag it "silently discards all events". — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)

### Inferences
- **(a) silent skip.** Grafana's history explains *transitions*, but it cannot answer "why didn't it fire" when the condition simply stayed false: no transition, no record. The Grafana answer to "didn't fire" is the current instance list plus the query value. For Remit, where AI agents debug through tools (g), a per-subscription *evaluation trace* would close the gap. It would record, per reading considered, the reading seq, the CEL result (true/false/error), the reason for any skip (cooldown, outside window, key mismatch, dedup hit) and the resulting state. It can be bounded (last N per subscription), since transitions alone leave "didn't fire" unexplained.
- Keep the decision record apart from delivery (history ignores silences). Remit could record "condition fired" and "action delivered / suppressed by cooldown / failed" as separate facts, so an agent can tell which stage lost the event.
- A `reason` enum on every transition (Grafana's `grafana_state_reason`) is a cheap, machine-readable explanation that suits tool-using agents (g) better than free-text logs.

### Gaps
- State-history retention remains a documented gap. [Configure alert state history](https://grafana.com/docs/grafana/latest/alerting/set-up/configure-alert-state-history/) and [View alert state history](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/) both state no retention setting. Retention belongs to the Loki or Prometheus backend the operator runs, and the UI queries "up to 30 days". The exact Loki log-line schema was not found.
- No found doc covers the version history of the annotation backend (the old default) and its deprecation.

## Q3. Alertmanager: routing tree, grouping and timers, inhibition, silences, time intervals, deduplication, HA gossip and at-least-once

### Takeaway
Alertmanager is a delivery pipeline that sits after state. Alerts enter a label-matched routing tree. Each matched route batches alerts into aggregation groups (`group_by`), and three timers decide when a group is notified (`group_wait`, `group_interval`, `repeat_interval`). A fixed stage pipeline then filters (inhibit, time intervals, silences), dedupes against a replicated notification log, and sends with retry. HA uses gossip and is deliberately at-least-once ("fail open").

### Cited Findings
- **Routing tree.** Every alert enters at the root route, which must match all alerts. It then walks the child nodes in order. With `continue: false` (the default), matching stops at the first matching child. With `continue: true`, later siblings are also tried. If no child matches, the current node handles the alert. Children inherit `group_wait`, `group_interval` and `repeat_interval` unless they override them. — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)
- In Grafana the deepest matching policy wins ("only the deepest matching child policy handles the alert instance"). Matchers within a policy are ANDed (`=`, `!=`, `=~`, `!~`). The default policy matches everything, "so alerts aren't missed". Grafana can hold several independent trees. — [Grafana: Notification policies](https://grafana.com/docs/grafana/latest/alerting/fundamentals/notifications/notification-policies/)
- **Timers and defaults:**
  - `group_wait` 30s: the first notification for a new group waits so related alerts can join, and alerts resolved within the window are never notified.
  - `group_interval` 5m: the recurring timer for group changes. It is also the context timeout for each send.
  - `repeat_interval` 4h: re-sends an unchanged group. It is rounded up to a multiple of `group_interval`. If it exceeds `--data.retention`, the repeat fires at the end of retention.

  — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)
- Grafana uses the same defaults (30s / 5m / 4h) and a maximum repeat interval of 5 days, limited by `notification_log_retention`. Default grouping is `alertname` + `grafana_folder`. `...` gives one notification per alert, and empty puts everything in one group. Rationale: in a large outage, grouping is the difference between "1 phone call and 100 phone calls". — [Grafana: Group alert notifications](https://grafana.com/docs/grafana/latest/alerting/fundamentals/notifications/group-alert-notifications/)
- **Inhibition.** "suppressing notifications for certain alerts if certain other alerts are already firing" — [Alertmanager overview](https://prometheus.io/docs/alerting/latest/alertmanager/)
  - Fields: `source_matchers`, `target_matchers`, `equal` (labels that must have the same value on both).
  - A missing label and an empty label count as the same.
  - If every `equal` label is missing on both sides, the rule still applies.
  - An alert that matches both sides cannot be inhibited by alerts that also match both sides, itself included.

  — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)
- **Silences** are matcher-based and time-bounded. "If they do [match], no notifications will be sent out for that alert." They are created in the UI or API, not in config. — [Alertmanager overview](https://prometheus.io/docs/alerting/latest/alertmanager/)
- **Time intervals.** A named `time_interval` combines `times` (HH:MM, start inclusive, end exclusive), `weekdays`, `days_of_month` (negatives count from month end), `months`, `years` and `location` (IANA zone, default UTC). All set fields must match. Routes reference these through `mute_time_intervals` (no sends while active, but routing still stops or continues as usual) and `active_time_intervals` (send only while active). The root route can have neither. — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)
- **Notification pipeline (HA doc).**
  - Dispatcher: match routes, find or create aggregation groups, throttle by group_wait and group_interval.
  - Notifier stages, in order:
    1. Wait for gossip to settle.
    2. Filter inhibited alerts.
    3. Filter alerts outside an active time interval.
    4. Filter time-muted alerts.
    5. Filter silenced alerts.
    6. Wait by HA peer index.
    7. Dedupe by repeat interval and HA state against the notification log (`DedupStage`). A send is skipped unless firing alerts changed, resolved alerts changed (with `send_resolved`), or `repeat_interval` elapsed.
    8. Notify and retry.
    9. Update the notification log (replicated).

  — [Alertmanager: High availability](https://prometheus.io/docs/alerting/latest/high_availability/)
- **HA design.**
  - Gossip runs over HashiCorp Memberlist. It replicates silences, the notification log (nflog) and membership. State is "eventually consistent".
  - "During network partitions, Alertmanager prefers to send duplicate notifications rather than miss critical alerts." "The system guarantees that notifications are delivered at least once."
  - Peers stagger sends: `wait_time = peer_position × peer_timeout` (default 15s), with position taken from alphabetically sorted peer names. Peer 0 sends first, and the others normally find its nflog entry and skip.
  - In a split brain both sides send ("Duplicate notifications sent"). The logs merge after healing with a "CRDT-like merge with timestamp".
  - On start or rejoin an instance "receives and stores alerts" but holds notifications until the peer count is stable for 3 consecutive checks or `--cluster.settle-timeout` expires.

  — [Alertmanager: High availability](https://prometheus.io/docs/alerting/latest/high_availability/)
- Prometheus must send to *all* Alertmanagers. "Do not load balance traffic between Prometheus and its Alertmanagers"; "The Alertmanager implementation expects all alerts to be sent to all Alertmanagers to ensure high availability." — [Alertmanager README](https://github.com/prometheus/alertmanager/blob/main/README.md); [HA doc](https://prometheus.io/docs/alerting/latest/high_availability/)
- **Persistence.** Silences go to a snapshot (`data/silences`) and the nflog to `data/nflog`. "Alerts themselves are not persisted", because Prometheus re-sends firing alerts. Expired silences are garbage-collected after EndsAt plus retention. — [Alertmanager: High availability](https://prometheus.io/docs/alerting/latest/high_availability/)
- Prometheus side:
  - It re-sends active alerts no more often than `--rules.alert.resend-delay` (1m).
  - It queues up to `--alertmanager.notification-queue-capacity` (10000), batched by `--alertmanager.notification-batch-size` (256).
  - `--alertmanager.drain-notification-queue-on-shutdown` defaults true.

  — [Prometheus CLI flags](https://prometheus.io/docs/prometheus/latest/command-line/prometheus/)
- `--alerts.per-alertname-limit` caps active alerts per alertname: "any new alerts are dropped, heartbeats from already know alerts are processed". — [Alertmanager overview](https://prometheus.io/docs/alerting/latest/alertmanager/)
- `send_resolved` defaults to false for email, Slack and WeChat, and true for most others (webhook, PagerDuty, Opsgenie, ...). — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)
- Version notes: Alertmanager 0.27+ has a UTF-8 matcher parser (fallback mode by default, with `utf8-strict-mode` and `classic-mode` flags), and the docs say strict will become the default "in the next two versions". The top-level `mute_time_intervals` is deprecated in favour of `time_intervals`. `match`/`match_re` are deprecated in favour of `matchers`. — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)

### Inferences
- **Level-triggered input, edge-triggered output.** Prometheus re-sends the full current alert set every resend-delay (a heartbeat of *state*), and Alertmanager turns it into notifications on change or on repeat. Because alerts are re-sent, Alertmanager can drop them on restart and persist only silences and the nflog. For Remit (d): a state-shaped signal (current value) can be re-derived and need not be stored forever. Only what was *delivered* (the firing record, Remit's nflog equivalent) needs durable idempotency.
- **Remit's firing record keyed (subscription, key, revision) is the nflog.** Alertmanager's dedupe rule ("send unless the firing set changed, the resolved set changed, or repeat_interval elapsed") is a precise, documented cooldown policy. Remit's cooldown/deadline window could adopt the same three-clause rule and so make (e) explicit: re-fire on change, re-fire on repeat interval, otherwise suppress *with a recorded reason*.
- **group_wait / group_interval** batch related keys into one action. For agent wakeups this means one "wake chain X with 5 matching readings" in place of 5 wakes. This is a Remit design option that does not exist today.
- **Silences and time intervals act on delivery and leave state alone.** In Remit this suggests that "pause this subscription for 2h" or "only during work hours" should suppress the *action* while evaluation and its trace continue (Grafana's pause-vs-mute distinction). Otherwise a resumed subscription has no idea of current state, which feeds (b) backfill.
- **At-least-once is chosen on purpose and made safe by idempotent receivers.** Remit's single SQLite writer gives exactly-once *recording*. Actions with outside effects (tell an agent, run a job, a plugin) are at-least-once across a crash between record and effect unless the effect itself is keyed. Alertmanager's answer is to accept duplicates and key receivers. The idempotency key (subscription, key, revision) should be passed *into* the action (for example as the chain-start request id).
- The staged pipeline (route → group → inhibit → time filter → silence → dedupe → send → log) gives each suppression a named stage, which makes "why was nothing sent" answerable by stage. This feeds (a) and (g).

### Gaps
- nflog retention is closed from source: `--data.retention` "How long to keep data for." defaults to `120h` (5 days, the same as Grafana's `notification_log_retention` ceiling). `--data.maintenance-interval` "Interval between garbage collection and snapshotting to disk of the silences and the notification logs." defaults to `15m`. `--dispatch.maintenance-interval` defaults to `30s`. — [alertmanager cmd/alertmanager/main.go](https://github.com/prometheus/alertmanager/blob/main/cmd/alertmanager/main.go) (lines 54-61, main at 10a6b699c3, 2026-10-16). A crash can therefore lose up to 15 minutes of nflog, and with it dedupe memory (at-least-once again).
- `resolve_timeout` is closed: "ResolveTimeout is the default value used by alertmanager if the alert does not include EndsAt, after this time passes it can declare the alert as resolved if it has not been updated", default `5m` ([Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)). This is a lease or heartbeat on state: an alert the source stops re-sending expires. - The client contract on `startsAt`/`endsAt` is closed from the repository doc `docs/alerts_api.md` (`prometheus.io/docs/alerting/latest/clients/` returns 404).
  - "Labels are used to deduplicate identical instances of the same alert."
  - When `startsAt` is omitted, it is set to now. When `endsAt` is omitted, it is set to "the current time + `resolve_timeout`" and refreshed on every re-send.
  - "Clients are expected to re-send firing alerts to the Alertmanager at regular intervals until the alert is resolved." "Firing alerts are resolved once their `endsAt` timestamp has elapsed."
  - Clients must also "re-send resolved alerts ... for up to 5 minutes after the alert has resolved. As the Alertmanager is stateless, this ensures that a resolved notification is sent even if the Alertmanager crashes or is restarted."
  - The doc recommends Prometheus rules over direct API posts, because Prometheus "supports a number of special cases to make sure alerts are delivered even if Alertmanager crashes or restarts". APIv1 was removed in 0.27.0.

  — [alertmanager docs/alerts_api.md](https://github.com/prometheus/alertmanager/blob/main/docs/alerts_api.md); the schema is in [api/v2/openapi.yaml](https://github.com/prometheus/alertmanager/blob/main/api/v2/openapi.yaml) (`postableAlert`: optional `startsAt`, `endsAt`, `annotations`; `alert`: required `labels`, optional `generatorURL`).
- Lesson for (d): this is the cleanest prior art for *state as a lease*. A state-shaped signal is a value the source keeps re-asserting; it expires on its own when re-assertion stops, and an ending is re-sent long enough to survive a receiver restart.
- Event Recorder outputs (configuration page past offset 100000) are closed. They are `file_outputs` (`path`), `webhook_outputs` (`url`, `timeout` 10s, `workers` 4, `max_retries` 3, `retry_backoff` 500ms, optional batching), `kafka_outputs` (`brokers`, `topic`, `format` json|protobuf, `acks` default leader) and `stdout_outputs`. — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/). Alertmanager now ships a structured event stream of its own decisions (suppressions included) to outside sinks, which supports recording decisions apart from delivery for (a)/(g).

## Q4. Separation of concerns: rule evaluation (state) vs notification (delivery); contact points and templates

### Takeaway
Both systems split "is this condition true, per instance, over time" (the ruler: Prometheus or the Grafana scheduler) from "who hears about it, when, how often, in what form" (Alertmanager: routing, grouping, silencing, dedupe, contact points and templates). The stated reason is that rules alone "are not a fully-fledged notification solution". The split lets one ruler or many feed one delivery layer, and lets operators mute delivery without touching evaluation.

### Cited Findings
- Prometheus alerting rules "are not a fully-fledged notification solution"; Alertmanager adds summarization, rate limiting, silencing and alert dependencies. — [Prometheus: Alerting rules](https://prometheus.io/docs/prometheus/latest/configuration/alerting_rules/)
- Alertmanager handles "deduplicating, grouping, and routing them to the correct receiver integration". It also handles silencing and inhibition. — [Alertmanager overview](https://prometheus.io/docs/alerting/latest/alertmanager/)
- Grafana offers two routing modes per rule. In Default (simplified), you pick one contact point, notifications go "to this contact point automatically and notification policies aren't used". In Advanced, the policy tree routes by label. "Preview routing" shows which policies would handle the rule's alerts. — [Grafana: Create Grafana-managed rule](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)
- The rule name becomes the `alertname` label. Annotations (summary, description, runbook URL) are templated with query data and carried into notifications. — [Grafana: Create Grafana-managed rule](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)
- Alertmanager templates: a top-level `templates:` list of files (wildcard allowed). Integration fields (`title`, `text`, ...) default to built-in named templates such as `slack.default.text` and can be overridden. Route `labels` are available to templates as `routeLabels`. — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)
- Prometheus rule templates expose `$labels`, `$externalLabels` and `$value`. — [Prometheus: Alerting rules](https://prometheus.io/docs/prometheus/latest/configuration/alerting_rules/)
- Grafana embeds an Alertmanager that uses the same gossip mechanism. Dedupe is best-effort, which "may result in occasional duplicated or out-of-order notifications". A shared external Alertmanager removes the duplicates. — [Grafana: Configure high availability](https://grafana.com/docs/grafana/latest/alerting/set-up/configure-high-availability/)

### Inferences
- Remit's subscription currently fuses three concerns: match (key pattern plus CEL), state (cursor, window) and delivery (one action). The Grafana and Alertmanager split argues for two layers: (1) evaluation produces state transitions per (subscription, key), recorded; (2) delivery maps transitions to actions with dedupe, cooldown and batching, recorded separately. Benefits: (a) failures are attributable to a layer; (b) a dry run is just layer 1 with layer 2 off; (e) windows and cooldowns live in one place (delivery); (f) a slow plugin action runs in the delivery layer, outside the evaluation lock.
- Grafana's two routing modes (a contact point on the rule vs a policy tree) are a usability lesson for (g). A direct "one rule → one action" path is easier for an agent to author and reason about. A label-routed tree is powerful but makes "where did it go" harder, which is why Grafana ships "Preview routing". Remit's one-action-per-subscription is the simplified mode, and no evidence here shows it needs a tree.
- Templated annotations (summary, runbook) carried with the transition are the Grafana way to put context into the notification. For Remit the matching idea is a templated message or brief on the subscription, rendered with the reading's fields when an agent is told.

### Gaps
- No Grafana Labs engineering blog post explaining *why* unified alerting (Grafana 8, 2021) embedded Alertmanager was read; the reasoning above relies on the docs' functional descriptions.

## Q5. Recording rules vs alerting rules (derived signals); rule preview/test (dry run)

### Takeaway
Recording rules are scheduled queries that write a *new derived series*, which alert rules then read. This is a clean "derived signal" concept with ordering (rule sequences or sequential groups). Grafana's dry run is the **Preview** button, which evaluates the query and condition *now* (not over history) and shows which instances would fire. "Preview routing" dry-runs the delivery side. No first-party historical backtest for rules turned up.

### Cited Findings
- Recording rules "pre-compute frequently used or computationally expensive queries" and save results as a new series. Each evaluation writes "a single value as a new data point" on a fixed interval. A Grafana-managed recording rule can query any supported data source. The listed reasons are fewer costly queries, reuse across rules and dashboards, and simpler alert rules. Caveat: "Recorded metrics are not guaranteed to exactly match the original raw data." — [Grafana: Recording rules](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rules/recording-rules/)
- Recording rules and alert rules share evaluation groups. Data-source-managed groups run sequentially so recording rules finish first. Grafana-managed rules can declare a *rule sequence* that runs recording rules before alert rules. — [Grafana: Evaluation within a group](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/evaluation-within-a-group/)
- The rule editor says "Click **Preview** to verify that the expression is successful". — [Grafana: Create Grafana-managed rule](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)
- Grafana v8.5 docs: Preview alerts checks the result of running the query *at this moment*, and preview *excludes no data and error handling*. — [Grafana v8.5 docs](https://grafana.com/docs/grafana/v8.5/alerting/unified-alerting/alerting-rules/index.xml)
- A Grafana learning path describes preview as showing which series would currently fire, to check the threshold is reasonable. — [Grafana learning path: set conditions](https://grafana.com/docs/learning-paths/infrastructure-alerting/set-conditions.md)
- A third-party blog claims preview shows whether the alert "would have fired based on historical data". This contradicts the official "at this moment" description and is not trusted here. — [oneuptime blog](https://oneuptime.com/blog/post/2026-02-09-grafana-alerting-promql/view)
- "Preview routing" shows which notification policies would handle the rule's alerts, based on its labels. — [Grafana: Create Grafana-managed rule](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)
- Alertmanager ships `amtool check-config` for config validation. — [Alertmanager configuration](https://prometheus.io/docs/alerting/latest/configuration/)

### Inferences
- **(d) and derived signals.** A recording rule is "subscription whose action is: write a reading". Remit could treat a derived signal as a subscription that appends a reading to the log under a new key, with explicit ordering. That keeps one log and one mechanism, instead of a separate rollup concept.
- **(b) dry run.** Grafana's preview is a *point-in-time* dry run (evaluate the condition against current data, list the instances that would fire), and it skips NoData/Error handling. Remit has a durable log with global seq, so it can do better cheaply: replay the CEL condition over readings from seq X to now with actions disabled. That yields "would have fired N times at seqs [...], skipped M for cooldown, K errors". This also answers start position: a new subscription's cursor is set explicitly (now / from seq / from time), and the dry-run report shows what a backfill would trigger before it is enabled.
- "Preview routing" equals "show which action would run with what parameters". For agents (g), both previews should be tools that return structured results.

### Gaps
- No found official doc describes a historical backtest for Grafana alert rules in 2025-2026. If one exists (for example in Grafana Cloud or Assistant features), it was not found.
- No found doc gives the Grafana version where Grafana-managed recording rules became GA.

## Q6. Known pitfalls and criticisms (flapping, alert fatigue, NoData storms, state lost on restart, HA duplicates)

### Takeaway
The documented pitfalls cluster around four problems: noise (flapping, NoData/Error storms, too many notifications), state across restarts and edits, side alerts escaping routing and silencing, and HA duplicates. Each has a named knob, and most knobs trade timeliness or strictness for quiet.

### Cited Findings
- **Flapping.** This is why keep firing for and Recovering exist ("caused by flapping conditions"). Re-entering Alerting from Recovering does not re-notify. — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)
- A user notes that with `keep_firing_for`, follow-up alerts close to a previous one are lost, since Prometheus tracks no state beyond the field. — [prometheus-users: Frequent Prometheus alerts](https://groups.google.com/g/prometheus-users/c/nPmjW2gEvqg/m/EaD80NNJBgAJ) (user report)
- **Alert fatigue.** Grouping exists because too many notifications "can overwhelm" responders and hinder incident response ("1 phone call and 100 phone calls"). — [Grafana: Group alert notifications](https://grafana.com/docs/grafana/latest/alerting/fundamentals/notifications/group-alert-notifications/)
- **NoData/Error storms.** The official mitigations, in order:
  1. Keep last state.
  2. Tune the query range (wider for NoData, but not so wide it times out into Error).
  3. Increase the pending period.
  4. Route `alertname=DatasourceError` grouped by `datasource_uid` so one broken source yields one notification.
  5. Change `evaluation_timeout`/`max_attempts` only as a last resort.

  — [Grafana: No Data and Error states](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)
- **Side alerts escape controls.** `DatasourceNoData`/`DatasourceError` "are independent of the original alert, so existing silences, mute timings, and notification policies may not apply to them." — [Grafana: State and health](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/state-and-health/)
- **Restart (Prometheus).** Pending `for` state is restored only within `--rules.alert.for-outage-tolerance` (1h). `--rules.alert.for-grace-period` (10m) is the "Minimum duration between alert and restored 'for' state". — [Prometheus CLI flags](https://prometheus.io/docs/prometheus/latest/command-line/prometheus/). A maintainer says restore is meant for long `for` clauses (hours to days), and for short ones a long restart "is going to be enough to cause issues". — [prometheus-users thread](https://groups.google.com/g/prometheus-users/c/Wy_fEpqSiL4/m/lTgXu3SaAgAJ) (forum, via search snippet)
- A reported pattern: Alertmanager "cycles" alerts after a Prometheus restart. — [prometheus-users thread](https://groups.google.com/g/prometheus-users/c/lQdbQELMIdY) (title only; content not read)
- PR #14048 fixed incomplete restoration when there were no rules to restore. — [Prometheus commit](https://forger.sitiv.fr/apitech/prometheus/commit/c10186eeea1edbe466f8738fc8fcc8871a334f82)
- **Restart (Grafana).** "Existing alert states remain in the database" across failures. Notifications and silences are saved to the database periodically and on graceful shutdown. — [Grafana: Configure high availability](https://grafana.com/docs/grafana/latest/alerting/set-up/configure-high-availability/)
- **Rule edits reset state** to Normal (except annotation, interval or internal edits), so editing a firing rule re-fires it later. — [Grafana: Alert rule evaluation](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)
- **HA duplicates and load.**
  - Duplicate notifications are expected under partition, by design. — [Alertmanager HA](https://prometheus.io/docs/alerting/latest/high_availability/)
  - In Grafana HA, every instance evaluates every rule by default, so data-source load grows with instance count. `ha_single_node_evaluation = true` elects a primary, at the cost of "a brief evaluation gap" on failover. — [Grafana: Configure high availability](https://grafana.com/docs/grafana/latest/alerting/set-up/configure-high-availability/)
  - A 2019 user saw duplicate Slack alerts, which went down with a lower `--cluster.peer-timeout`. — [prometheus-users thread](https://groups.google.com/g/prometheus-users/c/8bnd_-BJx68) (user report)
- **Global timeout knobs.** `evaluation_timeout` and `max_attempts` affect all rules, and too long a timeout "can cause missed evaluations". — [Grafana: No Data and Error states](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)
- **Load caps drop data.** `--alerts.per-alertname-limit` drops new alerts once reached. — [Alertmanager overview](https://prometheus.io/docs/alerting/latest/alertmanager/)
- **History UI limits.** 5000 alerts on the History page, and the OSS History views need Loki. — [Grafana: View alert state history](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/)

### Inferences
- Remit has one process and one SQLite writer, so the HA-duplicate class largely disappears. The crash window between "firing recorded" and "action performed" remains, and should be handled by passing the firing key into the action (idempotent receiver), as Alertmanager relies on.
- Prometheus keeps rule state in memory and restores it heuristically. Grafana persists instance state in the DB. Remit already persists cursors and firings in SQLite, so the lesson is to persist the *per-key state machine* too, or windows and pending timers are lost on restart (e). That matches the Grafana model, not the Prometheus one.
- An Error side-signal must stay governed by the same routing and pause controls as its parent. Otherwise "pause subscription" fails to silence its own error notices (the `DatasourceError` pitfall).
- **(c) retention.** Alertmanager's `repeat_interval` is clamped by `--data.retention`, and Grafana's max repeat interval by `notification_log_retention` (5 days). In both systems the dedupe log's retention bounds what dedupe can guarantee. For Remit: prune readings only below `min(SeenSeq)` of active subscriptions, prune firing records only past the longest cooldown/dedupe window, and refuse (or flag) a window longer than retention, as Alertmanager does.

### Gaps
- No found Grafana Labs blog post or GitHub issue gives aggregate criticism (for example, of unified alerting migration pain); the pitfalls above come from the docs' own caveats and user threads.

## Q7. Mapping mechanisms to Remit open problems (a)-(g)

### Takeaway
The highest-value transfers are four. First, an explicit per-key state machine with Error and NoData as states and a reason enum on each transition (a, d). Second, a transition history separate from delivery records (a, g). Third, a staged delivery pipeline with an nflog-style dedupe rule and per-call timeouts outside the evaluation lock (e, f). Fourth, a replay-based dry run made possible by Remit's global-seq log, which goes further than Grafana's point-in-time Preview (b).

### Cited Findings
| Mechanism (source) | Problem | How it informs Remit |
|---|---|---|
| NoData/Error as configurable states, `DatasourceError` alert ([Grafana](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/)) | (a) | CEL eval error, processor error or timeout → Error state on (subscription, key), visible and routable, author picks the mapping. A source that goes quiet → NoData state. |
| `grafana_state_reason` ([Grafana](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/state-and-health/)) | (a)(g) | Every transition and every skip carries a machine-readable reason (cooldown, window, dedup, error, paused, updated, missing-series). |
| State history per transition, ignoring silences ([Grafana](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/)) | (a)(g) | Record decisions apart from delivery. Add a bounded per-evaluation trace, because transitions alone cannot explain "didn't fire". |
| Preview / Preview routing ([Grafana](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)) | (b)(g) | Agent tools: `dry_run_subscription(from_seq)` replays the condition with actions off, and `preview_action` shows what would run. |
| Rule edit resets state ([Grafana](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)) | (b) | A condition change = new revision, fresh state, explicit start position. |
| Alerts not persisted, re-sent by source; nflog persisted ([Alertmanager HA](https://prometheus.io/docs/alerting/latest/high_availability/)) | (c)(d) | State signals are re-derivable and can be retained short. Durable records are firings (dedupe) and stream events still behind some cursor. |
| `repeat_interval` clamped to retention; nflog retention 5d ([AM config](https://prometheus.io/docs/alerting/latest/configuration/), [Grafana](https://grafana.com/docs/grafana/latest/alerting/fundamentals/notifications/group-alert-notifications/)) | (c) | Retention must respect the longest dedupe window and the minimum subscriber cursor. |
| Instance = (rule, label set); recording rules write derived series ([Grafana](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rules/recording-rules/)) | (d) | Distinguish *state* (latest value per key, instance state) from *stream* (appended readings). Derived signals are subscriptions that append readings. |
| Pending period / keep firing for / Recovering ([Grafana](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/)) | (e) | Windows are state durations measured by the evaluation clock, with an explicit re-entry rule (no re-notify during Recovering). |
| group_wait / group_interval / repeat_interval; dedupe rule ([AM config](https://prometheus.io/docs/alerting/latest/configuration/), [HA](https://prometheus.io/docs/alerting/latest/high_availability/)) | (e) | Make the overlap and repeat policy for recurring runs explicit: fire on change, fire on repeat interval, otherwise suppress with a reason. Optional batching of keys into one action. |
| mute/active time intervals, silences, pause vs mute ([AM config](https://prometheus.io/docs/alerting/latest/configuration/), [Grafana](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)) | (e) | Calendar windows suppress delivery and keep evaluating, so state stays current. |
| `evaluation_timeout` 30s / `max_attempts` 3; `group_interval` as send timeout ([Grafana](https://grafana.com/docs/grafana/latest/alerting/fundamentals/alert-rule-evaluation/nodata-and-error-states/), [AM config](https://prometheus.io/docs/alerting/latest/configuration/)) | (f) | Every evaluation and every send has a deadline, and exhaustion becomes an Error state. The delivery stage runs outside the evaluation path, as Alertmanager's notifier runs apart from Prometheus's ruler. |
| Slow evaluation: newest tick wins, older dropped, `schedule_rule_evaluations_missed_total{reason}` ([schedule.go](https://github.com/grafana/grafana/blob/main/pkg/services/ngalert/schedule/schedule.go)) | (a)(e)(f) | Name the overlap policy for recurring runs (skip, latest wins), and count and record every skip with a reason. |
| `--rules.max-concurrent-evals` 4 ([Prometheus flags](https://prometheus.io/docs/prometheus/latest/command-line/prometheus/)) | (f) | Bounded concurrency for independent evaluations, with no global serialization of slow work. |
| At-least-once + idempotent receivers ([AM HA](https://prometheus.io/docs/alerting/latest/high_availability/)) | (a)(f) | Pass the firing key into actions so a retried action after a crash is a no-op downstream. |
| Simplified routing (contact point on the rule) vs policy tree ([Grafana](https://grafana.com/docs/grafana/latest/alerting/alerting-rules/create-grafana-managed-rule/)) | (g) | Keep one action per subscription (the simpler mode) for agent authors. A tree would need a routing preview tool. |

### Inferences
- Remit's single global mutex serializes both evaluation and delivery. Prometheus and Grafana show the opposite: the ruler evaluates in bounded concurrency with per-evaluation timeouts, and delivery is a separate pipeline with its own timers and retries. A plugin processor that may be slow belongs in the delivery stage (or a bounded worker) with a deadline. Its result should come back as a new reading or state transition, not run inside the lock (f).
- The single-writer SQLite design is an advantage over Alertmanager: exactly-once recording without gossip. The remaining risk is the recorded-but-not-performed window, handled by keyed actions.
- For AI authors (g), every mechanism above should exist as a structured tool result rather than a UI: current state per key, last N transitions with reasons, last N evaluations with CEL results, dry-run over a seq range, and action preview. Grafana's own move toward "Analyze with Assistant" over state history ([Grafana](https://grafana.com/docs/grafana/latest/alerting/monitor-status/view-alert-state-history/)) shows the vendor sees state history as LLM-consumable debugging data.

### Gaps
- The mapping is the researcher's analysis; no source discusses Remit. Its fit should be checked against `internal/orch` subscription code, which this research did not read.
