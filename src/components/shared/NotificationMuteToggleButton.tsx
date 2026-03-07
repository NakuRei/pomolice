import { VolumeMutedIcon, VolumeOnIcon } from "../icons";

interface NotificationMuteToggleButtonProps {
  muted: boolean;
  onToggle: () => void;
  testId: string;
  iconClassName?: string;
  className?: string;
}

export function NotificationMuteToggleButton({
  muted,
  onToggle,
  testId,
  iconClassName = "h-5 w-5",
  className = "",
}: NotificationMuteToggleButtonProps) {
  const buttonTitle = muted ? "通知音ミュート中" : "通知音オン";
  const buttonAriaLabel = muted ? "通知音をオンにする" : "通知音をミュートする";
  const buttonColorClassName = muted
    ? "text-red-600 hover:text-red-700"
    : "text-gray-600 hover:text-gray-800";
  const buttonClassName =
    `transition-colors ${buttonColorClassName} ${className}`.trim();

  return (
    <button
      type="button"
      onClick={onToggle}
      className={buttonClassName}
      title={buttonTitle}
      data-testid={testId}
      aria-label={buttonAriaLabel}
    >
      {muted ? (
        <VolumeMutedIcon className={iconClassName} />
      ) : (
        <VolumeOnIcon className={iconClassName} />
      )}
    </button>
  );
}
