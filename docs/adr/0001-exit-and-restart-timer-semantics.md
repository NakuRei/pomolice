# Timer Semantics Across Exit And Restart

- Date: 2026-04-14
- Status: Accepted

## Context

Pomolice is not a reminder app. Its product goal is to make skipping breaks harder than taking them.

The current implementation already treats `working` as a real-time session. If the app is exited and later restarted, persisted `working` time is restored with elapsed wall-clock time deducted, and an expired work session is finalized on startup.

However, break handling is not aligned with that product goal yet. Persisted `on_break` state is currently normalized away during restore, which makes break behavior inconsistent with both the product concept and user expectations about what should happen after an out-of-band termination.

For this decision, "out-of-band termination" means any app exit that does not go through the in-app break skip flow, including tray `Exit`, OS force quit, crash, and similar process termination paths.

This ADR needs to define, state by state, whether time while the app is not running counts as elapsed session time, and how startup restore should behave after such an exit.

## Decision Drivers

- Preserve the core concept that breaks are enforced, not merely suggested
- Avoid creating an easy bypass by exiting the app at session boundaries
- Keep restore behavior deterministic and explainable per timer state
- Avoid over-penalizing accidental exits or crashes more than necessary

## Considered Options

- Keep the current design: `working` is wall-clock based, but restored `on_break` is normalized away during startup
- Make all running states wall-clock based: both `working` and `on_break` continue to elapse while the app is not running
- Pause all timer progress while the app is not running: neither work nor break consumes time after exit
- Use state-specific semantics: `working` consumes wall-clock time, while `on_break` consumes time only while the app is actively enforcing the break or the user completes the sanctioned in-app skip flow
- Use punitive break reset semantics: unauthorized exit during `on_break` restarts the full break duration on next launch

## Decision

We will use state-specific semantics across exit and restart.

`working` is a real-time session. While the timer is in `working`, elapsed wall-clock time counts even if the app process is not running. On startup, restored `working` time is reduced by elapsed real time. If restored `working` has already expired, the work-session boundary is finalized on startup.

`on_break` is an enforced-rest session, not a wall-clock countdown. Break time counts only while the app is actively running and enforcing the break. Time spent while the app is not running does not count as break consumption.

The only exception is the sanctioned in-app break skip flow. If the user completes that flow and the app transitions out of break through it, the break is considered intentionally ended by product design.

Partial or interrupted skip-flow progress is not itself sanctioned. If the app exits before the skip flow completes, that exit is treated the same as any other out-of-band termination during break.

If the app terminates out-of-band during `on_break`, the next launch must restore the break from the previously saved remaining time. Elapsed wall-clock time while the app was not running must not be subtracted from that remaining break time.

Restoring `on_break` means restoring an actively enforced break experience, not downgrading the user into a passive non-break or preview state on startup.

`post_break_decision_pending` does not auto-resolve while the app is not running. Restarting the app must preserve the pending decision state rather than consuming time or automatically advancing the cycle.

Once the timer has transitioned to `idle`, no further elapsed time is charged to the previous session. This includes cases where the user explicitly stopped the session through `emergency_stop` before exiting the app.

## Consequences

- Positive: exiting the app cannot be used to avoid a work-to-break transition
- Positive: exiting the app during break does not let the user silently consume enforced rest while enforcement is absent
- Positive: `emergency_stop` has a clear meaning; once the app is back in `idle`, the prior session no longer progresses
- Negative: restore semantics are intentionally asymmetric between `working` and `on_break`
- Negative: users who started but did not finish the skip flow lose that in-progress attempt after exit and must resume the break instead
- Constraints: persistence and restore must preserve enough break state to resume `on_break` from saved remaining time after unauthorized termination
- Constraints: a completed break skip is the only sanctioned early break exit; incomplete skip-flow progress must not count as break resolution

## Validation

The decision is applied correctly when the following scenarios behave as specified:

- A work session with `3` seconds remaining is exited and relaunched after more than `3` seconds, and startup enters break rather than resuming work
- A break with `3` minutes remaining is exited out-of-band and relaunched after more than `3` minutes, and startup resumes the same break with `3` minutes still remaining
- Completing the sanctioned break skip flow during break ends the break intentionally, and a later restart does not resurrect that break
- Exiting partway through the sanctioned break skip flow does not count as a sanctioned break exit, and the next launch resumes the enforced break rather than the partial skip attempt
- Calling `emergency_stop` and then exiting the app leaves the next launch in `idle` without charging additional elapsed time to the stopped work session
- A restored `post_break_decision_pending` state still requires an explicit continue/stop decision after restart
