// Sets window.paintMs to the time from this call to the first frame drawn after it. The second requestAnimationFrame
// runs once the frame the first one preceded has painted, so the time holds Vue's render, style, layout and paint.
export function markPaint(): void {
  const t0 = performance.now();
  requestAnimationFrame(() => requestAnimationFrame(() => Object.assign(window, { paintMs: Math.round(performance.now() - t0) })));
}
