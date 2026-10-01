import { useState } from "react";
import { useFocusHeadingOnMount } from "@/hooks/useFocusHeadingOnMount";
import {
  playNotificationSoundPreview,
  type NotificationSoundType,
} from "@/services/notifications";
import { useSettingsSave } from "../useSettingsSave";
import type { Settings } from "../../../backend/settings";
import { Button } from "@/components/shared/Button";
import { NotificationMuteToggleButton } from "./NotificationMuteToggleButton";
import { SettingsPageHeader } from "./SettingsPageHeader";
import { SettingsSaveCancelFooter } from "./SettingsSaveCancelFooter";
import { rangedFieldIssueMessage } from "./settingsIssueMessages";

interface SettingsNotificationPageProps {
  settings: Settings;
  openSettingsTopScreen: () => void;
}

export function SettingsNotificationPage({
  settings,
  openSettingsTopScreen,
}: SettingsNotificationPageProps): React.ReactElement {
  const headingReference = useFocusHeadingOnMount();
  const { issues, isSaving, save, settleIssues } = useSettingsSave(
    openSettingsTopScreen,
  );

  const [notificationSoundMutedDraft, setNotificationSoundMutedDraft] =
    useState<boolean>(settings.notificationSoundMuted);
  const [
    notificationSoundVolumePercentDraft,
    setNotificationSoundVolumePercentDraft,
  ] = useState<number>(settings.notificationSoundVolumePercent);
  const [notificationSoundPreviewType, setNotificationSoundPreviewType] =
    useState<NotificationSoundType>("work");
  const [
    notificationSoundPreviewStatusText,
    setNotificationSoundPreviewStatusText,
  ] = useState<string>("");

  const volumeIssueMessage = rangedFieldIssueMessage(
    issues,
    "notification_sound_volume_percent",
  );

  const toggleNotificationMuteDraft = (): void => {
    setNotificationSoundMutedDraft((previousMutedState) => {
      if (!previousMutedState) {
        setNotificationSoundPreviewStatusText(
          "通知音がミュート中のため再生しませんでした。",
        );
      }
      return !previousMutedState;
    });
  };

  const runNotificationSoundPreview = async (): Promise<void> => {
    if (notificationSoundMutedDraft) {
      setNotificationSoundPreviewStatusText(
        "通知音がミュート中のため再生しませんでした。",
      );
      return;
    }

    const previewResult = await playNotificationSoundPreview({
      soundType: notificationSoundPreviewType,
      volumePercent: notificationSoundVolumePercentDraft,
      muted: notificationSoundMutedDraft,
    });

    if (previewResult.status === "played") {
      setNotificationSoundPreviewStatusText("通知音を再生しました。");
      return;
    }
    if (previewResult.status === "muted") {
      setNotificationSoundPreviewStatusText(
        "通知音がミュート中のため再生しませんでした。",
      );
      return;
    }
    if (previewResult.status === "unavailable") {
      setNotificationSoundPreviewStatusText(
        "通知音ファイルを読み込めなかったため再生できませんでした。",
      );
      return;
    }

    setNotificationSoundPreviewStatusText("通知音の再生に失敗しました。");
  };

  return (
    <section
      className="mx-auto w-full max-w-2xl"
      data-testid="settings-notification-page"
    >
      <SettingsPageHeader
        title="通知設定"
        headingRef={headingReference}
        headingTestId="settings-notification-heading"
        backButtonLabel="設定一覧へ戻る"
        backButtonTestId="settings-notification-back"
        onBackButtonClick={openSettingsTopScreen}
      />

      <div className="space-y-5 rounded-lg border border-border bg-surface p-4">
        <div>
          <div className="mb-2 flex items-center justify-between">
            <label
              htmlFor="settings-notification-sound-volume"
              className="text-sm font-medium text-text"
            >
              通知音量
            </label>
            <span
              className="text-sm text-text-muted tabular-nums"
              data-testid="settings-notification-sound-volume-value"
            >
              {notificationSoundVolumePercentDraft}%
            </span>
          </div>

          <div className="flex items-center gap-3">
            <NotificationMuteToggleButton
              muted={notificationSoundMutedDraft}
              onToggle={toggleNotificationMuteDraft}
            />

            <input
              id="settings-notification-sound-volume"
              type="range"
              min={0}
              max={100}
              step={1}
              value={notificationSoundVolumePercentDraft}
              onChange={(event) => {
                setNotificationSoundVolumePercentDraft(
                  Number(event.target.value),
                );
                settleIssues(
                  (issue) =>
                    issue.code === "out_of_range" &&
                    issue.field === "notification_sound_volume_percent",
                );
              }}
              className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-surface-raised accent-primary-700"
              data-testid="settings-notification-sound-volume"
              aria-label="通知音量"
              disabled={notificationSoundMutedDraft}
            />
          </div>
          {volumeIssueMessage !== undefined && (
            <p
              className="mt-1 text-xs text-danger-300"
              data-testid="settings-notification-sound-volume-error"
              role="alert"
            >
              {volumeIssueMessage}
            </p>
          )}
        </div>

        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <label
              htmlFor="settings-notification-sound-preview-type"
              className="text-sm text-text"
            >
              音の種類
            </label>
            <select
              id="settings-notification-sound-preview-type"
              value={notificationSoundPreviewType}
              onChange={(event) => {
                setNotificationSoundPreviewType(
                  event.target.value as NotificationSoundType,
                );
              }}
              className="rounded-md border border-border bg-surface-raised px-2 py-1 text-sm text-on-surface"
              data-testid="settings-notification-sound-preview-type"
              aria-label="テストする通知音の種類"
            >
              <option value="work">作業終了</option>
              <option value="short_break">短い休憩終了</option>
              <option value="long_break">長い休憩終了</option>
            </select>
            <Button
              appearance="primary"
              size="sm"
              onClick={() => {
                void runNotificationSoundPreview();
              }}
              disabled={notificationSoundMutedDraft}
              testId="settings-notification-sound-preview-button"
              aria-label="通知音をテスト再生する"
            >
              テスト再生
            </Button>
          </div>
          <p
            className="text-xs text-text-muted"
            data-testid="settings-notification-sound-preview-status"
            aria-live="polite"
          >
            {notificationSoundPreviewStatusText}
          </p>
        </div>

        <SettingsSaveCancelFooter
          onCancelButtonClick={openSettingsTopScreen}
          onSaveButtonClick={() => {
            void save({
              notificationSoundMuted: notificationSoundMutedDraft,
              notificationSoundVolumePercent:
                notificationSoundVolumePercentDraft,
            });
          }}
          saveButtonDisabled={isSaving}
          footerTestId="settings-notification-footer"
          footerClassName="pt-2"
        />
      </div>
    </section>
  );
}
