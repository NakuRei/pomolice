import { useEffect, useRef, useState } from "react";
import {
  playNotificationSoundPreview,
  type NotificationSoundType,
} from "../../services/notifications";
import { useSettingsStore, VALIDATION_RULES } from "../../store/settingsStore";
import { VolumeMutedIcon, VolumeOnIcon } from "../icons";

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
      <header className="mb-5">
        <h1
          ref={headingReference}
          tabIndex={-1}
          data-testid="settings-notification-heading"
          className="text-2xl font-bold text-gray-900"
        >
          通知設定
        </h1>
      </header>

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
            <button
              type="button"
              onClick={toggleNotificationMuteDraft}
              className={`transition-colors ${
                notificationSoundMutedDraft
                  ? "text-red-600 hover:text-red-700"
                  : "text-gray-600 hover:text-gray-800"
              }`}
              title={
                notificationSoundMutedDraft ? "通知音ミュート中" : "通知音オン"
              }
              data-testid="settings-notification-mute-button"
              aria-label={
                notificationSoundMutedDraft
                  ? "通知音をオンにする"
                  : "通知音をミュートする"
              }
            >
              {notificationSoundMutedDraft ? (
                <VolumeMutedIcon className="h-5 w-5" />
              ) : (
                <VolumeOnIcon className="h-5 w-5" />
              )}
            </button>

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
              className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-gray-200 accent-blue-600"
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
            <button
              type="button"
              onClick={runNotificationSoundPreview}
              disabled={notificationSoundMutedDraft}
              className={`rounded-md px-3 py-1 text-sm font-medium transition-colors ${
                notificationSoundMutedDraft
                  ? "cursor-not-allowed bg-gray-200 text-gray-500"
                  : "bg-blue-600 text-white hover:bg-blue-700"
              }`}
              data-testid="settings-notification-sound-preview-button"
              aria-label="通知音をテスト再生する"
            >
              テスト再生
            </button>
          </div>
          <p
            className="text-xs text-gray-600"
            data-testid="settings-notification-sound-preview-status"
            aria-live="polite"
          >
            {notificationSoundPreviewStatusText}
          </p>
        </div>

        <footer
          className="flex gap-3 pt-2"
          data-testid="settings-notification-footer"
        >
          <button
            type="button"
            onClick={cancelNotificationSettingsPage}
            className="flex-1 rounded-lg border border-gray-300 bg-gray-100 px-4 py-2 text-sm font-medium text-gray-700 transition-colors hover:bg-gray-200 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-gray-400"
            data-testid="settings-cancel"
          >
            キャンセル
          </button>
          <button
            type="button"
            onClick={saveNotificationSettingsPage}
            disabled={!notificationSoundVolumeDraftIsValid}
            className={`flex-1 rounded-lg px-4 py-2 text-sm font-medium text-white transition-colors focus-visible:outline-2 focus-visible:outline-offset-2 ${
              notificationSoundVolumeDraftIsValid
                ? "bg-blue-600 hover:bg-blue-700 focus-visible:outline-blue-500"
                : "cursor-not-allowed bg-gray-400 focus-visible:outline-gray-400"
            }`}
            data-testid="settings-save"
          >
            保存
          </button>
        </footer>
      </div>
    </section>
  );
}
