// Makes a fresh VM deterministic before any script runs, as Claude Code's Workflow tool does: a script replayed from its
// journal must take the same path every time, so nothing it can read may change between evaluations. The clock and
// randomness throw; code from strings (eval, Function and the constructors reachable from any function) does not exist.
// The context has no timers, modules, Performance or WeakRef and no host object but the API's call (lib.rs), so nothing
// else reads a file, the network, the environment or the garbage collector.
(function () {
  "use strict";
  const forbidden = (what, why) => function () { throw new TypeError(what + " is not available in a workflow: " + why); };
  const clock = "it would change between replays; pass a time in through args";

  // Date is replaced by a function rather than a subclass: a subclass's prototype is the real Date, whose constructor
  // and now() read the clock through Object.getPrototypeOf(Date).
  const RealDate = Date;
  const SafeDate = function Date(...a) {
    if (new.target === undefined) throw new TypeError("Date() is not available in a workflow: " + clock);
    if (a.length === 0) throw new TypeError("new Date() is not available in a workflow: " + clock);
    return Reflect.construct(RealDate, a, new.target);
  };
  SafeDate.prototype = RealDate.prototype;
  SafeDate.UTC = RealDate.UTC;
  SafeDate.parse = RealDate.parse;
  SafeDate.now = forbidden("Date.now()", clock);
  Object.defineProperty(RealDate.prototype, "constructor", { value: SafeDate, writable: false, configurable: false });
  Object.defineProperty(globalThis, "Date", { value: SafeDate, writable: false, configurable: false });

  Math.random = forbidden("Math.random()", "it would change between replays; vary the prompt or label by index");

  const noCode = forbidden("Code from a string", "a workflow is the script it was started with");
  const makers = [function () {}, async function () {}, function* () {}];
  try { makers.push(eval("(async function* () {})")); } catch (e) {}
  for (const f of makers) {
    Object.defineProperty(Object.getPrototypeOf(f), "constructor", { value: noCode, writable: false, configurable: false });
  }
  delete globalThis.eval;
  delete globalThis.Function;
})()
