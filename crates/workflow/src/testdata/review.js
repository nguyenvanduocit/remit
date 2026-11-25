export const meta = {
  name: 'review-changes',
  description: 'Find bugs along several dimensions, dedup, verify each',
  phases: [
    { title: 'Find', detail: 'one finder per dimension' },
    { title: 'Verify', detail: 'one verifier per finding' },
  ],
}
const DIMENSIONS = args.dimensions.map(d => ({ name: d, prompt: 'Find ' + d + ' bugs in the diff.' }))
const FINDINGS_SCHEMA = { type: 'object', properties: { findings: { type: 'array' } }, required: ['findings'] }
const VERDICT_SCHEMA = { type: 'object', properties: { real: { type: 'boolean' } }, required: ['real'] }
const dedupeByFileAndLine = fs => {
  const seen = new Set()
  return fs.filter(f => { const k = f.file + ':' + f.line; if (seen.has(k)) return false; seen.add(k); return true })
}

phase('Find')
const all = await parallel(DIMENSIONS.map(d => () => agent(d.prompt, { label: d.name, schema: FINDINGS_SCHEMA })))
const deduped = dedupeByFileAndLine(all.filter(Boolean).flatMap(r => r.findings))
if (!deduped.length) return []
log(deduped.length + ' findings to verify')
phase('Verify')
const verified = await parallel(deduped.map(f => () =>
  agent('Verify ' + f.file + ':' + f.line + ' ' + f.desc + '. Default to real=false if uncertain.', { schema: VERDICT_SCHEMA })
    .then(v => ({ ...f, real: v.real }))))
return verified.filter(Boolean).filter(v => v.real)
