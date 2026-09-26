import { WarningTriangleIcon } from "@/components/icons/AppIcons";
import { useTimerStore } from "@/store/timerStore";
import { StatusNotice } from "./StatusNotice";

/**
 * Tells the user their saved settings could not be read at startup, so the
 * defaults are in effect until they save settings again.
 */
export function SettingsFileNotice(): React.ReactElement | null {
  const settingsFileStatus = useTimerStore((state) => state.settingsFileStatus);

  if (settingsFileStatus !== "unusable") {
    return null;
  }

  return (
    <StatusNotice
      tone="warning"
      role="status"
      ariaLive="polite"
      testId="settings-file-notice"
      icon={
        <WarningTriangleIcon className="h-5 w-5 shrink-0 text-warning-400" />
      }
    >
      保存済みの設定を読み込めなかったため、初期設定で動作しています。設定を保存すると、今表示されている値で保存し直されます。
    </StatusNotice>
  );
}
