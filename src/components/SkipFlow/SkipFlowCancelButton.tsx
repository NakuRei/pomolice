import { Button } from "../shared/Button";

/**
 * Shared cancel button for challenge-style Skip Flow steps.
 */
interface SkipFlowCancelButtonProps {
  onCancel: () => void;
}

export default function SkipFlowCancelButton({
  onCancel,
}: SkipFlowCancelButtonProps): React.ReactElement {
  return (
    <Button appearance="secondary" onClick={onCancel}>
      Cancel and Take Break
    </Button>
  );
}
