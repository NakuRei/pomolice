import { VolumeMutedIcon, VolumeOnIcon } from "@/components/icons/AppIcons";
import { ToggleIconButton } from "./ToggleIconButton";

interface NotificationMuteToggleButtonProps {
  muted: boolean;
  onToggle: () => void;
  iconClassName?: string;
  className?: string;
}

export function NotificationMuteToggleButton({
  muted,
  onToggle,
  iconClassName = "h-5 w-5",
  className = "",
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
      className={className}
      title={buttonTitle}
      pressedChildren={<VolumeMutedIcon className={iconClassName} />}
    >
      <VolumeOnIcon className={iconClassName} />
    </ToggleIconButton>
  );
}
