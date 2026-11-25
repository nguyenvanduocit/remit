# rollout

The rule a chain's landing passes before it reaches users (ADR-0050, ADR-0114): a change goes live in stages, and `vet`
lets a stage land only when the one before it on the repository has baked and every guardrail it watches reads
unbreached. The plan is the landing's (`model::StagePlan`); `Stage::landed` is what a landed plan is to the next one,
derived from the landing record. Pure: the queue hands it the previous stage, the time and a probe of each guardrail.
