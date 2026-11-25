# How coding agents learn from people: research report, 2026-11-04

Deep-research run `wf_80948d89-859`: 5 search angles, 23 sources fetched, 103 claims extracted, 25 put to a 3-vote adversarial check (7 dropped for budget), 23 confirmed, 2 refuted. The raw results are in [report.json](report.json). The [source table](#sources) links to the research material.

## Question

How can a coding agent (an LLM-based software engineering agent such as Claude Code, Codex, Devin, Cursor agents) learn from humans over time: how it accumulates knowledge, experience, and taste (human aesthetic and engineering judgment), and the concrete mechanisms for that — persistent memory and skill libraries, learning from code review feedback and corrections, preference learning (RLHF, DPO, Constitutional AI, RLAIF), in-context learning from examples and style guides, mentorship/apprenticeship loops, reflection (Reflexion, Voyager skill libraries, ExpeL), learning from demonstrations and edit histories, and evaluating whether the agent's taste matches a specific human's taste. Cover both research literature (2023–2026) and what production tools actually ship today, and what works vs. what is hype.

## Summary

The evidence supports one main conclusion. Coding agents learn from humans mostly outside the model weights: through prompts, memory and skill files that sit beside a frozen model. These include induced workflows (AWM), banks of past experience (SWE-Exp), natural-language preferences inferred from user edits (PRELUDE/CIPHER), and human-curated skills and knowledge items (Anthropic Agent Skills, Devin Knowledge). Today's production tools ship the human-in-the-loop version. Devin suggests Knowledge entries from chat corrections, and the user approves them before they are saved. Anthropic's Skills are written by people or at a person's direction; agents writing their own skills is still stated as a hope. Research results are mixed. Experience reuse can help: SWE-Exp self-reports 73.0% on SWE-Bench Verified, a lead of 1.4 to 2.2 points. But naive memory can also hurt: in SWE-Bench-CL's pilot, failed attempts polluted the memory ('garbage-in, garbage-out'). Work on measuring taste is still immature. Matching personal coding style was a little-studied task as of 2024 (MPCoder's style metric covers surface conventions). Reward-model accuracy predicts personalized behavior poorly, and plain in-context learning from a user's own examples beat reward-guided decoding above about 3B parameters. In practice, the methods that work now are curated in-context knowledge and preferences inferred from edits. Autonomous self-improving skill libraries and per-user RLHF for taste are not established.

## Findings

### 1. Confidence high (vote 3-0 (all merged claims))

Most mechanisms for accumulating experience leave the model's weights unchanged. AWM induces reusable workflows from past trajectories and feeds them back into context, both offline from annotated examples and online from the agent's own test queries, judged by an LLM evaluator. SWE-Exp distills successful and failed repair trajectories into a multi-level experience bank, from problem comprehension down to code changes. One survey classes such memory, skill libraries and subagents as 'T2 tool adaptation': built from the agent's outputs while the base agent stays frozen.

*Evidence:* Merges claims 2, 4, 5, 6 and 7, all with 3-0 votes from primary sources. AWM was evaluated on web navigation (Mind2Web, WebArena), not coding, so applying it to coding agents is an analogy. The A1/A2/T1/T2 taxonomy comes from one survey and is not a field-wide standard.

*Sources:* https://arxiv.org/abs/2409.07429, https://huggingface.co/papers/2507.23361, https://arxiv.org/abs/2512.16301

### 2. Confidence medium (vote 3-0)

Experience reuse gives mixed results on coding benchmarks. SWE-Exp self-reports 73.0% Pass@1 on SWE-Bench Verified with Claude 4 Sonnet, against SWE-Search at 70.8%, OpenHands at 70.4% and SWE-Agent at 66.6%. In SWE-Bench-CL's pilot runs, retrieval memory did as well as or slightly worse than no memory: pass rates stayed below about 8.5%, and the authors blame failed attempts filling the memory. SWE-Bench-CL orders SWE-Bench Verified issues chronologically to test transfer and forgetting.

*Evidence:* Merges claims 0, 1 and 3 (3-0). SWE-Exp's lead is small, comes from a single run without variance, is self-reported, and the paper flags possible contamination; its ablations ran on DeepSeek-V3. The SWE-Bench-CL authors call their results inconclusive, and they used simple retrieval-augmented prompting rather than their proposed framework. Both papers are preprints. Medium because the two results point in opposite directions and neither is replicated.

*Sources:* https://huggingface.co/papers/2507.23361, https://arxiv.org/abs/2507.00014

### 3. Confidence medium (vote 3-0)

Learning a user's taste from their edits works without fine-tuning. PRELUDE/CIPHER infers a natural-language preference from each user edit, retrieves the preferences of the k most similar past contexts, and puts them in the prompt. It had the lowest cumulative edit distance of the non-oracle methods: about 32,974 against 48,269 with no learning on summarization, and about 10,058 against 31,103 on email writing.

*Evidence:* Merges claims 8 and 9 (3-0; NeurIPS 2024 primary source, numbers match the paper's table). One source. The users were GPT-4 standing in for people, the tasks were writing, not code, and variance was high (for example ±4,868 for one email variant). The best email result was the BERT variant at 8,391.

*Sources:* https://arxiv.org/abs/2404.15269

### 4. Confidence medium (vote 3-0)

Weight-level learning from edits and preferences helps a little but does not match frontier models, and the 'human' signal is often synthetic. Fine-tuning small open models on real developer edits to AI-generated code (DECODE-Real) made their edits more like developers' edits, but did not close the gap to frontier models; the fine-tuned models also tended to under-edit. DPO-f+ adds a frozen reward-model margin to DPO for code-repair feedback, reports +5.71 pp Pass@1, and needs no online RL. Its 'developer preferences', though, were GPT-4 rubric scores (RLAIF-style), not real developer feedback.

*Evidence:* Merges claims 10, 11 and 12 (3-0). Both are preprints. The over-editing comparison is between fine-tuned and frontier models, not before and after fine-tuning, and 26% of the fine-tuned models' errors were failures to edit. DPO-f+ used a small benchmark (CodeLlama-7B, novice tasks). The paper reports that people checked 100 of the judge's cases and agreed 95% of the time. A related claim, that frontier models mostly revert human edits, was refuted 1-2.

*Sources:* https://arxiv.org/pdf/2607.25130, https://arxiv.org/html/2511.01043v1

### 5. Confidence high (vote 3-0)

Production tools ship memory curated by humans, not autonomous learning. Devin suggests new or updated Knowledge items from chat corrections, and the user edits or dismisses them before saving. Anthropic Agent Skills are folders with a SKILL.md file plus scripts and resources, loaded progressively at runtime across Claude Code, the API and claude.ai. Anthropic's recommended workflow is that the user asks Claude to save what worked and its common mistakes into a skill. Agents that create, edit and evaluate skills on their own were a stated hope as of October 2025.

*Evidence:* Merges claims 13, 14, 15 and 16 (3-0, vendor primary documentation, which is the authority for what the vendor ships). The Devin documentation has moved from /onboard-devin/knowledge to /product-guides/knowledge. A broader claim describing Devin Knowledge as human-authored tips was refuted 0-3. The skills feature is visible live in Claude Code sessions as of November 2026.

*Sources:* https://docs.devin.ai/product-guides/knowledge, https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills

### 6. Confidence medium (vote 3-0 (ICL sub-claim 2-1))

Measuring whether an agent matches one person's taste is immature, and simple in-context learning is a strong baseline. MPCoder (ACL 2024) frames multi-user personalized code generation as a new, little-studied task. It splits style into explicit syntax standards and implicit semantic conventions, and proposes a style-similarity metric (Coding Style Score). In personalized alignment, reward-model accuracy correlates only weakly with policy-level discrimination (Kendall's tau 0.08 to 0.31). Methods 20 points apart in reward-model accuracy produce almost the same output quality. For models of 3B parameters and up, in-context learning from the user's examples beat every reward-guided decoding method tested (about 3 ROUGE-1 points at 7B in v3).

*Evidence:* Merges claims 17 through 22. Claims 17 to 21 voted 3-0; the ICL claim (22) voted 2-1 and is corrected from '3-5' to '~3' points per v3 (October 2026). The Coding Style Score measures surface conventions such as formatting and naming, not engineering judgment. The Pref-LaMP results cover reward-guided decoding on text, scored with ROUGE-1, not RLHF/DPO training and not code.

*Sources:* https://aclanthology.org/2024.acl-long.207/, https://arxiv.org/abs/2512.23067

## Caveats

Most sources are 2024–2026 arXiv preprints with self-reported, unreplicated results. Several key results (PRELUDE/CIPHER, Pref-LaMP, AWM) come from writing or web tasks, not coding agents, so applying them to coding is inference. Simulated users (GPT-4) and AI judges stand in for real human feedback in several studies, so evidence of learning from real humans is thin. Vendor documentation only shows that features ship, not that they work. The surviving claims cover no direct evidence on Constitutional AI, RLHF applied to individual coding taste, Reflexion/Voyager/ExpeL on coding benchmarks, or the memory features of Cursor and Codex. Product statements are time-sensitive: Anthropic's 'hope' for autonomous skill creation dates from October 2025, and the Devin documentation has already moved. The research count is 23 claims confirmed and 2 refuted; claims never extracted are not represented.

## Open questions

- How can an agent's memory be kept from filling up with failed experience? What gating, such as verifier-filtered or human-approved entries, makes memory help over long chronological sequences like SWE-Bench-CL?
- Does learning preferences from edits (CIPHER-style) carry over to real developers' code review corrections, and how does it compare with curated rule files such as CLAUDE.md or skills?
- What metric captures engineering taste (design choices, abstraction level, naming judgment) beyond surface style, and is it validated against a specific human's accept/reject decisions?
- Has any production tool shipped autonomous skill creation and evaluation since late 2025, and is there evidence that it beats human-curated skills?

## Refuted claims

- (1-2) Frontier LLMs asked to infer a developer's intent from their edits to AI-generated code mostly fail by undoing the human's edit and going back to the original completion. In other words, they ignore explicit correction feedback. Source: https://arxiv.org/pdf/2607.25130
- (0-3) Devin ships a persistent memory feature called Knowledge: short, human-authored tips and instructions that it can retrieve in any session. Note that the requested URL (/onboard-devin/knowledge) returned 404, and this content was fetched from docs.devin.ai/product-guides/knowledge. Source: https://docs.devin.ai/onboard-devin/knowledge

## Sources

| Quality | Angle | Claims | URL |
|---|---|---|---|
| primary | Academic: agent memory, reflection and skill libraries | 5 | https://arxiv.org/abs/2507.00014 |
| primary | Academic: agent memory, reflection and skill libraries | 5 | https://huggingface.co/papers/2507.23361 |
| primary | Academic: agent memory, reflection and skill libraries | 4 | https://arxiv.org/abs/2409.07429 |
| primary | Academic: agent memory, reflection and skill libraries | 5 | https://arxiv.org/abs/2512.16301 |
| secondary | Academic: agent memory, reflection and skill libraries | 4 | https://chatpaper.com/paper/285768 |
| unreliable | Academic: agent memory, reflection and skill libraries | 0 | https://www.catalyzex.com/author/Heng%20Lian |
| primary | Learning from code review, edits and corrections | 5 | https://arxiv.org/abs/2404.15269 |
| primary | Learning from code review, edits and corrections | 5 | https://arxiv.org/html/2511.01043v1 |
| primary | Learning from code review, edits and corrections | 5 | https://arxiv.org/pdf/2607.25130 |
| primary | Production tools: what actually ships | 5 | https://docs.devin.ai/onboard-devin/knowledge |
| primary | Production tools: what actually ships | 5 | https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills |
| blog | Production tools: what actually ships | 5 | https://getunblocked.com/blog/claude-md-vs-agents-md-vs-cursor-rules/ |
| blog | Production tools: what actually ships | 5 | https://localskills.sh/blog/cursor-memories-guide |
| primary | Taste: personalization and evaluating alignment with a specific human | 5 | https://aclanthology.org/people/zhenlong-dai/unverified/ |
| primary | Taste: personalization and evaluating alignment with a specific human | 5 | https://arxiv.org/abs/2512.23067v1 |
| primary | Taste: personalization and evaluating alignment with a specific human | 4 | https://proceedings.iclr.cc/paper_files/paper/2025/hash/a730abbcd6cf4a371ca9545db5922442-Abstract-Conference.html |
| primary | Taste: personalization and evaluating alignment with a specific human | 5 | https://arxiv.org/abs/2510.27565v1 |
| secondary | Taste: personalization and evaluating alignment with a specific human | 4 | https://www.chatpaper.ai/ja/dashboard/paper/89935bb5-f23d-4a79-ad9b-4155d2809dbf |
| primary | Taste: personalization and evaluating alignment with a specific human | 5 | https://arxiv.org/pdf/2411.00027 |
| primary | Skeptical: limits and failure modes of agent memory | 5 | https://arxiv.org/abs/2602.11988 |
| primary | Skeptical: limits and failure modes of agent memory | 3 | https://arxiv.org/abs/2310.01798 |
| primary | Skeptical: limits and failure modes of agent memory | 4 | https://arxiv.org/abs/2606.04329 |
| primary | Skeptical: limits and failure modes of agent memory | 5 | https://research.trychroma.com/context-rot |

Stats: `{"angles": 5, "sourcesFetched": 23, "claimsExtracted": 103, "claimsVerified": 25, "confirmed": 23, "killed": 2, "unverified": 0, "afterSynthesis": 6, "urlDupes": 0, "budgetDropped": 7, "agentCalls": 105}`
