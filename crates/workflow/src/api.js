// Installs the script's verbs on a fresh VM and returns settle, the one way a recorded result reaches the script.
// call(verb, inputJSON, label, phase) records a call in Rust (lib.rs) and returns its seq. The verbs validate their
// arguments by throwing, as Claude Code's do, so a wrong call fails at the line that made it.
//
// parallel and pipeline are plain promise combinators: concurrency inside a script is the interleaving of its promise
// jobs, which is deterministic once results are settled in the order they arrived (see advance).
(function (call) {
  "use strict";
  const waiting = new Map();
  let current = "";

  const issue = (verb, input, label, phase) => {
    const seq = call(verb, JSON.stringify(input === undefined ? null : input), label || "", phase || "");
    return new Promise((resolve, reject) => waiting.set(seq, { resolve, reject }));
  };
  const record = (verb, input) => { call(verb, JSON.stringify(input), "", current); };
  const text = (what, v) => { if (typeof v !== "string") throw new TypeError(what + " must be a string"); return v; };
  const list = (what, v) => {
    if (!Array.isArray(v)) throw new TypeError(what + " must be an array");
    if (v.length > 4096) throw new RangeError(what + " takes at most 4096 items, got " + v.length);
    return v;
  };
  const object = (what, v) => {
    if (v === null || typeof v !== "object" || Array.isArray(v)) throw new TypeError(what + " must be an object");
    return v;
  };
  const AGENT_OPTIONS = ["label", "phase", "schema", "model", "effort", "agentType"];

  globalThis.agent = (prompt, opts) => {
    text("agent()'s prompt", prompt);
    const o = opts === undefined ? {} : object("agent()'s options", opts);
    // The agents of a chain share its one worktree, so parallel agents that each wanted their own would write the same tree.
    if (o.isolation !== undefined) {
      throw new TypeError('agent()\'s isolation "' + o.isolation + '": Remit gives no agent a worktree of its own yet: run these agents one after another, or let one agent make the changes');
    }
    const input = { prompt };
    for (const k of AGENT_OPTIONS) if (o[k] !== undefined) input[k] = o[k];
    return issue("agent", input, o.label, o.phase || current);
  };
  globalThis.workflow = (nameOrRef, args) => {
    const input = typeof nameOrRef === "string"
      ? { name: nameOrRef }
      : { scriptPath: text("workflow()'s scriptPath", object("workflow()'s reference", nameOrRef).scriptPath) };
    if (args !== undefined) input.args = args;
    return issue("workflow", input, input.name || input.scriptPath, current);
  };
  globalThis.hire = (input) => issue("hire", object("hire()'s input", input), input.role, current);
  globalThis.wait = (spec) => issue("wait", object("wait()'s input", spec), "", current);
  globalThis.askOwner = (question) => issue("ask_owner", { question: text("askOwner()'s question", question) }, "", current);
  globalThis.accept = (card, notes) => {
    const input = { card: text("accept()'s card", card) };
    if (notes !== undefined) input.notes = text("accept()'s notes", notes);
    return issue("accept", input, "", current);
  };
  globalThis.sendBack = (card, feedback) =>
    issue("send_back", { card: text("sendBack()'s card", card), feedback: text("sendBack()'s feedback", feedback) }, "", current);

  globalThis.log = (message) => { record("log", { message: String(message) }); };
  globalThis.phase = (title) => { current = text("phase()'s title", title); record("phase", { title: current }); };

  globalThis.parallel = (thunks) => Promise.all(list("parallel()'s thunks", thunks).map((t) => {
    try { return Promise.resolve(t()).then((v) => v, () => null); } catch (e) { return null; }
  }));
  globalThis.pipeline = (items, ...stages) => {
    for (const s of stages) if (typeof s !== "function") throw new TypeError("pipeline()'s stages must be functions");
    return Promise.all(list("pipeline()'s items", items).map((item, i) => {
      let p = Promise.resolve(item);
      for (const s of stages) p = p.then((prev) => s(prev, item, i));
      return p.then((v) => v, () => null);
    }));
  };
  globalThis.budget = Object.freeze({ total: null, spent: () => 0, remaining: () => Infinity });

  return (seq, ok, payload) => {
    const w = waiting.get(seq);
    waiting.delete(seq);
    if (ok) w.resolve(payload === "" ? null : JSON.parse(payload));
    else w.reject(new Error(payload));
  };
})
