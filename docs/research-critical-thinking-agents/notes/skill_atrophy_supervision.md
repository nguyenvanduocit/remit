# Skill atrophy from AI assistance and the "paradox of supervision"

Evidence-strength labels used below:
- **PR**: peer-reviewed. **PP**: preprint / not peer-reviewed. **ORG**: organisation report or blog (not peer-reviewed). **SEC**: secondary coverage only.
- Design: **RCT**, **EXP** (lab experiment, non-randomised or randomised), **OBS** (observational), **SURVEY/QUAL**, **META** (meta-analysis), **CASE** (accident investigation).
- Confidence in my reading of the source: **HIGH** (read primary text or abstract this session), **MEDIUM** (secondary source or partial text), **LOW** (memory, not verified).

---

## 1. What Anthropic's research actually says

### Takeaway
The "paradox of supervision" is an Anthropic framing drawn from a non-anonymous internal survey (n=132) and interviews (n=53) of its own engineers, with no measurement of skill. The only Anthropic data that measures skill is a separate small RCT preprint (n=52, Jan 2026). It found AI help during learning of a new library cut immediate comprehension by 17 percentage points, mostly on debugging, the skill supervision needs. How people used the AI decided the result.

### Cited Findings
**"How AI is transforming work at Anthropic"** (Societal Impacts team, published 2 Dec 2025). [ORG, SURVEY/QUAL, HIGH]
- Survey of 132 Anthropic engineers/researchers (Aug 2025), 53 in-depth interviews, plus 200,000 internal Claude Code transcripts (Feb and Aug 2025) analysed with Clio. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- Self-reported share of work using Claude: 59% (vs 28% a year earlier). Self-reported productivity gain: +50% (vs +20%). 14% report gains above 100%. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- Merged PRs per engineer per day up 67% after Claude Code adoption. 27% of Claude-assisted work would not have been done otherwise. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- More than half of respondents can "fully delegate" only 0–20% of their work. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- Transcript data: consecutive autonomous tool calls per transcript 9.8 → 21.2 (+116%); human turns per transcript 6.2 → 4.1 (−33%); task complexity 3.2 → 3.8 on a 1–5 scale. Humans intervene less per task, so they supervise more and hand-code less. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- The report names the "paradox of supervision": using Claude well requires supervision, and supervising Claude requires the coding skills that may atrophy from AI overuse. Engineer quotes: *"having my skills atrophy or fail to develop is primarily gonna be problematic with respect to my ability to safely use AI"*; *"I am for sure atrophying in my skills as a software engineer"* (the same engineer added they could recover the skills if needed); *"When producing output is so easy and fast, it gets harder and harder to actually take the time to learn something."* Some engineers deliberately practise without AI. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- Limitations the authors state: convenience/purposive sampling (68 responses from Slack, 64 from direct outreach at a 31% response rate), non-anonymous responses (social-desirability bias), recall bias for the earlier period, self-reported productivity, and data collected in the Sonnet 4 / Opus 4 era. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- The report does not use the word "deskilling" and contains no objective skill measurement. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic) (HIGH, per a full-page read)

**"How AI Impacts Skill Formation"**, Shen & Tamkin (Anthropic Fellows), 29 Jan 2026, arXiv 2601.20245. [PP, RCT, HIGH] The study exists, so the claim in the brief checks out.
- 52 participants, mostly junior engineers who had used Python weekly for over a year and had never used the Trio async library. Protocol: 10-min warm-up, 35-min Trio coding task, 25-min quiz, 5-min survey. AI (a sidebar chat assistant) was available only during the coding task. [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills); [arXiv](https://www.arxiv.org/pdf/2601.20245)
- Quiz: AI group ~50% vs hand-coding group ~67%, a 17-point gap, Cohen's d = 0.738, p = 0.01. Debugging questions showed the largest gap. [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills)
- Speed: the AI group was about 2 minutes faster (≈23 vs ≈25 min), not significant (p = 0.391). [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills)
- Interaction patterns (qualitative, small n, not causal):
  - Higher-scoring: generation-then-comprehension (n=2, ~86%), hybrid code-explanation (n=3, ~68%), conceptual inquiry (n=7, ~65%).
  - Lower-scoring: AI delegation (n=4, ~39%), progressive AI reliance (n=4, ~35%), iterative AI debugging (n=4, ~24%).
  [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills)
- Caveats the authors state: small sample, juniors only, immediate test (no retention data), one library, and a chat sidebar rather than agentic tools. The authors expect agentic tools to have a *stronger* effect. They note the tension with their earlier finding of up to 80% time savings on tasks where people already had the skill. Their hypothesis is that AI speeds up skills already developed while hindering the learning of new ones. [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills)
- One news report gives 51 participants, while the primary source says 52. [ITPro](https://itpro.com/software/development/anthropic-research-ai-coding-skills-formation-impact) vs [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills)

### Inferences
- MEDIUM: The "paradox" is a hypothesis voiced by practitioners. Anthropic did not test it. Its RCT tested skill *formation* (learning new material), not *atrophy* of skills already held. No Anthropic study measures whether experienced engineers lose supervisory ability over months.
- MEDIUM: The RCT does support one link in the paradox. Debugging, the skill most needed to check AI output, is the skill that formed worst under AI assistance. It also shows that the *mode* of use (asking conceptual questions, reading for comprehension) matters as much as the amount of use.
- MEDIUM: The transcript data (fewer human turns, more autonomous tool calls) shows that hands-on practice is falling inside the very population that reports atrophy worries. That fits a rising-supervision, falling-practice pattern.

### Gaps
- Not found: an Anthropic (or any) longitudinal study measuring coding-skill decay in experienced developers who delegate to agents.
- The arXiv paper's per-question-type breakdown (exact debugging gap) was not extracted; the blog gives no number.
- Peer-review status of arXiv 2601.20245 as of Nov 2026 not confirmed.

---

## 2. Cross-domain deskilling evidence

### Takeaway
The strongest real-world signal is in medicine. Experienced endoscopists' unassisted adenoma detection fell 6 points within 3 months of AI introduction (observational, confounded). Aviation evidence matches the Anthropic paradox closely. Hand skills survive automation fairly well; the *cognitive* skills of manual operation (situational awareness, spotting failures) degrade, and they degrade with mind-wandering while supervising. Radiology shows a different failure mode: automation bias, where incorrect AI drags experts down. Experience reduces the drag but does not remove it. GPS evidence suggests habitual offloading degrades the underlying capacity, from small samples.

### Cited Findings
**Endoscopy: Budzyń et al., *Lancet Gastroenterology & Hepatology* 2025;10(10):896–903** [PR, OBS, HIGH]
- Retrospective observational study nested in the ACCEPT trial: 4 Polish centres that introduced Olympus ENDO-AID CADe in late 2021. It compared *non-AI* colonoscopies in the 3 months before vs 3 months after AI introduction (Sep 2021–Mar 2022). [ACG EBGI summary](https://gi.org/journals-publications/ebgi/zhou_sep2025/)
- 1,443 non-AI colonoscopies (795 before, 648 after), done by 19 endoscopists with >2,000 colonoscopies each (mean experience 28 years). [ACG EBGI](https://gi.org/journals-publications/ebgi/zhou_sep2025/)
- ADR 28.4% → 22.4%, absolute −6.0 points (95% CI −10.5 to −1.6; P=0.009). Adjusted OR for AI exposure 0.69 (0.53–0.89). Adenomas per colonoscopy 0.54 → 0.43 (P=0.071, not significant). Cancer detection did not fall: 6 (0.8%) vs 8 (1.2%). [ACG EBGI](https://gi.org/journals-publications/ebgi/zhou_sep2025/)
- Limitations: observational, residual confounding, sex and sedation imbalances between periods, withdrawal time not reported, per-endoscopist analysis not possible, low Polish baseline ADR (the US target is 35%). Proposed mechanism: CADe reduces endoscopists' eye travel during exams. [ACG EBGI](https://gi.org/journals-publications/ebgi/zhou_sep2025/); [Warwick repository record](https://wrap.warwick.ac.uk/id/eprint/191005)

**Aviation: Casner, Geven, Recker & Schooler, "The retention of manual flying skills in the automated cockpit," *Human Factors* 2014** [PR, EXP, HIGH]
- 16 airline pilots flew routine and non-routine scenarios in a B747-400 simulator at varied automation levels. [Europe PMC abstract, DOI 10.1177/0018720814535628](https://europepmc.org/search?query=DOI%3A10.1177%2F0018720814535628)
- Instrument scanning and stick-and-rudder control were "mostly intact" even when rarely practised. The *cognitive* tasks of manual flight were the ones that failed more often and more seriously: tracking position without a map display, deciding the next navigation steps, recognising instrument failures. [Europe PMC](https://europepmc.org/search?query=DOI%3A10.1177%2F0018720814535628)
- Performance on those cognitive tasks was associated with how often pilots had task-unrelated thoughts while automation flew. Conclusion: retention of these cognitive skills "may depend on the degree to which pilots remain actively engaged in supervising the automation." [Europe PMC](https://europepmc.org/search?query=DOI%3A10.1177%2F0018720814535628)

**Aviation: Air France 447 (BEA final report, published July 2012)** [CASE, HIGH: primary PDF read]
- The PF's excessive control inputs after autopilot disconnection are explained "by the startle effect and the emotional shock at the autopilot disconnection, amplified by the lack of practical training for crews in flight at high altitude, together with unusual flight control laws." [BEA final report](https://bea.aero/fileadmin/documents/docspa/2009/f-cp090601.en/pdf/f-cp090601.en.pdf)
- Recommendation section "Training for Manual Aircraft Handling": "Manual aeroplane handling cannot be improvised… only specific and regular training can provide the skills necessary"; "the copilots had not been trained for manual aeroplane handling of approach to stall and stall recovery at high altitude." The BEA recommends EASA mandate "specific and regular exercises dedicated to manual aircraft handling of approach to stall and stall recovery, including at high altitude." [BEA final report](https://bea.aero/fileadmin/documents/docspa/2009/f-cp090601.en/pdf/f-cp090601.en.pdf)
- The stall warning and buffeting were not identified, and the PNF did not catch the PF's inputs "through an absence of effective monitoring of the flight path", a supervision failure. [BEA final report](https://bea.aero/fileadmin/documents/docspa/2009/f-cp090601.en/pdf/f-cp090601.en.pdf)

**Radiology: Dratsch et al., "Automation Bias in Mammography," *Radiology* 2023** [PR, EXP (prospective, randomised sets), HIGH]
- 27 radiologists read 50 mammograms with a purported AI. In the 40-case test set, 12 cases carried a deliberately incorrect BI-RADS suggestion. [Europe PMC, DOI 10.1148/radiol.222176](https://europepmc.org/search?query=DOI%3A10.1148%2Fradiol.222176)
- Accuracy with correct vs incorrect AI suggestions: inexperienced 79.7% vs 19.8%; moderately experienced 81.3% vs 24.8%; very experienced 82.3% vs 45.5% (all P≤.003). Inexperienced readers were the most biased. [Europe PMC](https://europepmc.org/search?query=DOI%3A10.1148%2Fradiol.222176)

**GPS: Dahmani & Bohbot, "Habitual use of GPS negatively impacts spatial memory during self-guided navigation," *Scientific Reports* 2020** [PR, OBS (cross-sectional plus small longitudinal), HIGH]
- 50 regular drivers cross-sectionally: more lifetime GPS use went with worse spatial memory when navigating without GPS. 13 were retested 3 years later: more GPS use in the interval was associated with a steeper decline in hippocampal-dependent spatial memory. Heavy GPS users did not report a poorer sense of direction at baseline, which the authors read as GPS causing decline rather than the reverse. Authors call the longitudinal sample "small." [Europe PMC, DOI 10.1038/s41598-020-62877-0](https://europepmc.org/search?query=DOI%3A10.1038%2Fs41598-020-62877-0)

**LLM writing: Kosmyna et al., "Your Brain on ChatGPT: Accumulation of Cognitive Debt…" (MIT Media Lab, arXiv 2506.08872, 2025)** [PP, EXP, HIGH on the abstract]
- 54 participants over 3 sessions (LLM / search / brain-only), and 18 in a 4th crossover session. LLM users showed the weakest EEG connectivity. When switched to brain-only they showed reduced alpha/beta connectivity ("under-engagement"), struggled to quote their own essays, and reported the lowest ownership. [arXiv](https://arxiv.org/abs/2506.08872)

### Inferences
- MEDIUM: Aviation gives the most direct analogue of the "paradox of supervision". In Casner et al., what decays is not the motor act (typing code) but the mental model needed to take over and to notice failure: tracking state, anticipating the next step, recognising anomalies. For software, that maps to reading and debugging unfamiliar code and holding system state, the same area where the Anthropic RCT found the largest gap.
- MEDIUM: Casner's mind-wandering finding implies that the *quality of supervision*, not just the amount of hands-on practice, protects the skill. Engaged supervision keeps the cognitive skill; passive monitoring erodes it.
- MEDIUM: The endoscopy and radiology studies show two separate risks. Removing the AI leaves the expert below their own baseline (deskilling). Leaving the AI in place lets its errors pass to the expert (automation bias). Both are harder to see in experts than in novices, but they are present.
- LOW: The endoscopy decline appeared within 3 months, which suggests degradation of attentive habits can be fast. Confounding means this speed is not established.

### Gaps
- FAA/PARC-CAST Flight Deck Automation Working Group report (2013) and FAA SAFO 13002 (Jan 2013, which encourages manual flight) could not be fetched (HTTP 403 by WebFetch and by curl). Their findings are not used in these notes.
- Radiology: no longitudinal *skill decay* study (with AI removed) was found in this pass; the radiology evidence found concerns automation bias, not atrophy.
- No replication of Budzyń et al. was located.

---

## 3. Productivity vs perception (METR) and self-assessment

### Takeaway
In METR's 2025 RCT, experienced developers were 19% slower with AI while believing they were 20% faster, a perception error of about 40 points. METR's Feb 2026 follow-up (57 developers, 800+ tasks) measured an 18% speedup for returning developers and 4% for new ones, both with confidence intervals crossing zero. METR calls these an unreliable signal, likely lower bounds, mainly because developers would not work without AI. That refusal is itself a dependence signal. Self-reports of one's own speed, and by extension one's own skill, are unreliable.

### Cited Findings
- METR RCT (arXiv 2507.09089, July 2025): 16 experienced OSS developers, 246 tasks in mature repos (avg 5 years' prior experience), each task randomised to AI-allowed or not, with mainly Cursor Pro and Claude 3.5/3.7 Sonnet. [PP, RCT, HIGH] [arXiv](https://arxiv.org/abs/2507.09089)
  - Forecast before the study: 24% faster. Estimate after: 20% faster. Measured: 19% *slower*.
  - Experts predicted 39% (economics) and 38% (ML) time savings.
  - The authors analysed 20 candidate factors and judge the slowdown unlikely to be primarily an experimental artifact.
- METR follow-up, "We are Changing our Developer Productivity Experiment Design" (24 Feb 2026). [ORG, RCT with acknowledged selection bias, HIGH, primary post read] [METR](https://metr.org/blog/2026-02-24-uplift-update/)
  - Sample: 57 developers (10 from the original study plus 47 new recruits), 143 repos, 800+ tasks, median 10 years' experience. Pay was cut from $150/hr to $50/hr.
  - Estimates (negative = faster with AI): returning developers −18% (95% CI −38% to +9%); new developers −4% (−15% to +9%). The original 2025 estimate was +19% (CI +2% to +39%).
  - In a survey, 30–50% of developers said they skipped submitting some tasks because they did not want to do them without AI. One developer completed no AI-disallowed tasks. Quotes: "I will feel so painful if the task is decided as AI-disallowed"; "I'm torn… I really like using AI!"
  - METR's own judgement: the data gives "an unreliable signal of the current productivity effect of AI tools". The −18% and −4% are likely lower bounds, and the data is only very weak evidence of a productivity increase since early 2025. Its reasons are selection effects (refusals, lower pay, task choice), quality differences between conditions, and unreliable time tracking when developers run several agents at once.
- Earlier secondary reports of "18% faster" for the follow-up match METR's −18% for returning developers only. [Bowley](https://blog.robbowley.net/2026/04/04/metrs-developer-productivity-research-2026-update/) is superseded by the primary post above.
- Within Anthropic, productivity figures are self-reported, and the authors caution that self-reports should be read carefully. [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- Dratsch et al.: radiologists' perceived AI accuracy and confidence were measured alongside the large accuracy drop under incorrect AI. Expertise did not protect fully. [Europe PMC](https://europepmc.org/search?query=DOI%3A10.1148%2Fradiol.222176)

### Inferences
- MEDIUM: If developers misjudge their own speed by ~40 points while working with the tool, introspection is unlikely to tell them where they sit on the offloading spectrum, or how much skill they have lost. Self-assessment of atrophy needs an external probe, such as a timed or graded no-AI task compared with a baseline. That is what a "calibration exercise" would have to be in order to calibrate anything.
- MEDIUM: METR's participation problem ("won't work without AI") is a behavioural marker of dependence, though not evidence of skill loss.
- LOW: The 2025 result predates agentic tools (Claude Code era). Its speed finding may not transfer, but the perception-gap finding is the more durable lesson.

### Gaps
- METR has not published a redesigned study's results as of this pass.
- No study found that directly measures developers' accuracy in estimating their own *skill* (as opposed to speed) under AI use.

---

## 4. Skill retention science: decay rates and maintenance frequency

### Takeaway
Skill decay after nonuse is large and well established. Cognitive and accuracy-based skills decay faster than physical and speed-based ones, which is the bad case for code review and debugging. The 2025 Psychological Bulletin meta-analysis puts the half-life of accuracy gains at about 6.5 months of nonuse. No study found supports "once a week" specifically: the literature supports *intermittent practice* as a moderator, but not a particular frequency for software skills.

### Cited Findings
- Arthur, Bennett, Stanush & McNelly (1998), "Factors That Influence Skill Decay and Retention: A Quantitative Review and Analysis," *Human Performance* 11(1), DOI 10.1207/s15327043hup1101_3 [PR, META, HIGH on abstract; MEDIUM on table values]
  - 189 data points from 53 articles. Skill loss ranges from d = −0.01 immediately after training to d = −1.4 after more than 365 days of nonuse.
  - Physical, natural, and speed-based tasks lose less than cognitive, artificial, and accuracy-based tasks.
  - Recognition tests and similar retrieval conditions show less loss.
  - Overlearning reduces decay.
  - Source: [PDF via gwern.net](https://www.gwern.net/doc/psychology/spaced-repetition/1998-arthur.pdf)
- Arthur et al. Table 3, from my text extraction of the PDF (layout garbled, MEDIUM). Corrected mean d by interval: <1 day ≈ +0.02; 1–7 days ≈ −1.0; 8–14 days ≈ −1.5; 15–28 days ≈ −0.9; 29–90 days ≈ −0.65; 91–180 days ≈ −1.4; 181–365 days ≈ −1.0; >365 days ≈ −1.3. The authors report a correlation of −0.51 between interval and corrected d across 8 points, and SDs large enough to imply moderators. Decay is therefore not monotone across studies, and it appears even at short intervals. [PDF](https://www.gwern.net/doc/psychology/spaced-repetition/1998-arthur.pdf)
- Arthur et al. say closed-loop (fixed-sequence) tasks decay faster than open-loop tasks. They report a prior hypothesis that cognitive skills benefit more from mental practice. [PDF](https://www.gwern.net/doc/psychology/spaced-repetition/1998-arthur.pdf)
- "Procedural skill retention and decay: A meta-analytic review," *Psychological Bulletin* 2025 (PMID 40455501) [PR, META, HIGH on abstract]
  - 1,344 effect sizes from 457 reports.
  - Decay grows by 0.08 SD/month for accuracy and 0.06 SD/month for speed or mixed measures.
  - Half of the initial acquisition gains are lost after about 6.5 months (accuracy), 13 months (speed), and 11 months (mixed).
  - Moderators: task type, complexity, *intermittent performance opportunities*, and instructions.
  - The authors call for separating retention from relearning effects.
  - Source: [Europe PMC / PubMed 40455501](https://pubmed.ncbi.nlm.nih.gov/40455501/)

### Inferences
- MEDIUM: Code review, debugging, and system-state reasoning are cognitive, accuracy-based skills, the class the meta-analyses find most fragile. On the 2025 estimate, an engineer who stops hand-debugging could plausibly lose about half the accuracy advantage within roughly half a year. That is an extrapolation from general procedural-skill data, not from software.
- MEDIUM: "Weekly no-agent coding" is consistent with the literature's moderator ("intermittent performance opportunities" slow decay), but its specific frequency has no evidence behind it. Monthly or biweekly practice might suffice for some skills. And Casner et al. suggest *engaged supervision* matters as much as separate practice sessions.
- MEDIUM: The term "calibration exercise" is apt in a narrower sense. A recurring no-AI task gives an external measurement of one's own skill, which addresses the METR-style self-assessment gap even if it does little to maintain the skill.
- LOW: Supervising AI is itself partial practice (reading, judging, debugging code). Whether it counts as "use" that prevents decay depends on how actively it is done (the Casner and Anthropic interaction-pattern findings). If it counts, the decay estimates above overstate the risk for engaged supervisors.

### Gaps
- No study found that tests maintenance-practice frequency for software or programming skills.
- No study found that measures decay of programming skill in experienced developers after months of agent delegation.

---

## 5. Counter-evidence: offloading that freed capacity

### Takeaway
Calculators are the main case where offloading did not harm, and sometimes improved, achievement, *when the tool was integrated into instruction*. Even AI evidence shows the harm depends on the mode of use: the higher-scoring Anthropic patterns kept learning intact while still using AI. Offloading is not automatically deskilling; disengaged offloading is the risk.

### Cited Findings
- Ellington (2006), meta-analysis of 42 studies on non-CAS graphing calculators (middle school through first-semester calculus) in *School Science and Mathematics*. [PR, META, HIGH on abstract]
  - With calculators in instruction but not testing, students benefited while developing conceptual skills.
  - With calculators in both instruction and testing, procedural, conceptual, and overall achievement improved.
  - Source: [Crossref DOI 10.1111/j.1949-8594.2006.tb18067.x](https://doi.org/10.1111/j.1949-8594.2006.tb18067.x)
- Ellington (2003), "A Meta-Analysis of the Effects of Calculators on Students' Achievement and Attitude Levels in Precollege Mathematics Classes," *Journal for Research in Mathematics Education* 34(5):433–463. The study exists; its abstract was not retrieved (see Gaps). [Crossref DOI 10.2307/30034795](https://doi.org/10.2307/30034795)
- Anthropic RCT: the "generation-then-comprehension" (~86%) and "conceptual inquiry" (~65%) patterns scored at or above the no-AI mean (~67%) while still using AI. Small n, not causal. [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills)
- Anthropic's earlier work (as cited in the RCT post) found up to 80% time savings on tasks where people already had the skill. [Anthropic](https://www.anthropic.com/research/AI-assistance-coding-skills)
- Within Anthropic, 27% of Claude-assisted work is new work that would not otherwise have been done, and the design/planning share of Claude Code tasks rose 1.0% → 9.9%. That is consistent with effort moving up toward higher-level work (self-report and task mix, not skill). [Anthropic](https://www.anthropic.com/research/how-ai-is-transforming-work-at-anthropic)
- Casner et al.: hand-flying motor skills were "mostly intact" despite infrequent practice. Not every offloaded skill decays at the same rate. [Europe PMC](https://europepmc.org/search?query=DOI%3A10.1177%2F0018720814535628)
- Endoscopy: cancer detection did not fall in unassisted exams after AI exposure, so the decline was in subtler findings. [ACG EBGI](https://gi.org/journals-publications/ebgi/zhou_sep2025/)

### Inferences
- MEDIUM: The calculator literature supports a "freed capacity" story only where the offloaded skill is *not* the one needed to check the tool. Calculators do arithmetic, and students still need number sense to spot a wrong answer. The AI-supervision case differs because the offloaded skill (writing and debugging code) *is* the checking skill. That makes the calculator analogy weaker than it first appears for the supervision paradox.
- MEDIUM: Across domains, the common protective factor is active cognitive engagement during assisted work: the Casner supervision/mind-wandering link, the Anthropic comprehension patterns, and the Kosmyna under-engagement finding. That points to a design rule. Use AI in ways that force the human to predict, explain, or verify, rather than relying only on separate no-AI drills.

### Gaps
- The Ellington (2003) abstract and figures were not retrieved (JSTOR blocked). Its findings are not used in these notes.
- Hembree & Dessart (1986) calculator meta-analysis not retrieved.
- Not searched this pass: Bainbridge's "Ironies of Automation" (1983, *Automatica*), the original statement of the supervision paradox. The OpenAlex search returned only a 2023 follow-on paper, which confirms Bainbridge's ironies include the de- and re-skilling of the humans who monitor and take over from automation ([ACM DOI 10.1145/3598469.3598514](https://doi.org/10.1145/3598469.3598514)). Worth citing as the intellectual origin of the Anthropic framing.

---

**Coverage note.** About 30 tool calls. WebSearch failed after 4 queries (auth error), so the rest of the sources came from direct fetches and from the Europe PMC, arXiv, Crossref, and OpenAlex APIs. The following could not be retrieved: the FAA 2013 report (403), FAA SAFO 13002 (403), the Ellington 2003 abstract (JSTOR), and Nature/RSNA/SAGE full texts (403; abstracts were obtained via Europe PMC instead).
