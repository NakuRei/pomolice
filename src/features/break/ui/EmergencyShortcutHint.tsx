import { useEmergencyShortcutStore } from "@/backend/emergencyShortcutStore";

export function EmergencyShortcutHint(): React.ReactElement {
  const status = useEmergencyShortcutStore((state) => state.status);
  const connectionFailed = useEmergencyShortcutStore(
    (state) => state.connectionFailed,
  );
  let guidance: React.ReactNode = "確認中";
  if (connectionFailed) {
    guidance = "確認不可";
  } else {
    switch (status?.kind) {
      case "input_monitor":
        guidance = <kbd>Ctrl + Alt + Shift + E</kbd>;
        break;
      case "registered":
        guidance = <kbd>{status.trigger}</kbd>;
        break;
      case "registering":
        guidance = "登録待ち";
        break;
      case "unavailable":
        guidance = "ショートカット利用不可";
        break;
      case undefined:
      default:
        break;
    }
  }
  return (
    <p className="text-xs text-text-muted">
      緊急解除：
      {guidance}
    </p>
  );
}
