export type LogFieldValue = string | number | boolean | null;

export type FrontendLogSeverity = "trace" | "debug" | "info" | "warn" | "error";

export type RedactionClassification = "public" | "sensitive" | "secret";

export type FrontendScope =
  | "bootstrap"
  | "notifications"
  | "settings_sync"
  | "timer_sync";

type NoFields = Record<string, never>;
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
  "app.startup.completed": NoFields;
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
  "settings.sync.failed": {
    failure_message?: string;
    timer_status?: string;
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
  "app.startup.completed": {
    eventId: "app.startup.completed",
    severity: "info",
    messageTemplate: "application startup completed",
    allowedFields: [],
    redactionClassification: {},
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
  "settings.sync.failed": {
    eventId: "settings.sync.failed",
    severity: "warn",
    messageTemplate: "settings sync failed",
    allowedFields: ["failure_message", "timer_status"],
    redactionClassification: {
      failure_message: "sensitive",
      timer_status: "public",
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
  appStartupCompleted: "app.startup.completed",
  notificationPermissionRequestFailed: "notification.permission.request_failed",
  notificationSessionCompleteNotificationFailed:
    "notification.session_complete_notification_failed",
  notificationSoundPathResolutionFailed:
    "notification.sound_path_resolution_failed",
  notificationSoundPreviewPlaybackFailed:
    "notification.sound_preview_playback_failed",
  settingsSyncFailed: "settings.sync.failed",
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
