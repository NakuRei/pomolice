<!--
Small-project ADR template for this repository.

Conventions:
- 1 ADR = 1 important decision.
- Keep it short. One page is the default.
- Prefer plain Markdown over tooling-specific metadata.
- Do not rewrite old ADRs to hide history. Supersede them.
- Suggested file name: NNNN-short-kebab-case.md
- Required: Date, Status, Context, Decision, Consequences.
- Strongly recommended: Decision Drivers, Considered Options.
- Optional metadata should be filled only when it adds traceability.
-->

# {short title}

- Date: YYYY-MM-DD
- Status: Proposed | Accepted | Superseded by ADR-XXXX | Rejected
- Related: {optional links to issue / plan / investigation / PR}
- References: {optional links to benchmark / repro / external doc / notes}
- Supersedes: {optional ADR-XXXX if this replaces an older ADR}

## Context

{What problem are we solving? What constraints, risks, existing behavior, or non-goals matter for this decision?}

## Decision Drivers

- {List 2-5 criteria that matter most for this decision}
- {driver 1}
- {driver 2}

## Considered Options

- {option 1}: {main trade-off}
- {option 2}: {main trade-off}
- {option 3}: {main trade-off}

{Include "keep the current design" if it is a real option. Keep this short; one line per option is enough.}

## Decision

We will {state the chosen approach in clear, concrete terms}.

{Also state the default rule this establishes, plus any explicit exceptions.}

## Consequences

- Positive: {what becomes simpler, safer, faster, or clearer}
- Negative: {what cost, coupling, migration burden, or limitation we accept}
- Constraints: {follow-up rules, boundaries, or obligations created by this decision}

## Validation

{Optional. How will we know this decision was applied correctly or remains valid? Link tests, rollout checks, or manual verification if useful.}
