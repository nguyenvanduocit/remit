# Counterarguments to "maintain critical thinking with agents", and evidence on interventions that preserve critical engagement

Scope note: stress-test of Addy Osmani's 2026 "cognitive offloading → cognitive surrender" article and its recommendations (hypothesis first; ask what would make it wrong; multi-model opposite-side debate; ask agent for its decisions/assumptions; quiz before merge; "blind spot pass"; weekly no-agent practice).

Research limits (stated so the writer does not overclaim): about 30 tool calls in two passes. WebSearch stopped working after the first 4 queries ("Not logged in"), and Semantic Scholar returned HTTP 429, PNAS returned 403, and the WUSTL host refused the connection. The remaining evidence came from direct fetches of arXiv, PMC, Wikipedia and vendor pages. Several canonical sources were not re-verified this session. They are listed under **Gaps**, never under Cited Findings.

Evidence-strength labels used:
- **STRONG**: RCT, preregistered experiment or meta-analysis, peer-reviewed.
- **MODERATE**: a single peer-reviewed experiment, a small RCT, or a working paper with an experimental design.
- **WEAK**: a small exploratory study, a simulation, an encyclopedia summary of contested literature, or expert opinion.
- **NONE**: anecdote or practitioner advice with no reported evaluation.

## (a) What are the strongest counterarguments to the thesis?

### Takeaway
The strongest counterargument is empirical, not rhetorical. Where AI alone already beats humans, adding a human in the loop makes the result *worse* on average (meta-analysis, g = −0.54). Field experiments in medicine and radiology show physicians failing to capture AI gains, either by underweighting the AI or by adding noise. The "moral panic" and "obsolete scaffolding" arguments are weaker and partly unverified here. The research also contains a sharp counter-counterargument: AI use measurably damages skill formation for learners. The thesis therefore holds strongly for *learning* and weakly for *decisions where AI is already superior*.

### Cited Findings

**Human + AI often underperforms the better of the two**
- A meta-analysis of 370 effect sizes from 106 experiments (2020–Jun 2023) found human–AI combinations performed significantly worse than the best of human or AI alone: g = −0.23 (95% CI −0.39 to −0.07). The combinations still beat humans alone: g = 0.64. **STRONG** — [Vaccaro, Almaatouq, Malone 2024, Nature Human Behaviour (PMC full text)](https://pmc.ncbi.nlm.nih.gov/articles/PMC11659167/)
- The key moderator: when **AI outperformed humans alone, synergy was g = −0.54** (CI −0.71 to −0.37). When **humans outperformed AI alone, synergy was g = +0.46** (CI 0.28 to 0.66). **STRONG** — [Vaccaro et al. 2024](https://pmc.ncbi.nlm.nih.gov/articles/PMC11659167/)
- Decision tasks (about 85% of effect sizes) showed losses, g = −0.27. Creation tasks showed a non-significant gain, g = +0.19 (p = 0.18). **STRONG** — [Vaccaro et al. 2024](https://pmc.ncbi.nlm.nih.gov/articles/PMC11659167/)
- Neither AI explanations nor AI confidence displays significantly moderated human–AI performance in the meta-analysis. **STRONG (null result; effect sizes only in the SI, not read)** — [Vaccaro et al. 2024](https://pmc.ncbi.nlm.nih.gov/articles/PMC11659167/)

**Clinical diagnosis: the physician-plus-LLM pair did not beat the LLM**
- In an RCT of 50 physicians (26 attendings, 24 residents), access to GPT-4 did not significantly improve diagnostic reasoning. The scores were a median of 76% vs 74%, an adjusted difference of 2 pp (95% CI −4 to 8, P = .60). **STRONG (RCT; small n)** — [Goh et al. 2024, JAMA Netw Open 7(10):e2440969, via summary](https://www.consultantlive.com/view/using-language-models-as-diagnosis-aid-doesn-t-improve-physician-accuracy); [paper PDF mirror](https://villadonatello.com/wp-content/uploads/2025/01/goh_2024_oi_241182_1729527081.343.pdf)
- In a secondary exploratory analysis, the **LLM alone scored 16 pp higher** than the conventional-resources group (95% CI 2–30, P = .03). Secondary coverage disagrees on the exact gap (14 pp, or a "92% median" for the model). **MODERATE (exploratory secondary outcome)** — [news-medical summary](https://www.news-medical.net/news/20241028/Study-reveals-AIs-potential-and-pitfalls-in-medical-diagnosis.aspx); [lifespan.io](https://lifespan.io/ai-outperforms-ai-assisted-doctors-in-diagnostic-reasoning/)
- Commentators note that the trial did not document *how* physicians used the AI, so non-use and over-anchoring on their own view cannot be distinguished. **WEAK (commentary)** — [clinician.com](https://www.clinician.com/articles/examining-the-effectiveness-of-using-ai-tools-in-practice)

**Radiology: humans systematically mis-weight AI**
- An experiment with 227 professional radiologists found that AI predictions did not reliably improve diagnoses. Radiologists **underweighted** the AI and treated their own signal and the AI's as independent. The authors conclude that, unless these belief-updating errors are fixed, the best policy assigns cases to *either* humans *or* AI, rarely to humans assisted by AI. **MODERATE–STRONG (large expert experiment; NBER working paper)** — [Agarwal, Moehring, Rajpurkar, Salz, NBER w31422](https://www.nber.org/system/files/working_papers/w31422/w31422.pdf); [SSRN abstract](https://papers.ssrn.com/abstract=4505053)
- This cuts **against** the article's framing. The radiologists' failure mode was *too much* independent judgement (automation neglect), not surrender. "Keep thinking critically" can deepen the bias that actually cost accuracy. **Inference from the above, MEDIUM.**

**Centaur chess**
- In 2013, Tyler Cowen and others assessed that engine progress made a major centaur advantage over engines alone hard to see, and unlikely to last. At the 2017 Infinity Chess Ultimate Challenge, an engine (Zor) finished first and the top centaur placed third. Kasparov claimed the same year that a good operator still adds value. **WEAK (encyclopedia summary; no controlled data)** — [Wikipedia: Advanced chess](https://en.wikipedia.org/wiki/Advanced_chess)
- I found no rigorous dataset showing centaurs vs top engines (Stockfish or Leela era) win rates. The "centaurs no longer beat engines" claim is widely repeated but weakly documented. — see Gaps.

**Historical moral panics: the Google effect's replication record**
- Sparrow et al. 2011 ("Google effects on memory") **failed to replicate** in Camerer et al. 2018 (SSRP, 21 Nature/Science studies). Hesselmann 2020 also found "no conclusive evidence". Schooler & Storm 2021 replicated it only when participants had prior experience that saved information stays accessible. A 2024 meta-analysis (Gong & Yang, 35 studies) found frequent search associated with changed information processing. **WEAK–MODERATE (secondary summary; mixed literature)** — [Wikipedia: Google effect](https://en.wikipedia.org/wiki/Google_effect)
- The implication for the article: a canonical "tools rot memory" finding is fragile. Offloading may be a *rational* allocation of memory when access is reliable, which is the Schooler & Storm condition. **Inference, MEDIUM.**

**Counter-counterargument: AI use does harm skill formation (the thesis's strongest support)**
- In an RCT, 52 mostly junior engineers learned the Trio async library with or without an AI assistant. On a later quiz the AI group scored about 50% and the hand-coding group about 67% (p = 0.01). The largest gap was in debugging. The speed gain of about 2 minutes was **not significant** (p = 0.391). **MODERATE (small RCT, vendor-run)** — [Anthropic, "How AI assistance impacts the formation of coding skills," Jan 29 2026](https://www.anthropic.com/research/AI-assistance-coding-skills)
- High scorers (≥65%) used AI to build understanding: generate-then-ask-why (n=2, ~86%), code with explanations (n=3, ~68%), and conceptual questions only with errors fixed alone (n=7, ~65%). Low scorers (<40%) delegated, relied on AI progressively more, or debugged iteratively through AI. The authors say these associations are not causal. **WEAK (tiny subgroups, correlational)** — [Anthropic 2026](https://www.anthropic.com/research/AI-assistance-coding-skills)

**Moral panic precedent: Socrates on writing**
- In Plato's *Phaedrus*, King Thamus answers Theuth that writing is "a remedy for reminding, not remembering," giving "the appearance but not the reality of wisdom". Written texts "cannot speak, answer questions, or come to their own defense." This is the archetype of the cognitive-tool panic. Note that the "cannot answer questions" part of the critique does *not* apply to interactive AI, which weakens the analogy in both directions. **WEAK (primary text via encyclopedia summary)** — [Wikipedia: Phaedrus (dialogue)](https://en.wikipedia.org/wiki/Phaedrus_(dialogue))

**Bitter lesson**
- Sutton (13 Mar 2019): general methods that scale with computation tend to outperform methods built on human domain understanding. Applied here, human-side scaffolding (forcing functions, verification rituals) is the kind of hand-built structure that capability growth tends to erode. **WEAK (opinion essay, via secondary summary; the primary page failed with a TLS error)** — [Wikipedia: The Bitter Lesson](https://en.wikipedia.org/wiki/The_Bitter_Lesson)

**Perceived vs measured benefit (supports the thesis)**
- An RCT with 16 experienced open-source developers (246 tasks; Cursor Pro, Claude 3.5/3.7 Sonnet) found **AI increased completion time by 19%**. Developers had forecast a 24% speed-up and afterwards still believed they were 20% faster. Experts forecast speed-ups of 38–39%. **MODERATE (RCT, small n, early-2025 tools)** — [Becker, Rush, Barnes, Rein (METR), arXiv 2507.09089](https://arxiv.org/abs/2507.09089)
- A survey of 319 knowledge workers (936 examples) found "higher confidence in GenAI is associated with less critical thinking", while higher self-confidence was associated with more. GenAI shifted critical thinking toward verification, integration and task stewardship. **MODERATE (self-report survey, correlational; CHI 2025)** — [Lee, Sarkar, Tankelevitch et al., Microsoft Research](https://www.microsoft.com/en-us/research/publication/the-impact-of-generative-ai-on-critical-thinking-self-reported-reductions-in-cognitive-effort-and-confidence-effects-from-a-survey-of-knowledge-workers/)
- The METR result shows that people's own sense of whether AI helps is unreliable. This undercuts the "the models are better, so just defer" counterargument as a *self-assessed* policy: users cannot judge AI superiority by feel. **Inference, MEDIUM.**

### Inferences
- **MEDIUM:** The article's "the models will be better at this than me" objection is backed by the best available evidence (Vaccaro g = −0.54 when AI > human). Telling people to "stay critical" without telling them **when** to defer is likely to *lower* accuracy in AI-superior decision tasks. Critical engagement must be paired with calibrated deference, not equated with overriding.
- **MEDIUM:** Two different goals get conflated. (1) *Output quality now*: here human overrides of a superior AI often subtract value. (2) *Human capability later* (learning, debugging, catching out-of-distribution failures): here offloading demonstrably erodes skill (Anthropic RCT). The article's advice is defensible for (2) and only conditionally for (1).
- **MEDIUM:** The surrender framing predicts over-acceptance. Radiology (Agarwal) shows the opposite error (under-weighting AI) among experts. Expertise level probably flips which error dominates: novices surrender, experts neglect.
- **LOW:** In agentic coding, the AI is not uniformly superior. Agents fail on repo-specific context and intent, which is the "humans hold contextual information" result of Agarwal. This is where human verification keeps positive value even as base error rates fall.

### Gaps
- **Calculator research** (Hembree & Dessart 1986 meta-analysis): unverified. JSTOR returned a page with no content, and ERIC refused the connection (ECONNREFUSED).
- **Bastani et al., "Generative AI without guardrails can harm learning"** (PNAS 2025): unverified, so its numbers are not stated here. PNAS and SSRN returned 403, Semantic Scholar returned 429 then 403, and the arXiv search returned 0 results.
- **Sutton's Bitter Lesson primary page** failed with a TLS "self signed certificate" error. It is cited via Wikipedia only.
- No rigorous centaur-vs-engine win-rate data was found.
- No study measures "cognitive surrender" as distinct from "overreliance" in agentic (multi-step, tool-using) settings. Nearly all the evidence comes from single-shot classification or diagnosis tasks.

## (b) Which concrete interventions actually preserve critical engagement?

### Takeaway
Only **decide-first / cognitive forcing** has good controlled evidence for reducing overreliance, and it comes at a cost: users dislike it, and it helps mainly people high in Need for Cognition. **Explanations backfire**: they raise acceptance regardless of correctness. Devil's-advocate and deliberative AI designs have only small exploratory evidence. Quizzing and retrieval practice have strong evidence for *retention* (cognitive psychology) but none for catching bugs before merge. The "blind spot pass" and weekly no-agent practice have **no evaluation at all**.

### Cited Findings

**Cognitive forcing functions: decide before seeing the AI (closest to "form a hypothesis before asking")**
- In an experiment with 199 participants, cognitive forcing designs significantly reduced overreliance compared with simple explainable-AI designs. Participants rated the most effective designs **least favourably**. The interventions did not eliminate overreliance, and they helped high Need-for-Cognition participants more. **MODERATE–STRONG (peer-reviewed experiment, CSCW)** — [Buçinca, Malaya, Gajos, CSCW 2021 (PACM HCI 5, CSCW1, Art. 188)](https://www.eecs.harvard.edu/~kgajos/papers/2021/bucinca2021trust.shtml); [arXiv 2102.09692](https://arxiv.org/abs/2102.09692v1)
- Cognitive forcing functions are defined there as interventions applied at decision time that disrupt heuristic reasoning. Examples include checklists, diagnostic time-outs, and **asking the person to explicitly rule out an alternative**. The last one directly maps to "ask what would make it wrong". — [Buçinca et al. 2021 via search summary](https://www.eecs.harvard.edu/~kgajos/papers/2021/bucinca2021trust.shtml)

**AI explanations: asking the agent to explain its decisions can increase overreliance**
- With an AI of human-comparable accuracy, teams achieved complementary performance, but **explanations did not increase it**. Instead, "explanations increased the chance that humans will accept the AI's recommendation, regardless of its correctness." **MODERATE–STRONG (CHI 2021, three datasets)** — [Bansal et al., "Does the Whole Exceed its Parts?", CHI 2021, arXiv 2006.14779](https://arxiv.org/abs/2006.14779)
- This is consistent with the meta-analytic null for explanations. **STRONG** — [Vaccaro et al. 2024](https://pmc.ncbi.nlm.nih.gov/articles/PMC11659167/)
- The implication for the article's "ask the agent for decisions/assumptions it made": a fluent list of assumptions may function like an explanation and *raise* trust. It helps only if the human actively checks each assumption against independent knowledge. **Inference, MEDIUM.**

**Deliberative and devil's-advocate AI (closest to "have models argue the opposite side")**
- "Deliberative AI" elicits the human's dimension-level opinions first, then discusses disagreements via an LLM. In a graduate-admissions task it beat conventional XAI on appropriate reliance and on task performance. **WEAK–MODERATE (exploratory evaluation, CHI 2025)** — [Ma et al., arXiv 2403.16812](https://arxiv.org/abs/2403.16812)
- An LLM devil's advocate was tested in 12 four-person groups (n=48) with power imbalance. It raised process and outcome satisfaction, most for minority members, with a non-significant workload increase. It measured satisfaction, **not decision accuracy**. **WEAK** — [Lee, Hwang, Kim, Lee 2025, arXiv 2503.14263](https://arxiv.org/abs/2503.14263)
- In a multi-agent GPT-4 simulation with a devil's-advocate role on 16 diagnostic case reports, accuracy rose from 0/80 initial diagnoses to 57/80 for the top differential after discussion. An initial accuracy of 0% suggests the cases were chosen because they are commonly misdiagnosed (my inference, not checked against the paper), and no humans were involved. **WEAK (simulation, likely selection bias)** — [Ke et al. 2024, arXiv 2401.14589](https://arxiv.org/abs/2401.14589)
- LLMs conform to majority answers, more so when uncertain. Devil's Advocate prompting is proposed as a mitigation, with no numeric results in the abstract. This is a caution that "multiple models arguing" may converge rather than truly disagree. **WEAK** — [Zhu et al., arXiv 2410.12428](https://arxiv.org/abs/2410.12428)

**Quiz yourself / retrieval practice**
- Roediger & Karpicke 2006, "Test-Enhanced Learning" (Psychological Science), is the canonical testing-effect paper. The pretesting effect (testing *before* learning, with feedback) is reported to improve later learning in lab and classroom settings, with mixed results against post-testing. Even unsuccessful retrieval attempts can enhance learning (the generation effect). **WEAK as cited here (encyclopedia summary; the article itself is flagged as lacking effect sizes)** — [Wikipedia: Testing effect](https://en.wikipedia.org/wiki/Testing_effect)
- The pretesting result supports "form a hypothesis before asking" as a *learning* intervention, independent of its effect on decision accuracy. **Inference, MEDIUM.**

**Usage pattern instead of friction**
- In the Anthropic RCT, learners who asked conceptual "why" questions, or who fixed errors themselves, kept most of the learning (about 65–86%, vs under 40% for delegators). **WEAK (n of 2–7 per pattern, correlational)** — [Anthropic 2026](https://www.anthropic.com/research/AI-assistance-coding-skills)

**Blind spot pass**
- The Claude blog's field guide recommends a "blind spot pass" (ask Claude for your unknown unknowns, with context about what you know), along with interviews, plans that lead with decisions likely to change, implementation notes, and quizzes before merging. **The post cites no evidence**: it rests on the author's experience. **NONE** — [Claude blog, "A field guide to Claude Fable: finding your unknowns"](https://claude.com/blog/a-field-guide-to-claude-fable-finding-your-unknowns)

### Inferences
- **MEDIUM:** Evidence ranking of the article's seven recommendations:

| Recommendation | Closest evidence | Strength | Verdict |
|---|---|---|---|
| Form a hypothesis before asking | Cognitive forcing (Buçinca); pretesting effect | MODERATE–STRONG | Best supported. Costs satisfaction; benefits are uneven |
| Ask what would make it wrong | "Rule out an alternative" forcing function (Buçinca) | MODERATE (indirect) | Plausible |
| Multiple models argue the opposite side | Deliberative AI (Ma), LLM devil's advocate (Lee), simulation (Ke); LLM conformity (Zhu) | WEAK | Unproven for accuracy; models may converge |
| Ask the agent for its decisions/assumptions | Explanations raise acceptance regardless of correctness (Bansal; Vaccaro null) | MODERATE–STRONG, **against** | Risky. Helps only with active independent checking |
| Quiz yourself before merging | Testing effect (retention) | STRONG for memory, NONE for defect-catching | Good for understanding; unproven as a quality gate |
| Blind spot pass | Vendor blog | NONE | Untested |
| Weekly no-agent practice | Skill-formation RCT (indirect) | WEAK–MODERATE (indirect) | Plausible for skill upkeep; no direct test |

- **MEDIUM — when the advice holds:**
  - The human is learning a new domain or library (Anthropic RCT).
  - The human holds context the AI lacks (Agarwal).
  - The task is creative or open-ended rather than a well-specified decision (Vaccaro).
  - Errors are costly and can fall outside the AI's training distribution.
  - The human is a novice prone to surrender.
- **MEDIUM — when it is overkill or wrong:**
  - The AI is demonstrably more accurate on a well-specified decision task (Vaccaro g = −0.54; Goh). Here the rational policy is to defer, or to route the case to AI or human, not to combine them.
  - The human is an expert prone to *neglecting* the AI (Agarwal), where "think critically" amplifies the bias.
  - Explanation-style prompts produce fluent rationales that inflate trust (Bansal).
  - Users with low Need for Cognition (Buçinca), for whom forcing functions add friction with little benefit.
- **LOW:** A better reframing than "maintain critical thinking" would be "calibrate reliance": measure the AI's accuracy on your task, defer where it is superior, and spend scarce attention on the cases where you hold unique context, or where AI confidence and agreement are low.

### Gaps
- No controlled study tests any of these interventions in **agentic software development** (multi-file changes, long-horizon agents). All the decision-aid evidence comes from classification or diagnosis tasks.
- No study was found that measures whether pre-merge quizzes catch defects, or whether weekly no-agent practice preserves skill.
- Roediger & Karpicke's one-week retention numbers and the meta-analytic effect sizes (Rowland 2014; Adesope et al. 2017) could not be retrieved. The WUSTL PDF refused the connection (ECONNREFUSED), SAGE returned 403, PubMed served a cookie wall, and Wikipedia lacks the numbers.
- The Bastani et al. guardrailed-tutor result would be the most directly relevant intervention evidence for Socratic AI designs, but it is unverified (see the errors under (a) Gaps).
- Long-term durability is unknown: whether forcing-function benefits persist, or whether users route around friction they dislike.
