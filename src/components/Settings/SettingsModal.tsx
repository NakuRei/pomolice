import { useState, useEffect } from "react";
import { useSettingsStore, VALIDATION_RULES } from "../../store/settingsStore";
import { VolumeMutedIcon, VolumeOnIcon } from "../icons";
import { DurationInput } from "./DurationInput";

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

  // Sync local state with store when modal opens
  useEffect(() => {
    if (isOpen) {
      setWorkDuration(workDurationStore);
      setShortBreakDuration(shortBreakDurationStore);
      setLongBreakDuration(longBreakDurationStore);
      setLongBreakInterval(longBreakIntervalStore);
      setNotificationSoundVolumePercent(notificationSoundVolumePercentStore);
      setNotificationSoundMuted(notificationSoundMutedStore);
    }
  }, [isOpen]);

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

  return (
    <div
      className="fixed inset-0 bg-black/50 flex items-center justify-center z-50"
      data-testid="settings-modal"
    >
      <div className="bg-white rounded-lg shadow-xl p-8 max-w-md w-full mx-4">
        <h2 className="text-2xl font-bold mb-6 text-gray-900">設定</h2>

        <div className="space-y-6">
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
                onClick={() => setNotificationSoundMuted((prev) => !prev)}
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
          </div>
        </div>

        <div className="flex gap-4 mt-8">
          <button
            onClick={handleCancel}
            className="flex-1 px-6 py-3 text-gray-700 bg-gray-200 rounded-lg hover:bg-gray-300 focus:outline-none focus:ring-2 focus:ring-gray-400 transition-colors"
            data-testid="settings-cancel"
          >
            キャンセル
          </button>
          <button
            onClick={handleSave}
            disabled={!isFormValid}
            className={`flex-1 px-6 py-3 text-white rounded-lg focus:outline-none focus:ring-2 transition-colors ${
              isFormValid
                ? "bg-blue-600 hover:bg-blue-700 focus:ring-blue-500"
                : "bg-gray-400 cursor-not-allowed"
            }`}
            data-testid="settings-save"
          >
            保存
          </button>
        </div>
      </div>
    </div>
  );
}
