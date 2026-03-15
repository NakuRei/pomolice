import type { ReactNode } from "react";

/**
 * Shared visual frame for Skip Flow steps.
 *
 * Usage example:
 * ```tsx
 * <SkipFlowStepFrame
 *   title="Typing Challenge"
 *   description={<>Type the phrase <strong>exactly</strong>.</>}
 *   notice={<div>Typo detected! Starting over...</div>}
 *   footer={<p>Any typo restarts this step.</p>}
 *   actions={<SkipFlowCancelButton onCancel={onCancel} />}
 * >
 *   <TypingPhraseInput />
 * </SkipFlowStepFrame>
 * ```
 */
interface SkipFlowStepFrameProps {
  /** Step title shown at the top of the frame. */
  title: ReactNode;
  /** Optional explanatory text shown below the title. */
  description?: ReactNode;
  /** Main step-specific content. */
  children: ReactNode;
  /** Optional status or warning block shown below the main content. */
  notice?: ReactNode;
  /** Optional hint or supporting text shown near the bottom. */
  footer?: ReactNode;
  /** Optional action area such as cancel or confirmation buttons. */
  actions?: ReactNode;
}

export default function SkipFlowStepFrame({
  title,
  description,
  children,
  notice,
  footer,
  actions,
}: SkipFlowStepFrameProps) {
  return (
    <div className="space-y-8">
      <div className="text-center">
        <h2 className="mb-2 text-2xl font-bold text-text">{title}</h2>
        {description ? <p className="text-text-muted">{description}</p> : null}
      </div>

      {children}

      {notice}

      {footer ? (
        <div className="text-center text-sm text-text-muted">{footer}</div>
      ) : null}

      {actions}
    </div>
  );
}
