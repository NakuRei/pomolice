import type { SVGProps } from "react";
import {
  ArrowLeft,
  LoaderCircle,
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
 * <VolumeMutedIcon className="h-6 w-6 text-red-600" />
 * ```
 */
type AppIconProps = Omit<SVGProps<SVGSVGElement>, "children">;

export function VolumeMutedIcon(props: AppIconProps) {
  return (
    <VolumeX
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function BackArrowIcon(props: AppIconProps) {
  return (
    <ArrowLeft
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function VolumeOnIcon(props: AppIconProps) {
  return (
    <Volume2
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function SettingsGearIcon(props: AppIconProps) {
  return (
    <Settings
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function WarningTriangleIcon(props: AppIconProps) {
  return (
    <TriangleAlert
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}

export function LoadingSpinnerIcon(props: AppIconProps) {
  return (
    <LoaderCircle
      strokeWidth={DEFAULT_ICON_STROKE_WIDTH}
      aria-hidden="true"
      {...props}
    />
  );
}
