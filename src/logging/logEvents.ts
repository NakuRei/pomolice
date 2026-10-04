import type { SettingsScreen } from "../types/appScreen";
export type { SettingsScreen } from "../types/appScreen";

export type LogFieldValue = string | number | boolean | null;

export type FrontendLogSeverity = "trace" | "debug" | "info" | "warn" | "error";

export type RedactionClassification = "public" | "sensitive";

export type ErrorBoundaryScope = "fullscreen_break" | "settings" | "app";

export type FrontendScope = "error_boundary" | "notifications" | "timer_sync";

type FrontendEventFields = Record<string, LogFieldValue>;

export interface FrontendLogEventDefinition<
  Fields extends FrontendEventFields = FrontendEventFields,
> {
  eventId: string;
  severity: FrontendLogSeverity;
  messageTemplate: string;
  allowedFields: readonly (keyof Fields & string)[];
  redactionClassification: Partial<
    Record<keyof Fields & string, RedactionClassification>
  >;
}

export interface FrontendFieldsByEvent {
  "error_boundary.boundary_render_error_caught": {
    boundary_scope: ErrorBoundaryScope;
    component_stack?: string;
    current_screen?: SettingsScreen;
    failure_message?: string;
  };
  "error_boundary.recoverable_render_error": {
    component_stack?: string;
    failure_message?: string;
  };
  "error_boundary.render_error_reset": {
    boundary_scope: ErrorBoundaryScope;
  };
  "error_boundary.root_render_error_caught": {
    component_stack?: string;
    failure_message?: string;
  };
  "error_boundary.uncaught_render_error": {
    component_stack?: string;
    failure_message?: string;
  };
  "notification.permission.request_failed": {
    failure_message?: string;
  };
  "notification.session_complete_notification_failed": {
    failure_message?: string;
    notification_sound_type?: string;
    notification_title?: string;
  };
  "notification.sound_path_resolution_failed": {
    failure_message?: string;
    notification_sound_type?: string;
  };
  "notification.sound_preview_playback_failed": {
    failure_message?: string;
    notification_sound_type?: string;
  };
  "timer.sync.setup_failed": {
    failure_message?: string;
    max_connection_retries?: number;
    retry_attempt?: number;
  };
}

export type FrontendEventId = keyof FrontendFieldsByEvent;

type FrontendLogEventCatalog = {
  [EventId in FrontendEventId]: FrontendLogEventDefinition<
    FrontendFieldsByEvent[EventId]
  >;
};

export const frontendLogEventCatalog = {
  "error_boundary.boundary_render_error_caught": {
    eventId: "error_boundary.boundary_render_error_caught",
    severity: "error",
    messageTemplate: "error boundary caught render error",
    allowedFields: [
      "boundary_scope",
      "current_screen",
      "failure_message",
      "component_stack",
    ],
    redactionClassification: {
      boundary_scope: "public",
      current_screen: "public",
      failure_message: "sensitive",
      component_stack: "sensitive",
    },
  },
  "error_boundary.recoverable_render_error": {
    eventId: "error_boundary.recoverable_render_error",
    severity: "warn",
    messageTemplate: "root reported recoverable render error",
    allowedFields: ["failure_message", "component_stack"],
    redactionClassification: {
      failure_message: "sensitive",
      component_stack: "sensitive",
    },
  },
  "error_boundary.render_error_reset": {
    eventId: "error_boundary.render_error_reset",
    severity: "info",
    messageTemplate: "error boundary reset render error state",
    allowedFields: ["boundary_scope"],
    redactionClassification: {
      boundary_scope: "public",
    },
  },
  "error_boundary.root_render_error_caught": {
    eventId: "error_boundary.root_render_error_caught",
    severity: "error",
    messageTemplate: "root caught render error",
    allowedFields: ["failure_message", "component_stack"],
    redactionClassification: {
      failure_message: "sensitive",
      component_stack: "sensitive",
    },
  },
  "error_boundary.uncaught_render_error": {
    eventId: "error_boundary.uncaught_render_error",
    severity: "error",
    messageTemplate: "root reported uncaught render error",
    allowedFields: ["failure_message", "component_stack"],
    redactionClassification: {
      failure_message: "sensitive",
      component_stack: "sensitive",
    },
  },
  "notification.permission.request_failed": {
    eventId: "notification.permission.request_failed",
    severity: "error",
    messageTemplate: "notification permission request failed",
    allowedFields: ["failure_message"],
    redactionClassification: {
      failure_message: "sensitive",
    },
  },
  "notification.session_complete_notification_failed": {
    eventId: "notification.session_complete_notification_failed",
    severity: "error",
    messageTemplate: "session complete notification failed",
    allowedFields: [
      "failure_message",
      "notification_sound_type",
      "notification_title",
    ],
    redactionClassification: {
      failure_message: "sensitive",
      notification_sound_type: "public",
      notification_title: "public",
    },
  },
  "notification.sound_path_resolution_failed": {
    eventId: "notification.sound_path_resolution_failed",
    severity: "warn",
    messageTemplate: "notification sound path resolution failed",
    allowedFields: ["failure_message", "notification_sound_type"],
    redactionClassification: {
      failure_message: "sensitive",
      notification_sound_type: "public",
    },
  },
  "notification.sound_preview_playback_failed": {
    eventId: "notification.sound_preview_playback_failed",
    severity: "warn",
    messageTemplate: "notification sound preview playback failed",
    allowedFields: ["failure_message", "notification_sound_type"],
    redactionClassification: {
      failure_message: "sensitive",
      notification_sound_type: "public",
    },
  },
  "timer.sync.setup_failed": {
    eventId: "timer.sync.setup_failed",
    severity: "error",
    messageTemplate: "timer sync setup failed",
    allowedFields: [
      "failure_message",
      "max_connection_retries",
      "retry_attempt",
    ],
    redactionClassification: {
      failure_message: "sensitive",
      max_connection_retries: "public",
      retry_attempt: "public",
    },
  },
} satisfies FrontendLogEventCatalog;

export const FRONTEND_LOG_EVENT_IDS = {
  errorBoundaryBoundaryRenderErrorCaught:
    "error_boundary.boundary_render_error_caught",
  errorBoundaryRecoverableRenderError:
    "error_boundary.recoverable_render_error",
  errorBoundaryRenderErrorReset: "error_boundary.render_error_reset",
  errorBoundaryRootRenderErrorCaught: "error_boundary.root_render_error_caught",
  errorBoundaryUncaughtRenderError: "error_boundary.uncaught_render_error",
  notificationPermissionRequestFailed: "notification.permission.request_failed",
  notificationSessionCompleteNotificationFailed:
    "notification.session_complete_notification_failed",
  notificationSoundPathResolutionFailed:
    "notification.sound_path_resolution_failed",
  notificationSoundPreviewPlaybackFailed:
    "notification.sound_preview_playback_failed",
  timerSyncSetupFailed: "timer.sync.setup_failed",
} as const satisfies Record<string, FrontendEventId>;

export function getFrontendLogEventDefinition(
  eventId: FrontendEventId,
): FrontendLogEventDefinition {
  return frontendLogEventCatalog[eventId];
}

export function isFrontendEventId(value: string): value is FrontendEventId {
  return value in frontendLogEventCatalog;
}
