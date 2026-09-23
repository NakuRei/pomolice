import { VolumeMutedIcon, VolumeOnIcon } from "@/components/icons/AppIcons";
import { ToggleIconButton } from "./ToggleIconButton";

interface NotificationMuteToggleButtonProps {
  muted: boolean;
  onToggle: () => void;
}

export function NotificationMuteToggleButton({
  muted,
  onToggle,
}: NotificationMuteToggleButtonProps): React.ReactElement {
  const buttonTitle = muted ? "通知音ミュート中" : "通知音オン";

  return (
    <ToggleIconButton
      label="通知音"
      pressed={muted}
      onPressedChange={() => {
        onToggle();
      }}
      pressedAppearance="danger"
      title={buttonTitle}
      pressedChildren={<VolumeMutedIcon className="h-5 w-5" />}
    >
      <VolumeOnIcon className="h-5 w-5" />
    </ToggleIconButton>
  );
}
