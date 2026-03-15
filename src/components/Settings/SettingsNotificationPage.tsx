import { useEffect, useRef, useState } from "react";
import {
  playNotificationSoundPreview,
  type NotificationSoundType,
} from "../../services/notifications";
import { useSettingsStore, VALIDATION_RULES } from "../../store/settingsStore";
import { Button } from "../shared/Button";
import { NotificationMuteToggleButton } from "../shared/NotificationMuteToggleButton";
import { SettingsPageHeader } from "./SettingsPageHeader";
import { SettingsSaveCancelFooter } from "./SettingsSaveCancelFooter";

interface SettingsNotificationPageProps {
  openSettingsTopScreen: () => void;
}

const isFiniteNumberValue = (value: number | null): value is number =>
  typeof value === "number" && Number.isFinite(value);

export function SettingsNotificationPage({
  openSettingsTopScreen,
}: SettingsNotificationPageProps) {
  const headingReference = useRef<HTMLHeadingElement | null>(null);

  const notificationSoundMutedFromStore = useSettingsStore(
    (state) => state.notificationSoundMuted,
  );
  const notificationSoundVolumePercentFromStore = useSettingsStore(
    (state) => state.notificationSoundVolumePercent,
  );
  const updateSettings = useSettingsStore((state) => state.updateSettings);

  const [notificationSoundMutedDraft, setNotificationSoundMutedDraft] =
    useState<boolean>(notificationSoundMutedFromStore);
  const [
    notificationSoundVolumePercentDraft,
    setNotificationSoundVolumePercentDraft,
  ] = useState<number | null>(notificationSoundVolumePercentFromStore);
  const [notificationSoundPreviewType, setNotificationSoundPreviewType] =
    useState<NotificationSoundType>("work");
  const [
    notificationSoundPreviewStatusText,
    setNotificationSoundPreviewStatusText,
  ] = useState<string>("");

  useEffect(() => {
    headingReference.current?.focus();
  }, []);

  const notificationSoundVolumeDraftIsValid =
    isFiniteNumberValue(notificationSoundVolumePercentDraft) &&
    notificationSoundVolumePercentDraft >=
      VALIDATION_RULES.notificationSoundVolumePercent.min &&
    notificationSoundVolumePercentDraft <=
      VALIDATION_RULES.notificationSoundVolumePercent.max;

  const saveNotificationSettingsPage = () => {
    if (!notificationSoundVolumeDraftIsValid) {
      return;
    }
    if (notificationSoundVolumePercentDraft === null) {
      return;
    }

    updateSettings({
      notificationSoundMuted: notificationSoundMutedDraft,
      notificationSoundVolumePercent: notificationSoundVolumePercentDraft,
    });
    openSettingsTopScreen();
  };

  const cancelNotificationSettingsPage = () => {
    openSettingsTopScreen();
  };

  const toggleNotificationMuteDraft = () => {
    setNotificationSoundMutedDraft((previousMutedState) => {
      if (!previousMutedState) {
        setNotificationSoundPreviewStatusText(
          "通知音がミュート中のため再生しませんでした。",
        );
      }
      return !previousMutedState;
    });
  };

  const runNotificationSoundPreview = async () => {
    if (notificationSoundMutedDraft) {
      setNotificationSoundPreviewStatusText(
        "通知音がミュート中のため再生しませんでした。",
      );
      return;
    }

    const previewResult = await playNotificationSoundPreview({
      soundType: notificationSoundPreviewType,
      volumePercent: notificationSoundVolumePercentDraft ?? 0,
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

      <div className="space-y-5 rounded-lg border border-gray-200 bg-white p-4">
        <div>
          <div className="mb-2 flex items-center justify-between">
            <label
              htmlFor="settings-notification-sound-volume"
              className="text-sm font-medium text-gray-700"
            >
              通知音量
            </label>
            <span
              className="text-sm text-gray-600 tabular-nums"
              data-testid="settings-notification-sound-volume-value"
            >
              {notificationSoundVolumePercentDraft ?? 0}%
            </span>
          </div>

          <div className="flex items-center gap-3">
            <NotificationMuteToggleButton
              muted={notificationSoundMutedDraft}
              onToggle={toggleNotificationMuteDraft}
              testId="settings-notification-mute-button"
            />

            <input
              id="settings-notification-sound-volume"
              type="range"
              min={VALIDATION_RULES.notificationSoundVolumePercent.min}
              max={VALIDATION_RULES.notificationSoundVolumePercent.max}
              step={1}
              value={notificationSoundVolumePercentDraft ?? 0}
              onChange={(event) =>
                setNotificationSoundVolumePercentDraft(
                  Number(event.target.value),
                )
              }
              className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-gray-200 accent-primary-600"
              data-testid="settings-notification-sound-volume"
              aria-label="通知音量"
              disabled={notificationSoundMutedDraft}
            />
          </div>
        </div>

        <div className="space-y-2">
          <div className="flex items-center gap-2">
            <label
              htmlFor="settings-notification-sound-preview-type"
              className="text-sm text-gray-700"
            >
              音の種類
            </label>
            <select
              id="settings-notification-sound-preview-type"
              value={notificationSoundPreviewType}
              onChange={(event) =>
                setNotificationSoundPreviewType(
                  event.target.value as NotificationSoundType,
                )
              }
              className="rounded-md border border-gray-300 px-2 py-1 text-sm text-gray-700"
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
              onClick={runNotificationSoundPreview}
              disabled={notificationSoundMutedDraft}
              testId="settings-notification-sound-preview-button"
              aria-label="通知音をテスト再生する"
            >
              テスト再生
            </Button>
          </div>
          <p
            className="text-xs text-gray-600"
            data-testid="settings-notification-sound-preview-status"
            aria-live="polite"
          >
            {notificationSoundPreviewStatusText}
          </p>
        </div>

        <SettingsSaveCancelFooter
          onCancelButtonClick={cancelNotificationSettingsPage}
          onSaveButtonClick={saveNotificationSettingsPage}
          saveButtonDisabled={!notificationSoundVolumeDraftIsValid}
          footerTestId="settings-notification-footer"
          footerClassName="pt-2"
        />
      </div>
    </section>
  );
}
