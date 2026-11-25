// Makes running out of memory end the evaluation, as running too long does. QuickJS throws its out-of-memory error as an
// ordinary InternalError, which a script's try/catch could swallow; every error it throws is handed to
// Error.prepareStackTrace for its stack, so that is where the engine hears of it (outOfMemory, lib.rs), and its interrupt
// handler then raises what no catch can take. The stack is written as QuickJS writes it, and scripts cannot replace the
// hook. An allocation that fails with the heap full to the last bytes leaves QuickJS no room for the error itself: it
// throws null, which reaches no hook (seeing it would take an allocator of our own, which is unsafe code).
(function (outOfMemory) {
  "use strict";
  const frame = (site) => {
    const name = site.getFunctionName() || "<anonymous>";
    if (site.isNative()) return "    at " + name + " (native)";
    const file = site.getFileName();
    const line = site.getLineNumber();
    const where = line > 0 ? ":" + line + ":" + site.getColumnNumber() : "";
    return "    at " + name + " (" + (file === undefined || file === null ? "<null>" : file) + where + ")";
  };
  const prepare = function (error, sites) {
    if (error instanceof InternalError && error.message === "out of memory") outOfMemory();
    return Array.isArray(sites) && sites.length > 0 ? sites.map(frame).join("\n") + "\n" : "";
  };
  Error.prepareStackTrace = prepare;
  Object.defineProperty(Error, "prepareStackTrace", { value: prepare, writable: false, configurable: false });
})
