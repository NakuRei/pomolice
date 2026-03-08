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
      <button
        type="button"
        onClick={onCancelButtonClick}
        className="flex-1 rounded-lg border border-gray-300 bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-gray-400"
        data-testid={cancelButtonTestId}
      >
        {cancelButtonLabel}
      </button>
      <button
        type="button"
        onClick={onSaveButtonClick}
        disabled={saveButtonDisabled}
        className={`flex-1 rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 ${
          saveButtonDisabled
            ? "cursor-not-allowed bg-gray-400 focus-visible:outline-gray-400"
            : "bg-blue-600 hover:bg-blue-700 focus-visible:outline-blue-500"
        }`}
        data-testid={saveButtonTestId}
      >
        {saveButtonLabel}
      </button>
    </footer>
  );
}
