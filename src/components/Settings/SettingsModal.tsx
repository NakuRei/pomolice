import { useState, useEffect, useRef, useId, type KeyboardEvent } from "react";
import { useSettingsStore, VALIDATION_RULES } from "../../store/settingsStore";
import {
  playNotificationSoundPreview,
  type NotificationSoundType,
} from "../../services/notifications";
import { VolumeMutedIcon, VolumeOnIcon } from "../icons";
import { DurationInput } from "./DurationInput";
import "./SettingsModal.css";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

/**
 * SettingsModal Component
 * Modal dialog for configuring timer durations
 *
 * Allows users to customize:
 * - Work session duration (1-120 minutes)
 * - Short break duration (1-60 minutes)
 * - Long break duration (5-60 minutes)
 * - Long break interval (2-10 cycles)
 */
export function SettingsModal({ isOpen, onClose }: SettingsModalProps) {
  const dialogRef = useRef<HTMLDivElement | null>(null);
  const cancelButtonRef = useRef<HTMLButtonElement | null>(null);
  const titleId = useId();

  const workDurationStore = useSettingsStore((state) => state.workDuration);
  const shortBreakDurationStore = useSettingsStore(
    (state) => state.shortBreakDuration,
  );
  const longBreakDurationStore = useSettingsStore(
    (state) => state.longBreakDuration,
  );
  const longBreakIntervalStore = useSettingsStore(
    (state) => state.longBreakInterval,
  );
  const notificationSoundVolumePercentStore = useSettingsStore(
    (state) => state.notificationSoundVolumePercent,
  );
  const notificationSoundMutedStore = useSettingsStore(
    (state) => state.notificationSoundMuted,
  );
  const updateSettings = useSettingsStore((state) => state.updateSettings);

  const isFiniteNumber = (value: number | null): value is number =>
    typeof value === "number" && Number.isFinite(value);

  // Local state for form inputs
  const [workDuration, setWorkDuration] = useState<number | null>(workDurationStore);
  const [shortBreakDuration, setShortBreakDuration] = useState<number | null>(
    shortBreakDurationStore,
  );
  const [longBreakDuration, setLongBreakDuration] = useState<number | null>(
    longBreakDurationStore,
  );
  const [longBreakInterval, setLongBreakInterval] = useState<number | null>(
    longBreakIntervalStore,
  );
  const [notificationSoundVolumePercent, setNotificationSoundVolumePercent] =
    useState<number | null>(notificationSoundVolumePercentStore);
  const [notificationSoundMuted, setNotificationSoundMuted] =
    useState<boolean>(notificationSoundMutedStore);
  const [notificationSoundPreviewType, setNotificationSoundPreviewType] =
    useState<NotificationSoundType>("work");
  const [notificationSoundPreviewStatusText, setNotificationSoundPreviewStatusText] =
    useState<string>("");

  // Sync local state with store when modal opens
  useEffect(() => {
    if (isOpen) {
      setWorkDuration(workDurationStore);
      setShortBreakDuration(shortBreakDurationStore);
      setLongBreakDuration(longBreakDurationStore);
      setLongBreakInterval(longBreakIntervalStore);
      setNotificationSoundVolumePercent(notificationSoundVolumePercentStore);
      setNotificationSoundMuted(notificationSoundMutedStore);
      setNotificationSoundPreviewType("work");
      setNotificationSoundPreviewStatusText("");
    }
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const previousBodyOverflow = document.body.style.overflow;
    const previousBodyOverscrollBehavior = document.body.style.overscrollBehavior;

    document.body.style.overflow = "hidden";
    document.body.style.overscrollBehavior = "contain";

    return () => {
      document.body.style.overflow = previousBodyOverflow;
      document.body.style.overscrollBehavior = previousBodyOverscrollBehavior;
    };
  }, [isOpen]);

  useEffect(() => {
    if (!isOpen) {
      return;
    }

    cancelButtonRef.current?.focus();
  }, [isOpen]);

  const handleDialogKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Escape") {
      event.preventDefault();
      onClose();
      return;
    }

    if (event.key !== "Tab") {
      return;
    }

    const dialog = dialogRef.current;
    if (!dialog) {
      return;
    }

    const focusableElements = Array.from(
      dialog.querySelectorAll<HTMLElement>(
        'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
      ),
    );

    if (focusableElements.length === 0) {
      event.preventDefault();
      dialog.focus();
      return;
    }

    const firstElement = focusableElements[0];
    const lastElement = focusableElements[focusableElements.length - 1];
    const activeElement = document.activeElement as HTMLElement | null;
    const isFocusInsideDialog = activeElement
      ? dialog.contains(activeElement)
      : false;

    if (event.shiftKey) {
      if (!isFocusInsideDialog || activeElement === firstElement) {
        event.preventDefault();
        lastElement.focus();
      }
      return;
    }

    if (!isFocusInsideDialog || activeElement === lastElement) {
      event.preventDefault();
      firstElement.focus();
    }
  };

  if (!isOpen) {
    return null;
  }

  // Validation: Check if all values are within valid ranges
  const isWorkDurationValid =
    isFiniteNumber(workDuration) &&
    workDuration >= VALIDATION_RULES.workDuration.min &&
    workDuration <= VALIDATION_RULES.workDuration.max;
  const isShortBreakValid =
    isFiniteNumber(shortBreakDuration) &&
    shortBreakDuration >= VALIDATION_RULES.shortBreakDuration.min &&
    shortBreakDuration <= VALIDATION_RULES.shortBreakDuration.max;
  const isLongBreakValid =
    isFiniteNumber(longBreakDuration) &&
    longBreakDuration >= VALIDATION_RULES.longBreakDuration.min &&
    longBreakDuration <= VALIDATION_RULES.longBreakDuration.max;
  const isIntervalValid =
    isFiniteNumber(longBreakInterval) &&
    longBreakInterval >= VALIDATION_RULES.longBreakInterval.min &&
    longBreakInterval <= VALIDATION_RULES.longBreakInterval.max;
  const isNotificationSoundVolumeValid =
    isFiniteNumber(notificationSoundVolumePercent) &&
    notificationSoundVolumePercent >=
      VALIDATION_RULES.notificationSoundVolumePercent.min &&
    notificationSoundVolumePercent <=
      VALIDATION_RULES.notificationSoundVolumePercent.max;

  const isFormValid =
    isWorkDurationValid &&
    isShortBreakValid &&
    isLongBreakValid &&
    isIntervalValid &&
    isNotificationSoundVolumeValid;

  const handleSave = () => {
    if (
      !isWorkDurationValid ||
      !isShortBreakValid ||
      !isLongBreakValid ||
      !isIntervalValid ||
      !isNotificationSoundVolumeValid ||
      workDuration === null ||
      shortBreakDuration === null ||
      longBreakDuration === null ||
      longBreakInterval === null ||
      notificationSoundVolumePercent === null
    ) {
      return;
    }

    updateSettings({
      workDuration,
      shortBreakDuration,
      longBreakDuration,
      longBreakInterval,
      notificationSoundVolumePercent,
      notificationSoundMuted,
    });

    onClose();
  };

  const handleCancel = () => {
    // Reset to current settings
    setWorkDuration(workDurationStore);
    setShortBreakDuration(shortBreakDurationStore);
    setLongBreakDuration(longBreakDurationStore);
    setLongBreakInterval(longBreakIntervalStore);
    setNotificationSoundVolumePercent(notificationSoundVolumePercentStore);
    setNotificationSoundMuted(notificationSoundMutedStore);
    onClose();
  };

  const handleNotificationSoundPreview = async () => {
    if (notificationSoundMuted) {
      setNotificationSoundPreviewStatusText("通知音がミュート中のため再生しませんでした。");
      return;
    }

    const previewResult = await playNotificationSoundPreview({
      soundType: notificationSoundPreviewType,
      volumePercent: notificationSoundVolumePercent ?? 0,
      muted: notificationSoundMuted,
    });

    if (previewResult.status === "played") {
      setNotificationSoundPreviewStatusText("通知音を再生しました。");
      return;
    }

    if (previewResult.status === "muted") {
      setNotificationSoundPreviewStatusText("通知音がミュート中のため再生しませんでした。");
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
    <div
      className="settingsModalOverlay"
      data-testid="settings-modal"
    >
      <div
        ref={dialogRef}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        onKeyDown={handleDialogKeyDown}
        className="settingsModalDialog"
        data-testid="settings-modal-dialog"
      >
        <header className="settingsModalHeader" data-testid="settings-modal-header">
          <h2 id={titleId} className="settingsModalTitle">
            設定
          </h2>
        </header>

        <section className="settingsModalBody" data-testid="settings-modal-body">
          <DurationInput
            label="作業時間"
            value={workDuration}
            onChange={setWorkDuration}
            min={VALIDATION_RULES.workDuration.min}
            max={VALIDATION_RULES.workDuration.max}
            testId="settings-work-duration"
          />

          <DurationInput
            label="短い休憩"
            value={shortBreakDuration}
            onChange={setShortBreakDuration}
            min={VALIDATION_RULES.shortBreakDuration.min}
            max={VALIDATION_RULES.shortBreakDuration.max}
            testId="settings-short-break"
          />

          <DurationInput
            label="長い休憩"
            value={longBreakDuration}
            onChange={setLongBreakDuration}
            min={VALIDATION_RULES.longBreakDuration.min}
            max={VALIDATION_RULES.longBreakDuration.max}
            testId="settings-long-break"
          />

          <DurationInput
            label="長い休憩の間隔"
            value={longBreakInterval}
            onChange={setLongBreakInterval}
            min={VALIDATION_RULES.longBreakInterval.min}
            max={VALIDATION_RULES.longBreakInterval.max}
            unit="回"
            testId="settings-interval"
          />

          <div>
            <div className="flex items-center justify-between mb-2">
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
                {notificationSoundVolumePercent ?? 0}%
              </span>
            </div>
            <div className="flex items-center gap-3">
              <button
                type="button"
                onClick={() =>
                  setNotificationSoundMuted((previousMutedState) => {
                    if (!previousMutedState) {
                      setNotificationSoundPreviewStatusText(
                        "通知音がミュート中のため再生しませんでした。",
                      );
                    }
                    return !previousMutedState;
                  })
                }
                className={`transition-colors ${
                  notificationSoundMuted
                    ? "text-red-600 hover:text-red-700"
                    : "text-gray-600 hover:text-gray-800"
                }`}
                title={notificationSoundMuted ? "通知音ミュート中" : "通知音オン"}
                data-testid="settings-notification-mute-button"
                aria-label={notificationSoundMuted ? "通知音をオンにする" : "通知音をミュートする"}
              >
                {notificationSoundMuted ? (
                  <VolumeMutedIcon className="w-5 h-5" />
                ) : (
                  <VolumeOnIcon className="w-5 h-5" />
                )}
              </button>

              <input
                id="settings-notification-sound-volume"
                type="range"
                min={VALIDATION_RULES.notificationSoundVolumePercent.min}
                max={VALIDATION_RULES.notificationSoundVolumePercent.max}
                step={1}
                value={notificationSoundVolumePercent ?? 0}
                onChange={(event) =>
                  setNotificationSoundVolumePercent(Number(event.target.value))
                }
                className="w-full h-2 bg-gray-200 rounded-lg appearance-none cursor-pointer accent-blue-600"
                data-testid="settings-notification-sound-volume"
                aria-label="通知音量"
                disabled={notificationSoundMuted}
              />
            </div>
            <div className="mt-3 flex items-center gap-2">
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
                onClick={handleNotificationSoundPreview}
                disabled={notificationSoundMuted}
                className={`rounded-md px-3 py-1 text-sm font-medium transition-colors ${
                  notificationSoundMuted
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
              className="mt-2 text-xs text-gray-600"
              data-testid="settings-notification-sound-preview-status"
              aria-live="polite"
            >
              {notificationSoundPreviewStatusText}
            </p>
          </div>
        </section>

        <footer
          className="settingsModalFooter"
          data-testid="settings-modal-footer"
          style={{ paddingBottom: "calc(1.5rem + env(safe-area-inset-bottom))" }}
        >
          <button
            ref={cancelButtonRef}
            onClick={handleCancel}
            className="settingsModalCancelButton"
            data-testid="settings-cancel"
          >
            キャンセル
          </button>
          <button
            onClick={handleSave}
            disabled={!isFormValid}
            className={`settingsModalSaveButton ${
              isFormValid
                ? "settingsModalSaveButtonEnabled"
                : "settingsModalSaveButtonDisabled"
            }`}
            data-testid="settings-save"
          >
            保存
          </button>
        </footer>
      </div>
    </div>
  );
}
