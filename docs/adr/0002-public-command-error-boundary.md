# Public Command Error Boundary

- Date: 2026-06-01
- Status: Accepted

## Context

Backend command failures cross from the application core into the frontend as values that frontend code can inspect and display. If those values expose raw internal errors, framework rejection details, file paths, panic text, or lower-level diagnostic messages, the boundary becomes surprising and unsafe: users can see implementation details, and future code may accidentally depend on unstable internal wording.

At the same time, diagnostic information is still necessary for development and support. The decision must separate public command error output from internal diagnostic information without making the frontend guess from raw backend messages.

This ADR covers command request/response failures. It does not cover event payload design, notification failures, log storage format, or non-error response schemas.

## Decision Drivers

- Keep the public error boundary predictable for future backend and frontend changes
- Prevent internal diagnostic details from becoming part of the external contract
- Preserve enough internal diagnostic information for troubleshooting
- Keep user-facing presentation as a frontend responsibility
- Make the behavior testable through observable public output rather than internal implementation shape

## Considered Options

- Return raw backend or framework errors: simple at the call site, but exposes unstable internal details and makes UI behavior depend on raw messages
- Return a small application-defined error envelope: creates an explicit public contract while keeping internals separate
- Return a rich backend-owned UI envelope with severity and retry metadata: centralizes behavior, but couples backend failures to frontend presentation policy

## Decision

Public command failures will be exposed through a small application-defined error envelope with a stable machine-readable code and a short public message.

The public envelope must not include raw internal error text, raw framework rejection details, file paths, panic details, stack traces, source chains, or other diagnostic-only information.

Backend code is responsible for converting command failures into the public envelope before they become observable at the frontend command boundary. Frontend code is responsible for turning the public code into user-facing presentation such as display text, severity, retry affordances, and placement.

Internal diagnostic information may be retained in internal error values and logs, but it must remain separate from the public command error envelope.

## Consequences

- Positive: new command failures have a predictable public shape instead of exposing incidental internal messages
- Positive: frontend behavior can be driven by stable codes rather than parsing backend text
- Positive: internal diagnostic detail can remain available without becoming public output
- Negative: every new command failure category needs an explicit public code and message decision
- Negative: framework-originated command rejection paths need deliberate normalization before they are treated as application errors

## Validation

This decision is applied correctly when command failures observed by frontend code have the public envelope shape, user-facing behavior is derived from the public code, and sensitive or diagnostic details remain absent from the public value even when they exist internally for troubleshooting.
