"""Swap a running Claude Code session's login over stdin, between two turns.

Usage: python3 -I poc.py [token]
With no token a fake one is pushed: turn 2 must then fail with a 401, which proves the pushed token is used.
Pass a second account's access token to see turn 2 answer on that account.
"""
import json, subprocess, sys, tempfile

token = sys.argv[1] if len(sys.argv) > 1 else "sk-ant-oat01-bogus-poc-token"
p = subprocess.Popen(
    ["claude", "-p", "--model", "haiku", "--input-format", "stream-json", "--output-format", "stream-json", "--verbose"],
    stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=tempfile.mkdtemp())


def send(o):
    p.stdin.write(json.dumps(o) + "\n")
    p.stdin.flush()


def turn(tag, text):
    send({"type": "user", "message": {"role": "user", "content": text}})
    for line in p.stdout:
        o = json.loads(line)
        if o.get("type") == "control_response":
            print(tag, "control_response", json.dumps(o["response"]))
        if o.get("type") == "result":
            print(tag, "is_error =", o.get("is_error"), "|", str(o.get("result"))[:200])
            return


turn("turn1", "Reply with the word ONE only.")
send({"type": "update_environment_variables", "variables": {"CLAUDE_CODE_OAUTH_TOKEN": token}, "request_id": "poc-1"})
turn("turn2", "Reply with the word TWO only.")
p.stdin.close()
p.wait(timeout=60)
