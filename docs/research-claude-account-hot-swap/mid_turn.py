"""Push a token while a tool call runs; the same turn's next model request must use it (fake token -> 401). Usage: python3 -I mid_turn.py"""
import json, subprocess, tempfile, threading, time
p = subprocess.Popen(["claude","-p","--model","haiku","--input-format","stream-json","--output-format","stream-json","--verbose",
    "--allowedTools","Bash"], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, cwd=tempfile.mkdtemp())
pid0 = p.pid
def send(o): p.stdin.write(json.dumps(o)+"\n"); p.stdin.flush()
t0 = time.time()
send({"type":"user","message":{"role":"user","content":"Run the bash command `sleep 8 && echo done`, then reply with the word FINISHED."}})
pushed = False
for line in p.stdout:
    o = json.loads(line); t = round(time.time()-t0,1)
    if o.get("type")=="assistant":
        for c in o["message"]["content"]:
            if c.get("type")=="tool_use" and not pushed:
                print(t,"tool_use started -> pushing bogus token mid-turn")
                send({"type":"update_environment_variables","variables":{"CLAUDE_CODE_OAUTH_TOKEN":"sk-ant-oat01-bogus-mid"},"request_id":"mid"})
                pushed = True
            if c.get("type")=="text": print(t,"assistant text:",c["text"][:80])
    if o.get("type")=="control_response": print(t,"ack",o["response"]["subtype"])
    if o.get("type")=="user": print(t,"tool_result returned")
    if o.get("type")=="result":
        print(t,"RESULT is_error=",o.get("is_error"),"|",str(o.get("result"))[:150]); break
print("same pid:", p.pid==pid0, "alive:", p.poll() is None)
p.stdin.close(); p.wait(timeout=60)
