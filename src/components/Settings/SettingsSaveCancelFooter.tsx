import { Button } from "../shared/Button";

/**
 * Shared action row for settings editor pages.
 *
 * Example:
 * <SettingsSaveCancelFooter
 *   onCancelButtonClick={openSettingsTopScreen}
 *   onSaveButtonClick={saveTimerSettingsPage}
 *   saveButtonDisabled={!timerDurationDraftValuesAreValid}
 *   footerTestId="settings-timer-footer"
 *   footerClassName="pt-2"
 * />
 */
interface SettingsSaveCancelFooterProps {
  onCancelButtonClick: () => void;
  onSaveButtonClick: () => void;
  saveButtonDisabled: boolean;
  footerTestId: string;
  footerClassName?: string;
  cancelButtonTestId?: string;
  saveButtonTestId?: string;
  cancelButtonLabel?: string;
  saveButtonLabel?: string;
}

export function SettingsSaveCancelFooter({
  onCancelButtonClick,
  onSaveButtonClick,
  saveButtonDisabled,
  footerTestId,
  footerClassName = "",
  cancelButtonTestId = "settings-cancel",
  saveButtonTestId = "settings-save",
  cancelButtonLabel = "キャンセル",
  saveButtonLabel = "保存",
}: SettingsSaveCancelFooterProps) {
  const combinedFooterClassName =
    footerClassName === "" ? "flex gap-3" : `flex gap-3 ${footerClassName}`;

  return (
    <footer className={combinedFooterClassName} data-testid={footerTestId}>
      <Button
        appearance="secondary"
        size="sm"
        onClick={onCancelButtonClick}
        className="flex-1"
        testId={cancelButtonTestId}
      >
        {cancelButtonLabel}
      </Button>
      <Button
        appearance="primary"
        size="sm"
        onClick={onSaveButtonClick}
        disabled={saveButtonDisabled}
        className="flex-1"
        testId={saveButtonTestId}
      >
        {saveButtonLabel}
      </Button>
    </footer>
  );
}
