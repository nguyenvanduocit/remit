// A card's context as the server answers cardContext for a busy agent: the server user's own Claude Code setup loaded
// into it (about 160 skills, a dozen agents, plugins, MCP servers, one of them failed) and Claude Code's count of what
// fills it. `counted: 'none'` answers as for a session Claude Code has not counted yet, `'stored'` as for one whose
// process did not answer now, so the count of when it last rested stands in; `loaded: false` as for one whose first turn
// has not reported what it loaded.
import type { CapabilityView, Card, CardContext, CardTelemetry, ContextCount, ContextGiven } from '../src/core/generated/types.generated';

const plugins = ['superpowers', 'acme-engineer', 'aio-advisor', 'aio-claude-toolkit', 'aio-design-system', 'figma', 'shopify-plugin', 'cowork-plugin-management', 'claude-code-vault-keeper'];
const words = ['review', 'deploy', 'health', 'query', 'verify', 'release', 'debug', 'design', 'test', 'report', 'sync', 'audit', 'plan', 'seed', 'trace', 'search', 'write', 'route'];
const areas = ['lambda', 'k8s', 'mongo', 'starrocks', 'bigquery', 'sqs', 'cloudflare', 'shopify', 'gitlab', 'vault', 'dashboard', 'kanban', 'playwright-bdd', 'testcase'];
// The server's capability catalog (orch.Capabilities).
export const capabilities: CapabilityView[] = [
  { id: 'history', label: 'read the history of past chains', gives: 'list_chains and read_chain: every chain’s goal, status, summary and landings' },
  { id: 'issues', label: 'read customer-reported bugs on Jira', gives: 'list_issues: open customer-reported bugs from Jira, credentials removed' },
  { id: 'propose', label: 'queue new chains on its own', gives: 'propose_chain: the Chair queues new chains for Remit to work on' },
  { id: 'playbooks', label: 'write and edit playbooks', gives: 'the Chair lists, reads, creates, updates, deletes and resets playbooks' },
  { id: 'data', label: 'read stored production and staging data, read-only', gives: 'mongo_collections, mongo_find (MongoDB); starrocks_query (StarRocks); bytebase_query (Bytebase), to every agent of the chain' },
  { id: 'observe', label: 'read metrics, alerts, incidents and logs of the running systems', gives: 'grafana_promql (Grafana); coroot_applications (Coroot); cloudwatch_logs (CloudWatch), to every agent of the chain' },
  { id: 'customers', label: 'read what customers say to support and how they use the product', gives: 'intercom_conversations (Intercom); amplitude_events (Amplitude), to every agent of the chain' },
];

export const skills = Array.from({ length: 162 }, (_, i) => {
  const name = `${areas[i % areas.length]}-${words[(i * 7) % words.length]}${i > 120 ? '-' + i : ''}`;
  return i % 3 === 0 ? `${plugins[i % plugins.length]}:${name}` : name;
});
const builtin = ['Agent', 'Bash', 'Edit', 'Glob', 'Grep', 'Read', 'Write', 'NotebookEdit', 'WebFetch', 'WebSearch', 'TodoWrite', 'Skill', 'ToolSearch', 'Monitor'];
const remitTools = [
  ['update_plan', 'Replace your plan: the steps from where you are to your result, each pending, in_progress or completed. The owner watches it; keep it true whenever a step starts, ends, or the path changes.'],
  ['hire_agent', 'Hire an agent: starts a new Claude Code session with your brief. Its report reaches you when its turn ends. Refused until the chain has a plan (update_plan). Always give deliverable and criteria.'],
  ['accept_work', 'Judge whether an agent did its assignment: accept its report against its acceptance criteria, or send its own work back for rework with specific feedback.'],
  ['route', 'Pass work between agents: handoff, feedback or question. Draws an edge and messages the receiver.'],
  ['open_repo', 'Open a repository for this chain: a git worktree on a branch only this chain uses, from the repository default branch.'],
  ['land', 'Land the chain work on a repository: Remit commits anything left, runs the checks, records the version as a landing, and once every gate approves it combines it with the base, checks it again and pushes it.'],
  ['list_agents', 'Every agent of this chain: status, activity, why it was hired, cwd, context fill, turns and a preview of its last report.'],
  ['read_output', 'The full last report of an agent.'],
  ['stop_agent', 'End an agent session: it is done, redundant or off track.'],
  ['subscribe', 'Subscribe to a time to come back to this chain at: for results that can only be seen later.'],
  ['finish_chain', 'Finish the chain when the goal is met (or cannot be).'],
  ['list_chains', 'Every chain Remit has, newest first.'],
  ['read_chain', 'One chain in full.'],
].map(([name, description]) => ({ name, description }));
const mcpServers = [
  { name: 'remit', status: 'connected', source: 'sdk' },
  { name: 'repowise', status: 'connected', source: 'user' },
  { name: 'github', status: 'connected', source: 'user' },
  { name: 'MongoDB-staging', status: 'connected', source: 'project' },
  { name: 'bytebase-sa', status: 'connected', source: 'user' },
  { name: 'plugin:figma:figma', status: 'needs-auth', source: 'plugin' },
  { name: 'mcp-server-starrocks-dev', status: 'failed', source: 'user' },
  { name: 'claude.ai Google Drive', status: 'pending', source: 'claudeai' },
];
const mcpTools = ['mcp__remit__update_plan', 'mcp__remit__hire_agent', 'mcp__repowise__search_codebase', 'mcp__repowise__get_context', 'mcp__github__list_issues', 'mcp__playwright__browser_snapshot', 'mcp__MongoDB-staging__find', 'mcp__MongoDB-staging__aggregate', 'mcp__bytebase-sa__query_database'];
const brief = `You are the Chair of a Remit chain: you hire agents, route their work and decide when the goal is met.

GOAL (from the owner)
Show me what fills an agent's context in its Session tab: which skills, MCP servers, AGENTS.md, ... were loaded.

PLAYBOOK: Build a feature
Fix what done means before anything is built: acceptance criteria that can be checked by running the feature.
${'Design before code, and follow the repository existing patterns rather than inventing new ones. '.repeat(12)}

The owner's working principles are in /workspace/example/AGENTS.md. Read them first and follow them.`;

function countOf(card: Card, at: string): ContextCount {
  const part = (id: string, name: string, tokens: number, kind = 'used') => ({ id, name, tokens, kind });
  const parts = [
    part('system_prompt', 'System prompt', 3_364),
    part('system_tools', 'System tools', 8_544),
    part('system_tools_deferred', 'System tools (deferred)', 14_801, 'deferred'),
    part('mcp_instructions', 'MCP server instructions', 412),
    part('mcp_tools_deferred', 'MCP tools (deferred)', 6_412, 'deferred'),
    part('custom_agents', 'Custom agents', 1_190),
    part('memory_files', 'Memory files', 11_670),
    part('skills', 'Skills', 7_509),
    part('messages', 'Messages', 58_920),
    part('autocompact_buffer', 'Autocompact buffer', 33_000, 'buffer'),
  ];
  const total = parts.filter((p) => p.kind === 'used').reduce((n, p) => n + p.tokens, 0);
  parts.push(part('free_space', 'Free space', 1_000_000 - total - 33_000, 'free'));
  const item = (name: string, tokens: number, source = '', deferred = false) => ({ name, source, tokens, deferred });
  const context = {
    sessionId: card.sessionId ?? '', model: 'claude-opus-4-7[1m]', total, size: 1_000_000, compactAt: 400_000, parts,
    memoryFiles: [
      item('/home/owner/.remit/work/ch_5bddcf0823e3/subscriptionfns/AGENTS.md', 9_840, 'Project'),
      item('/home/owner/.remit/work/ch_5bddcf0823e3/subscriptionfns/internal/billing/CLAUDE.md', 1_806, 'Project'),
      item('/home/owner/.remit/work/ch_5bddcf0823e3/subscriptionfns/CLAUDE.md', 24, 'Project'),
    ],
    skills: skills.map((name, i) => item(name, 20 + ((i * 37) % 160), i % 3 === 0 ? 'plugin' : 'projectSettings')),
    agents: ['candidate-evaluator', 'incident-responder', 'mr-reviewer'].map((name, i) => item(name, 240 + i * 90, 'projectSettings')),
    mcpTools: mcpTools.map((name, i) => item(name, 180 + i * 40, name.split('__')[1], i > 1)),
    toolCalls: [item('Bash', 21_400), item('Read', 18_230), item('Edit', 4_120), item('mcp__remit__update_plan', 610)],
  };
  return { context, at };
}

// What a busy agent's sessions exported (`card_telemetry`): `none` as for a card whose sessions exported nothing yet.
export function telemetryOf(opts: { none?: boolean } = {}): CardTelemetry {
  const now = Date.now();
  if (opts.none) {
    return { requests: 0, errors: 0, inputTokens: 0, outputTokens: 0, cacheReadTokens: 0, cacheCreationTokens: 0, costUsd: 0, latencyP50Ms: 0, latencyP95Ms: 0, ttftP50Ms: 0, models: [], tools: [], recentErrors: [], firstAtMs: 0, lastAtMs: 0 };
  }
  return {
    requests: 142, errors: 2, inputTokens: 3_910, outputTokens: 61_204, cacheReadTokens: 9_412_880, cacheCreationTokens: 402_118, costUsd: 7.4213,
    latencyP50Ms: 6_840, latencyP95Ms: 28_310, ttftP50Ms: 2_120,
    models: [{ model: 'claude-opus-4-7[1m]', effort: 'high', requests: 131, costUsd: 7.2902 }, { model: 'claude-haiku-5-5', effort: '', requests: 11, costUsd: 0.1311 }],
    tools: [
      { name: 'Bash', runs: 64, failed: 5, totalMs: 412_000, p95Ms: 38_200 },
      { name: 'Read', runs: 51, failed: 0, totalMs: 6_120, p95Ms: 240 },
      { name: 'Edit', runs: 18, failed: 1, totalMs: 2_904, p95Ms: 410 },
      { name: 'mcp__remit__update_plan', runs: 6, failed: 0, totalMs: 1_310, p95Ms: 380 },
    ],
    recentErrors: [
      { atMs: now - 11 * 60_000, model: 'claude-opus-4-7[1m]', status: '529', error: 'Overloaded' },
      { atMs: now - 52 * 60_000, model: 'claude-opus-4-7[1m]', status: '', error: 'Request timed out.' },
    ],
    firstAtMs: now - 3_600_000, lastAtMs: now - 60_000,
  };
}

export function contextOf(card: Card, opts: { counted?: 'now' | 'stored' | 'none'; loaded?: boolean; given?: Partial<ContextGiven> | null } = {}): CardContext {
  const counted = opts.counted ?? 'now';
  const at = new Date(Date.now() - (counted === 'stored' ? 42 * 60_000 : 0)).toISOString();
  return {
    cardId: card.id,
    given: opts.given === null ? null : {
      skills: [], claudeTools: null, settingSources: ['project'], missing: [], tools: remitTools,
      capabilities: capabilities.slice(0, 1),
      allowedTools: ['mcp__remit'],
      denyRules: ['Bash(git push:*)', 'Bash(git remote add:*)', 'Bash(git remote set-url:*)', 'Bash(glab mr merge:*)', 'Bash(glab mr accept:*)', 'Bash(glab mr approve:*)', 'Bash(glab mr create:*)', 'Bash(glab mr update:*)', 'Bash(glab mr close:*)', 'Bash(glab mr note:*)', 'Bash(glab mr rebase:*)', 'Bash(gh pr merge:*)', 'Bash(gh pr create:*)'],
      principles: '/workspace/example/AGENTS.md', opening: brief,
      ...opts.given,
    },
    loaded: opts.loaded === false ? null : {
      init: {
        sessionId: card.sessionId ?? '', version: '2.1.322', model: 'claude-opus-4-7[1m]', permissionMode: 'auto', outputStyle: 'default',
        tools: [...builtin, ...mcpTools], mcpServers, skills,
        plugins: plugins.map((name, i) => ({ name, path: `/home/owner/.claude/plugins/cache/${name}`, version: i % 2 ? `1.${i}.0` : '' })),
        pluginErrors: [{ plugin: 'my-tools@local', type: 'path-not-found', message: 'Plugin directory /home/owner/my-tools/plugin does not exist' }],
        agents: ['general-purpose', 'Explore', 'Plan', 'statusline-setup', 'claude', 'candidate-evaluator', 'incident-responder', 'mr-reviewer'],
        slashCommands: ['compact', 'context', 'cost', 'review', 'security-review', 'init', 'loop', 'schedule', 'simplify'],
      },
      at: new Date(Date.now() - 4 * 60_000).toISOString(),
    },
    context: counted === 'none' ? null : countOf(card, at),
    contextError: counted === 'stored' ? 'the agent\'s process did not answer' : '',
  };
}
