import { useEmergencyShortcutStore } from "@/backend/emergencyShortcutStore";

import { StatusNotice } from "@/components/shared/StatusNotice";

export function EmergencyShortcutNotice(): React.ReactElement | null {
  const status = useEmergencyShortcutStore((state) => state.status);
  const connectionFailed = useEmergencyShortcutStore(
    (state) => state.connectionFailed,
  );

  if (status?.kind !== "unavailable" && !connectionFailed) return null;
  return (
    <div className="relative z-60 shrink-0 bg-background p-3">
      <StatusNotice tone="warning" role="alert">
        {connectionFailed
          ? "緊急解除のショートカット情報を取得できません。"
          : "緊急解除のショートカットを利用できません。"}
      </StatusNotice>
    </div>
  );
}
