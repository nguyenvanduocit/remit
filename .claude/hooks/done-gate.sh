#!/bin/bash
# Done gate: runs when the main agent or a subagent stops (Stop, SubagentStop). If its final message hands back work that is
# unfinished, or a claim it did not verify, it is not allowed to stop: the reason goes back to it and it continues.
#
# A cheap keyword pass skips the judge for a clean message. Otherwise a headless `claude -p` judges (DONE_GATE_RUNNING stops that
# call from running this hook again). The gate fails open (a judge that errors lets the agent stop) and blocks at most
# DONE_GATE_MAX times per agent, so it can never loop.
set -u

[ -n "${DONE_GATE_RUNNING:-}" ] && exit 0

input=$(cat)
last=$(jq -r '.last_assistant_message // ""' <<<"$input")
who=$(jq -r '.agent_id // .session_id // "main"' <<<"$input")
[ "${#last}" -lt 300 ] && exit 0

if ! grep -Eiq 'not done|not verified|unverified|not looked at|could not|couldn.t|did not|didn.t|not (yet )?(built|run|checked|tested|wired|implemented)|left for|still red|still open|remaining|todo|deviation|out of scope|chưa|không (thể|kiểm)|cần (bạn|owner)|open question|next step' <<<"$last"; then
	exit 0
fi

state=/tmp/done-gate
mkdir -p "$state"
count_file="$state/${who//[^A-Za-z0-9_-]/_}"
count=$(cat "$count_file" 2>/dev/null || echo 0)
max=${DONE_GATE_MAX:-3}
[ "$count" -ge "$max" ] && exit 0

prompt=$(cat <<EOF
You are the done gate. Below is the final message of an AI agent that is about to stop. Decide whether it hands back anything unfinished.

BLOCK when the message lists, or admits to, any of these that the agent could still do itself with the tools it has (run a command or test, read a file, open the browser, call the API, edit code, write the doc, make the migration):
- work in the request that is not done, "not done", "left for later", "remaining", a deviation from the design that it could fix or reconcile;
- a claim it did not verify ("not verified", "not looked at", "not run", "should work"): every such claim must be verified by some means, and "I did not open the screen" is a reason to open it;
- a check that is red in the scope it changed.

ALLOW when every open item is one of: a decision that is the owner's to make, stated as BLOCKED: or as a question with options; work that a named other session or agent owns, outside the agent's scope; an external blocker the agent hit AFTER trying (an expired login, a quota, no network), with the evidence of the attempt; or nothing is open. A red check that predates the agent and sits outside what it changed is allowed when named as such.

Answer in this exact shape and nothing else. First line: BLOCK or ALLOW. If BLOCK, then one line per item as "- <the item> -> <what to do now, concretely>".

AGENT MESSAGE:
$last
EOF
)

verdict=$(DONE_GATE_RUNNING=1 timeout 100 claude -p "$prompt" --model haiku 2>/dev/null) || exit 0
first=$(head -n1 <<<"$verdict" | tr -d '[:space:]*')
[ "$first" = "BLOCK" ] || exit 0

echo $((count + 1)) >"$count_file"
items=$(tail -n +2 <<<"$verdict")
jq -n --arg items "$items" '{decision: "block", reason: ("You are not done. Finish these now (if one is truly impossible, say what you tried and what the evidence was), then report again:\n" + $items + "\nUnverified claims: find any way to verify them (run it, open it, call it, read it) before you say they hold.")}'
exit 0
