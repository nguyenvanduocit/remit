# Empirical evidence for "cognitive offloading → deferral → cognitive surrender" with AI/LLMs (as of 2026-11-04)

How to read these notes:
- Evidence-strength labels: **RCT** (randomized, controlled), **LAB** (controlled experiment, often online and short), **SURVEY** (cross-sectional, self-report), **OBS-EEG** (small neuro or observational study), **OPINION** (essay or theory).
- Status labels: **PEER-REVIEWED** or **PREPRINT**.
- Bibliographic data (title, authors, journal, volume, pages) of every classic paper was confirmed through the Crossref API. Content was confirmed per paper from its abstract (Crossref or Europe PMC) or a secondary source, named on each line. Publisher full texts (Nature, Taylor & Francis, Elsevier, SSRN) returned paywalls or 403s. PubMed is blocked by network policy. Semantic Scholar returned 429.

## 1. What do Shaw & Nave define and measure? (method, sample, effect sizes, limitations)

### Takeaway
Shaw & Nave ran three preregistered online experiments (N = 1,372; about 9,600 trials) with an AI whose answers were secretly made right or wrong. When it was wrong, people mostly followed it: accuracy fell about 15 points below a no-AI baseline, and confidence still rose. The design is strong for showing that people defer to a planted wrong answer on trick questions. It says nothing about longer-term "drift", and the paper is still a preprint. Evidence: **LAB, PREPRINT**.

### Cited Findings
- **Paper and status.** Shaw, S. D. & Nave, G., "Thinking—Fast, Slow, and Artificial: How AI is Reshaping Human Reasoning and the Rise of Cognitive Surrender", Wharton School research paper on SSRN (abstract_id 6097646). Dated Jan 11, 2026, posted Feb 2, 2026, revised Feb 10, 2026. As of Aug 13, 2026 it had not completed peer review. — [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason); [SSRN listing](https://papers.ssrn.com/abstract=6097646) (the SSRN page returns 403 to the fetcher; the abstract was read through the tagteam.harvard.edu feed mirror).
- **Definitions.** The authors propose a "Tri-System Theory": System 3 is artificial cognition outside the brain, added to Kahneman's System 1 and System 2. "Cognitive surrender" is adopting an AI's output with minimal scrutiny, overriding both intuition and deliberation. — [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason); [WebSearch summaries of SSRN, TNW, The Capitol Forum](https://thecapitolforum.com/?p=5364)
- **Task.**
  - Items come from an adapted Cognitive Reflection Test, where the intuitive answer is wrong.
  - Participants could consult an AI assistant. The researchers secretly set each AI answer to correct or incorrect, about half each.
  - Participants chose to consult the AI on most items.
  - Source: [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason)
- **Sample.** Three preregistered experiments, N = 1,372, 9,593 trials. Participants consulted the AI on more than half of trials. The abstract gives no rate of accepted faulty answers; the 73% and 80% figures come from the paper body and press. — [SSRN abstract via tagteam.harvard.edu](https://tagteam.harvard.edu/hub_feeds/3180/feed_items/18336747) (fetched 2026-11-04)
- **Main effect (Study 1).** Compared with no AI, accuracy rose about 25 percentage points when the AI was correct and fell about 15 points when it was wrong, a gap of about 40 points. The authors call the effect large. — [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason)
- **Acceptance of wrong AI answers.**
  - Ars Technica (fetched through a mirror) reports that users accepted faulty AI reasoning "80 percent of the time" on trials where the AI was wrong, against about 93% for correct AI. Participants consulted the AI on about 50% of problems. — [Ars Technica mirror](https://drwebdomain.blog/2026/04/04/cognitive-surrender-leads-ai-users-to-abandon-logical-thinking-research-finds-ars-technica/)
  - A search-result summary attributed 73.2% to Ars, but the fetched Ars text says 80%, so that attribution was wrong.
  - Other outlets report about 79.8–80%, likely from a different denominator or study. — [ecosistemastartup](https://ecosistemastartup.com/?p=89813); [gentic.news](https://www.gentic.news/article/wharton-study-cognitive-surrender-to-ai-leads-to-79-8-error-adoption-rate-underm) (this second outlet looks AI-generated and cites a different title).
  - Addy Osmani uses 73%. — [Addy Osmani blog](https://addyosmani.com/blog/cognitive-surrender/)
  - **Unresolved.** 73% vs 80% probably reflects a different denominator (all wrong-AI trials vs only trials where the AI was consulted, or a pooled vs a single study). It can only be settled from the paper, and SSRN returns 403 to the fetcher. Also tried: the SSRN PDF delivery URL (403), the Wharton faculty page (404), and Wharton Executive Education's write-up, which gives only the +25/−15 point accuracy changes and no acceptance rate. Cite "about three-quarters to four-fifths" with the source named, or read the PDF first.
- **Confidence.** AI use raised confidence even after errors. One secondary report puts the rise at about 11.7 percentage points, although the AI was wrong half the time. — [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason); [WebSearch summary of pulse.bot / Ars coverage](https://www.pulse.bot/ai/news/cognitive-surrender-leads-ai-users-to-abandon-logical-thinking-research-finds-a5c5954d-c8a5-489f-8e83-2cde3ceaf6b5) (the 11.7 figure is secondary).
- **Moderators.**
  - Time pressure (Study 2) and per-item incentives with feedback (Study 3) shifted performance but did not remove the pattern. — [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason)
  - Secondary reports put incentives plus feedback at about +19 points in successful overrides of faulty AI, and a 30-second timer at about −12 points in corrections. — [Ars Technica mirror](https://drwebdomain.blog/2026/04/04/cognitive-surrender-leads-ai-users-to-abandon-logical-thinking-research-finds-ars-technica/) (secondary; one summary instead says incentives "roughly doubled" rejections, which is inconsistent).
- **Who surrenders most.** People with higher trust in AI, lower need for cognition and lower fluid intelligence. — [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason)
- **Critiques.**
  - By the paper's own definition, "System 3" also covers old technologies such as books and calculators, so the renumbering adds labeling more than mechanism.
  - The individual-difference results can be misread as gullibility, when they more plausibly reflect a varying disposition to check.
  - Critics of dual-process models in general do not dispute the measured effect.
  - Source: [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason)
- **No replication found.** No independent 2026 replication of the cognitive surrender findings was found. — [WebSearch, "cognitive surrender replication 2026"](https://thedecisionlab.com/insights/technology/cognitive-surrender-from-tool-to-thinker)
- **Mismatch with Addy's framing.**
  - Addy's article defines offloading as handing off the "how" while keeping judgment over the "what", and surrender as having no independent view to compare against.
  - It does **not** describe a three-stage offloading → deferral → surrender sequence or an accuracy-driven "drift". Its closest mechanisms are "borrowed confidence", "path dependence" (each accepted chunk makes the next surrender likelier) and "comprehension debt".
  - Source: [Addy Osmani blog](https://addyosmani.com/blog/cognitive-surrender/) (fetched 2026-11-04)

### Inferences
- **MEDIUM:** CRT items are built so that the intuitive answer is wrong. A confident AI answer gives people a ready way out of the slow, effortful thinking the items require. This probably maximizes surrender compared with tasks where people hold real domain expertise, so 73–80% is likely an upper bound, not a typical workplace rate.
- **MEDIUM:** The AI was wrong 50% of the time, far more than a production LLM on most tasks. Participants therefore surrendered to a source that was visibly unreliable. That is strong evidence for deference, but it does not test the "trust grows as accuracy rises" drift.
- **HIGH:** The study is a single-session, between-condition design. It measures momentary deference, not a progression over time. Any "offloading → deferral → surrender" staging is a theoretical extrapolation, not something Shaw & Nave measured.

### Gaps
- The exact acceptance rate of wrong answers per study and its denominator: SSRN returned 403 and the PDF was not read.
- Per-study N, the participant pool (Prolific?), the AI implementation (live LLM or scripted answers) and payment levels were not verified.
- Whether a journal version exists by Nov 2026 is unknown. The last confirmed status is preprint (Aug 2026).

## 2. Other key studies: Lee et al. CHI 2025, Gerlich 2025, Kosmyna et al. 2025 (MIT), Bastani et al. PNAS 2025, 2026 work

### Takeaway
The only randomized evidence of real harm is Bastani et al. (PNAS, about 1,000 students) and Anthropic's Shen & Tamkin RCT (N = 52, preprint). Both show that **unguided** AI use during practice lowers later unaided performance, and that guarded or conceptual use largely avoids the harm. Lee et al. and Gerlich are cross-sectional self-report correlations. The MIT EEG study is a small preprint with a published methodological critique.

### Cited Findings
**Lee et al., CHI 2025 (Microsoft Research + CMU): SURVEY, PEER-REVIEWED (CHI)**
- **Paper.** "The Impact of Generative AI on Critical Thinking: Self-Reported Reductions in Cognitive Effort and Confidence Effects From a Survey of Knowledge Workers", DOI 10.1145/3706598.3713778. — [PDF](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf); [MSR](https://www.microsoft.com/en-us/research/?p=1135061)
- **Sample.** 319 knowledge workers recruited on Prolific gave 936 first-hand examples of GenAI use. — [PDF, abstract and §3](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf)
- **How often people say they think critically.** Participants reported some critical-thinking activity in about 60% (555/936) of examples. — [PDF §4.2](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf)
- **Predictors of perceived critical thinking.**
  - Confidence in the AI doing the task: β = −0.69, p < 0.001.
  - Self-confidence in doing the task: β = 0.26, p = 0.026.
  - Confidence in evaluating AI output: β = 0.31, p = 0.046.
  - General tendency to reflect on one's work: β = 0.52, p < 0.001.
  - Overall trust in GenAI: not significant.
  - Task type: no main effect.
  - Source: [PDF §4.2.1–4.2.2](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf)
- **Perceived effort.** Higher confidence in the AI went with a larger perceived drop in effort for five of six Bloom activities. Evaluation showed the largest drop (β = −0.23, p < 0.001). — [PDF §5.1.1](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf)
- **Barriers.** Some participants reported barriers to inspecting AI output, for example lacking domain knowledge (58/319). Some found AI responses hard to improve (72/319). Some said trust and reliance discouraged them from reflecting (83/319). — [PDF §4.3](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf)
- **Limitations stated by the authors.** Participants sometimes conflated reduced effort in using GenAI with reduced critical thinking. Everything is self-reported, with no pre/post measure and no random assignment. — [PDF Limitations](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf); [WebSearch summary](https://scienceblog.com/t-ai-confidence-less-critical-thinking-319-knowledge-workers/)

**Gerlich 2025, *Societies* 15(1):6: SURVEY + interviews, PEER-REVIEWED (MDPI)**
- **Sample.** 666 participants across age and education groups.
- **Main finding.** A significant negative correlation between frequent AI-tool use and critical-thinking scores, mediated by cognitive offloading.
- **Age and education.** Younger participants were more dependent and scored lower. Higher education predicted better critical thinking regardless of AI use.
- Source: [DOAJ](https://doaj.org/article/d5c17dd91ce64775a065095d8af730ce); [SSRN 5082524](https://papers.ssrn.com/abstract=5082524); DOI 10.3390/soc15010006
- **Press amplification.** Popular coverage turned the correlation into "AI is making us all dumber". — [Tech.co](https://tech.co/news/study-ai-making-us-dumber)
- No formal correction or published critique was found in this search. — [WebSearch results](https://econpapers.repec.org/RePEc:gam:jsoctx:v:15:y:2025:i:1:p:6-:d:1559787)

**Kosmyna et al. 2025, "Your Brain on ChatGPT" (MIT Media Lab): OBS-EEG, PREPRINT (arXiv 2506.08872)**
- **Design.** 54 participants took part in Sessions 1–3 of essay writing (LLM, search engine, or brain-only). Only 18 completed the crossover Session 4. The authors called the conclusions preliminary. — [arXiv 2506.08872](https://arxiv.org/abs/2506.08872); [WebSearch summary](https://maketecheasier.com/?p=860959)
- **Claims.** Reduced neural connectivity in the LLM group, weaker recall of their own essays, and the label "cognitive debt". — [Addy Osmani blog](https://addyosmani.com/blog/cognitive-surrender/); [MIT Media Lab](https://www.media.mit.edu/publications/your-brain-on-chatgpt/)
- **Formal critique.** Stankovic, Hirche, Kollatzsch & Doetsch, "Comment on: Your Brain on ChatGPT…", arXiv 2601.00856 (Dec 29, 2025). They cite small N and low statistical power, reproducibility doubts, problems in the EEG methodology (ambiguous criteria for "significant connections"), inconsistent reporting (55 completed but 54 analysed) and low transparency. They also suggest some results could be read more conservatively. — [arXiv 2601.00856](https://arxiv.org/abs/2601.00856v1)
- **Author's own caution.** Kosmyna stated the study did not measure IQ, and that EEG reflects engagement during the task, not general intelligence. — [WebSearch summary](https://maketecheasier.com/?p=860959)

**Bastani et al. 2025, PNAS 122(26) e2422633122: RCT (field), PEER-REVIEWED**
- **Design.** About 1,000 Turkish high-school math students were given either GPT Base (a plain ChatGPT-like interface) or GPT Tutor (prompts with learning safeguards).
- **Main result.** When access was removed, GPT Base students did worse than students who never had access: 17% lower exam grades.
- **Guardrails.** GPT Tutor largely mitigated the harm.
- **Mechanism.** Students used GPT Base as a crutch during practice.
- Source: [EconPapers / PNAS abstract](https://econpapers.repec.org/RePEc:nas:journl:v:122:y:2025:p:e2422633122); [Wharton Knowledge](https://knowledge.wharton.upenn.edu/article/without-guardrails-generative-ai-can-harm-education); [CHIBE](https://chibe.upenn.edu/publications/generative-ai-without-guardrails-can-harm-learning-evidence-from-high-school-mathematics)
- **Practice-session gains.** The abstract reports gains while access lasted: "48% improvement in grades for GPT Base and 127% for GPT Tutor". It gives the sample only as "nearly a thousand" students. — [EconPapers / PNAS abstract](https://econpapers.repec.org/RePEc:nas:journl:v:122:y:2025:p:e2422633122)
- **Critique of scope.** One school system, one subject, four 90-minute sessions. — [liveinthefuture.org](https://liveinthefuture.org/stories/ai-makes-students-worse)

**Shen & Tamkin (Anthropic) 2026, "How AI Impacts Skill Formation": RCT, PREPRINT (arXiv 2601.20245, v1 Jan 28, 2026, v2 Feb 1, 2026)** — authors and dates from [arXiv](https://arxiv.org/abs/2601.20245)
- **Design.** 52 mostly junior engineers learned the Trio async library, with or without an AI sidebar.
- **Quiz scores.** 50% for the AI group vs 67% for hand coding: 17 percentage points lower, Cohen's d = 0.738, p = 0.01. The biggest gap was on debugging questions.
- **Speed.** About 2 minutes faster with AI, not significant (p = 0.391).
- **Interaction patterns.**
  - Conceptual inquiry (n = 7) and generation-then-comprehension (n = 2) kept scores at 65% or higher.
  - Full delegation (n = 4), progressive reliance (n = 4) and AI debugging (n = 4) scored below 40%.
  - These patterns are associations, not causal.
- **Limitations.** Comprehension measured immediately (no retention test), no agentic tooling, one task.
- Source: [Anthropic research page](https://www.anthropic.com/research/AI-assistance-coding-skills)
- **"17%" wording.** Addy quotes this as "17% lower". It is 17 percentage points (50 vs 67), which is about 25% relative. — [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills); [Addy Osmani blog](https://addyosmani.com/blog/cognitive-surrender/)

### Inferences
- **HIGH:** The randomized studies (Bastani; Shen & Tamkin) agree on one point. *How* AI is used predicts the learning cost. Delegating answers hurts; conceptual or scaffolded use does not measurably hurt. This supports Addy's offloading/surrender distinction better than any survey does.
- **HIGH:** Lee et al. support a narrower claim than headlines suggest. Confidence in the AI goes with *self-reported* less critical thinking. Overall trust did not predict it. Self-confidence and the ability to evaluate output protect against it.
- **MEDIUM:** Gerlich's mediation model rests on cross-sectional self-report. Reverse causation (people with weaker critical thinking adopt AI more) and confounding by age and education are not ruled out. Treat it as a correlation.
- **HIGH:** Kosmyna et al. should not carry a "brain damage" claim. At most it shows lower task engagement and recall during LLM-assisted writing, in a small preprint.

### Gaps
- Bastani's exact N is not in the abstract ("nearly a thousand").
- Peer-review status of arXiv 2601.20245 (Anthropic) and arXiv 2506.08872 (Kosmyna) as of Nov 2026 is unknown.
- No 2026 replications of Lee et al. or Gerlich were found; search failed before a dedicated query could run.
- Addy cites an arXiv paper on "Cognitive Agency Surrender" proposing "Scaffolded Cognitive Friction", without a link. It was not located.

## 3. Classic precursors: automation bias and complacency, Ironies of Automation, the Google effect, cognitive offloading

### Takeaway
Decades-old human-factors research already established most of the mechanism: automation bias, complacency, out-of-the-loop skill loss, and offloading as a metacognitive choice. That literature is mostly peer-reviewed LAB evidence in aviation, medicine and process control, and fairly robust. The "Google effect" on memory is the weak link: its headline result failed replication.

### Cited Findings
- **Parasuraman & Manzey (2010), "Complacency and Bias in Human Use of Automation: An Attentional Integration", *Human Factors* 52(3):381–410, DOI 10.1177/0018720810376055.** It integrates two literatures:
  - Complacency: monitoring of automation falls when it is reliable.
  - Automation bias: errors of omission (missing a problem the aid did not flag) and of commission (following wrong advice).
  - Both appear in experts as well as novices, are not prevented by training or instructions alone, and are tied to attention allocation under multitasking.
  - Evidence: PEER-REVIEWED review. *[summary from Crossref abstract]* — [DOI](https://doi.org/10.1177/0018720810376055); [Crossref](https://api.crossref.org/works/10.1177/0018720810376055)
- **Parasuraman, Molloy & Singh (1993), "Performance Consequences of Automation-Induced 'Complacency'", *Int. J. Aviation Psychology* 3(1):1–23.** Operators are less likely to disengage from monitoring when the system's reliability can change. The classic reading, "constant high reliability breeds complacency", follows from this. Evidence: LAB, PEER-REVIEWED. *[content via Wikipedia's summary; metadata via Crossref]* — [DOI](https://doi.org/10.1207/s15327108ijap0301_1); [Wikipedia, Automation bias](https://en.wikipedia.org/wiki/Automation_bias)
- **Bainbridge (1983), "Ironies of Automation", *Automatica* 19(6):775–779.** Automation leaves the human two jobs: monitoring, which humans do poorly over long stretches, and taking over in rare abnormal cases, for which the automation has eroded their skills. Wikipedia's article on the paper says: "operators will not practice skills as part of their ongoing work"; "Their work now also includes exhausting monitoring tasks"; and operators "need to be trained more to be ready for the rare but crucial interventions." Evidence: OPINION / analytic essay. *[content via Wikipedia; metadata via Crossref; Elsevier full text 403]* — [DOI](https://doi.org/10.1016/0005-1098(83)90046-8); [Wikipedia, Ironies of Automation](https://en.wikipedia.org/wiki/Ironies_of_Automation)
- **Goddard, Roudsari & Wyatt (2012), "Automation bias: a systematic review of frequency, effect mediators, and mitigators", JAMIA 19(1):121–127.** Systematic review of automation bias in clinical decision support: 74 of 13,821 retrieved papers met inclusion criteria.
  - Mediators: user factors (cognitive style, task-specific experience), trust and confidence, and environment (workload, task complexity, time pressure).
  - Mitigators: training, emphasising user accountability, and design (placement of advice, updated confidence levels, information instead of recommendations).
  - Evidence: PEER-REVIEWED systematic review. *[abstract via Europe PMC]* — [DOI](https://doi.org/10.1136/amiajnl-2011-000089); [Europe PMC](https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:10.1136/amiajnl-2011-000089&resultType=core&format=json)
- **Risko & Gilbert (2016), "Cognitive Offloading", *Trends in Cognitive Sciences* 20(9):676–688.** Defines cognitive offloading as physical actions that reduce cognitive demand (tilting the head to read a rotated image, setting a phone reminder). It reviews what triggers offloading and what it does to cognition, within a metacognitive framework. *[abstract via Europe PMC]* The more specific claims — people offload more when they are less confident in their memory, whether or not that is accurate, and offloading can reduce memory for the offloaded content — are the review's well-known content from prior knowledge; the abstract does not state them. Evidence: PEER-REVIEWED review. — [DOI](https://doi.org/10.1016/j.tics.2016.07.002); [Europe PMC](https://www.ebi.ac.uk/europepmc/webservices/rest/search?query=DOI:10.1016/j.tics.2016.07.002&resultType=core&format=json)
- **Sparrow, Liu & Wegner (2011), "Google Effects on Memory", *Science* 333:776, DOI 10.1126/science.1207745.**
  - Camerer et al. (2018, *Nature Human Behaviour*, 21 Science/Nature replications) **failed** to replicate its "Google Stroop" result.
  - Sparrow objected to design differences: the original was run in 2006 with dated terms such as "Altavista" and "Lycos".
  - A 2020 German replication that took Sparrow's objections into account retried her first experiment and "failed to show the famed Google effect". The paper is Hesselmann, "No conclusive evidence that difficult general knowledge questions cause a 'Google Stroop effect'. A replication study", *PeerJ* 8:e10325 (Nov 6, 2020; Psychologische Hochschule Berlin). Its abstract (via Crossref) says it used a design closer to the original and found no conclusive evidence of the effect. — [McGill OSS](https://www.mcgill.ca/oss/article/critical-thinking-technology/digital-amnesia-has-been-exaggerated); [Crossref](https://api.crossref.org/works/10.7717/peerj.10325); [PeerJ](https://peerj.com/articles/10325)
  - The FORRT replication atlas records the 2018 failure ("the original result did not replicate") as the only listed replication. McGill names Sparrow's first experiment as the one that failed. No meta-analysis was found in either source, so a claimed 2024 meta-analysis is dropped as unverified. — [FORRT](https://forrt.org/flora-replication-atlas/doi/10.1126/science.1207745/); [McGill OSS](https://www.mcgill.ca/oss/article/critical-thinking-technology/digital-amnesia-has-been-exaggerated)
  - Some researchers question whether "transactive memory" with a computer is meaningful.
  - Sources: [FORRT replication atlas](https://forrt.org/flora-replication-atlas/doi/10.1126/science.1207745/); [PeerJ 10325](https://peerj.com/articles/10325); [McGill OSS](https://www.mcgill.ca/oss/article/critical-thinking-technology/digital-amnesia-has-been-exaggerated); [Wikipedia](https://www.wikipedia.org/wiki/Google_effect)

### Inferences
- **HIGH:** Shaw & Nave's "cognitive surrender" is close to what human factors calls *commission errors under automation bias*, measured with an LLM and reasoning items. The new label is not a new phenomenon. An article that calls it new would overstate novelty.
- **MEDIUM:** Bainbridge's irony maps directly onto agentic coding. The better the agent, the rarer and harder the human's review cases become, and the less practice the human has for them. Shen & Tamkin's largest gap, on debugging, fits this.
- **HIGH:** The "Google effect" should not be cited as solid evidence that tools erode memory. Its signature result failed replication twice.

### Gaps
- Risko & Gilbert's specific confidence and memory claims rest on prior knowledge beyond the abstract. The Elsevier full text is paywalled, Semantic Scholar returned 429 three times, and Wikipedia's page does not cite the paper. Everything else in this section is backed by an abstract or secondary source named on its line.
- No meta-analytic effect size for automation bias in LLM settings was found.

## 4. Evidence for the "drift" mechanism: trust rises with accuracy, so vigilance drops as error rates fall

### Takeaway
The drift mechanism is well supported **outside** LLMs by three lines of evidence:
1. Reliability-induced complacency: higher automation reliability lowers detection of its failures.
2. The vigilance decrement: monitoring performance falls over time.
3. The low-prevalence effect in visual search: rare targets are missed far more often.

Together these predict exactly the drift: as an LLM's error rate falls, the human's chance of catching each remaining error also falls. **No LLM study yet tests this directly** by manipulating AI error rate over time and measuring detection. In AI contexts the drift is a well-grounded inference, not a measured finding.

### Cited Findings
- **Low-prevalence effect.** Wolfe, Horowitz & Kenner (2005), "Rare items often missed in visual searches", *Nature* 435:439–440, DOI 10.1038/435439a. In an X-ray baggage-style search, error rates were about 30% at prevalence under 1%, 16% at 10%, and 7% at 50%. Evidence: LAB, PEER-REVIEWED. *[metadata confirmed through Crossref; the Nature full text is paywalled, so the rates come from a secondary source]* — [DOI](https://doi.org/10.1038/435439a); [Wikipedia, Prevalence effect](https://en.wikipedia.org/wiki/Prevalence_effect)
- **Complacency.** Operators keep monitoring better when automation reliability varies than when it stays constant. Evidence: LAB, PEER-REVIEWED. *[content via Wikipedia's summary; metadata via Crossref]* — [Parasuraman, Molloy & Singh 1993, DOI](https://doi.org/10.1207/s15327108ijap0301_1); [Wikipedia, Automation bias](https://en.wikipedia.org/wiki/Automation_bias)
- **Reliability and monitoring.** Reliable automation lowers monitoring, and this holds in experts. Evidence: PEER-REVIEWED review. *[summary from Crossref abstract]* — [Parasuraman & Manzey 2010, DOI](https://doi.org/10.1177/0018720810376055)
- **The LLM-specific version is only cross-sectional.**
  - Confidence in the AI for a task predicted less self-reported critical thinking (β = −0.69), and higher confidence in the AI predicted less perceived evaluation effort (β = −0.23).
  - Overall trust did not predict it.
  - Sources: [Lee et al. 2025](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf). Shaw & Nave found higher trust predicted more surrender ([The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason)).
- **Path dependence.** Addy's "path dependence" (each accepted chunk makes the next surrender likelier) is presented as reasoning, with no study cited for it. — [Addy Osmani blog](https://addyosmani.com/blog/cognitive-surrender/)
- **The friction lever is causal, but only in one paper.** Incentives with feedback reduced surrender and time pressure increased it, both manipulated experimentally in Shaw & Nave. They are the only manipulated levers found that bear on vigilance. — [The Decision Lab](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason); [Ars mirror](https://drwebdomain.blog/2026/04/04/cognitive-surrender-leads-ai-users-to-abandon-logical-thinking-research-finds-ars-technica/)

### Inferences
- **MEDIUM-HIGH:** Low prevalence plus complacency gives a principled reason that better models do not shrink review risk in proportion. Each remaining error is likelier to be missed. Expected missed errors fall less than the error rate does, and may become harder to detect when they do occur.
- **MEDIUM:** Shaw & Nave's 50% AI error rate is the opposite of the drift condition. Surrender to a coin-flip-reliable AI suggests deference may be even stronger with a 95%-reliable AI, but this is untested.
- **LOW:** A countermeasure the evidence suggests is planting known errors or "catch trials" in AI output to raise effective prevalence. This works in screening and radiology. No LLM study was found.

### Gaps
- No longitudinal or within-subject study was found that manipulates LLM accuracy over time and measures error detection, the direct test of drift.
- Wolfe-lab follow-ups (e.g., radiologists, prevalence fixes) were not retrieved because search was unavailable.

## 5. Which claims are strong, and which are weak or over-hyped

### Takeaway
Strong claims:
- Unguided AI use in learning lowers later unaided performance (field RCT plus lab RCT).
- People defer to wrong AI answers in short reasoning tasks (large preregistered lab, preprint).
- Automation bias and complacency in general (decades of peer-reviewed human-factors work).

Weak or over-hyped claims:
- EEG "brain rot" from Kosmyna.
- Causal "AI lowers critical thinking" from Gerlich and Lee, which are surveys.
- The Google effect.
- Precise headline rates such as "73%" or "80%".
- The staged offloading → deferral → surrender progression, which no study measures.

### Evidence table

| Claim | Best evidence | Type / status | Strength |
|---|---|---|---|
| Wrong AI answers are widely adopted on reasoning items | [Shaw & Nave 2026](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason) | LAB, preregistered, N = 1,372, PREPRINT | Strong for the task; narrow scope |
| AI raises confidence even when wrong | [Shaw & Nave 2026](https://thedecisionlab.com/insights/technology/when-ai-becomes-part-of-how-we-reason) | LAB, PREPRINT | Moderate |
| Unguided AI during practice harms later unaided performance | [Bastani et al. PNAS 2025](https://econpapers.repec.org/RePEc:nas:journl:v:122:y:2025:p:e2422633122); [Shen & Tamkin 2026](https://www.anthropic.com/research/AI-assistance-coding-skills) | Field RCT (peer-reviewed) + lab RCT (preprint, N = 52) | Strong (two designs agree) |
| Guardrails or conceptual use avoid the harm | Same two studies | RCT arm (Bastani); correlational subgroups, tiny n (Anthropic) | Moderate (Bastani); weak (Anthropic subgroups) |
| Confidence in AI goes with less critical thinking at work | [Lee et al. CHI 2025](https://advait.org/files/lee_2025_ai_critical_thinking_survey.pdf) | SURVEY, self-report, PEER-REVIEWED | Moderate as correlation; no causation |
| Frequent AI use lowers critical thinking, through offloading | [Gerlich 2025](https://doaj.org/article/d5c17dd91ce64775a065095d8af730ce) | SURVEY, cross-sectional, PEER-REVIEWED (MDPI) | Weak for causation |
| LLM writing lowers brain connectivity ("cognitive debt") | [Kosmyna et al. 2025](https://arxiv.org/abs/2506.08872), critiqued in [arXiv 2601.00856](https://arxiv.org/abs/2601.00856v1) | OBS-EEG, N = 54 (18 in Session 4), PREPRINT | Weak / over-hyped |
| Search engines make us forget ("Google effect") | [Sparrow 2011](https://forrt.org/flora-replication-atlas/doi/10.1126/science.1207745/) | LAB; failed replications | Weak |
| Reliability breeds complacency; rare errors get missed | Parasuraman & Manzey 2010; Wolfe 2005 (DOIs above) | LAB, PEER-REVIEWED, replicated | Strong (non-LLM domains) |
| "Drift" in LLM use specifically | none found | — | Hypothesis (well grounded) |
| Staged progression offloading → deferral → surrender | none found; Addy's article does not claim stages | OPINION | Unsupported as staging |

### Inferences
- **HIGH:** An article built on "cognitive surrender" stands on firm ground for "people defer to confident wrong AI answers, and unguided use costs skill". It is on shaky ground if it cites the MIT EEG study or Gerlich as causal, or presents 73% (or Ars's 80%) as a general workplace rate.
- **MEDIUM:** The strongest support for the drift argument comes from older human-factors literature, not from 2025–2026 LLM papers. Citing Parasuraman & Manzey and Wolfe would make the argument stronger than leaning on the new preprints.

### Gaps
- Coverage: search covered the 7 named studies plus Shen & Tamkin. A systematic search for 2026 LLM replications was attempted once and found none; a second attempt was blocked by search failure. Treat the absence of replications as "not found", not "does not exist".
- The Shaw & Nave and Bastani numbers come from their abstracts (fetched) and press coverage. Their full texts were not reachable (SSRN 403; the PNAS full text was not fetched), so per-study detail and the 73% vs 80% acceptance rate stay unresolved.
