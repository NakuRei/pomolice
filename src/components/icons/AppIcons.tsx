import type { SVGProps } from "react";
import {
  ArrowLeft,
  LoaderCircle,
  Play,
  Settings,
  TriangleAlert,
  Volume2,
  VolumeX,
} from "lucide-react";

const DEFAULT_ICON_STROKE_WIDTH = 1.5;

/**
 * Shared props for app-level icon wrappers.
 *
 * Usage example:
 * ```tsx
 * <VolumeMutedIcon className="h-6 w-6 text-danger-500" />
 * ```
 */
type AppIconProps = Omit<SVGProps<SVGSVGElement>, "children">;

export function VolumeMutedIcon(props: AppIconProps): React.ReactElement {
  return (
    <VolumeX
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function BackArrowIcon(props: AppIconProps): React.ReactElement {
  return (
    <ArrowLeft
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function VolumeOnIcon(props: AppIconProps): React.ReactElement {
  return (
    <Volume2
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function SettingsGearIcon(props: AppIconProps): React.ReactElement {
  return (
    <Settings
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function WarningTriangleIcon(props: AppIconProps): React.ReactElement {
  return (
    <TriangleAlert
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function LoadingSpinnerIcon(props: AppIconProps): React.ReactElement {
  return (
    <LoaderCircle
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function PlayIcon(props: AppIconProps): React.ReactElement {
  return (
    <Play
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}
