import { useState, useEffect, useRef, useId, type KeyboardEvent } from "react";
import {
  useSettingsStore,
  VALIDATION_RULES,
  WEEKDAY_KEYS,
  parseHhMm,
  validateWeeklyAutoRunConfig,
  type DayScheduleValidationError,
  type WeeklyAutoRunConfig,
  type WeeklyScheduleValidationError,
  type WeekdayKey,
} from "../../store/settingsStore";
import {
  playNotificationSoundPreview,
  type NotificationSoundType,
} from "../../services/notifications";
import { TimerStatus, useTimerStore } from "../../store/timerStore";
import { VolumeMutedIcon, VolumeOnIcon } from "../icons";
import { DurationInput } from "./DurationInput";
import "./SettingsModal.css";

interface SettingsModalProps {
  isOpen: boolean;
  onClose: () => void;
}

const WEEKDAY_LABEL_BY_KEY: Record<WeekdayKey, string> = {
  mon: "月",
  tue: "火",
  wed: "水",
  thu: "木",
  fri: "金",
  sat: "土",
  sun: "日",
};

function cloneWeeklyConfig(config: WeeklyAutoRunConfig): WeeklyAutoRunConfig {
  return {
    mon: { ...config.mon },
    tue: { ...config.tue },
    wed: { ...config.wed },
    thu: { ...config.thu },
    fri: { ...config.fri },
    sat: { ...config.sat },
    sun: { ...config.sun },
  };
}

function isOvernightDaySlot(daySlot: WeeklyAutoRunConfig[WeekdayKey]): boolean {
  const parsedStart = parseHhMm(daySlot.startTime);
  const parsedEnd = parseHhMm(daySlot.endTime);
  if (!parsedStart || !parsedEnd) {
    return false;
  }
  return parsedStart.minutesSinceStartOfDay > parsedEnd.minutesSinceStartOfDay;
}

/**
 * SettingsModal Component
 * Modal dialog for configuring timer and weekly schedule settings.
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
  const autoRunFeatureEnabledStore = useSettingsStore(
    (state) => state.autoRunFeatureEnabled,
  );
  const postBreakTransitionPolicyStore = useSettingsStore(
    (state) => state.post_break_transition_policy,
  );
  const weeklyAutoRunConfigStore = useSettingsStore(
    (state) => state.weeklyAutoRunConfig,
  );
  const updateSettings = useSettingsStore((state) => state.updateSettings);
  const timerStatus = useTimerStore((state) => state.status);

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
  const [autoRunFeatureEnabled, setAutoRunFeatureEnabled] = useState<boolean>(
    autoRunFeatureEnabledStore,
  );
  const [postBreakTransitionPolicy, setPostBreakTransitionPolicy] = useState(
    postBreakTransitionPolicyStore,
  );
  const [weeklyAutoRunConfig, setWeeklyAutoRunConfig] =
    useState<WeeklyAutoRunConfig>(cloneWeeklyConfig(weeklyAutoRunConfigStore));
  const [dayScheduleValidationErrors, setDayScheduleValidationErrors] = useState<
    Partial<Record<WeekdayKey, DayScheduleValidationError>>
  >({});
  const [weeklyScheduleValidationErrors, setWeeklyScheduleValidationErrors] =
    useState<WeeklyScheduleValidationError[]>([]);

  // Sync local state with store when modal opens
  useEffect(() => {
    if (!isOpen) {
      return;
    }

    const weeklyConfigClone = cloneWeeklyConfig(weeklyAutoRunConfigStore);
    const validationResult = validateWeeklyAutoRunConfig(weeklyConfigClone);

    setWorkDuration(workDurationStore);
    setShortBreakDuration(shortBreakDurationStore);
    setLongBreakDuration(longBreakDurationStore);
    setLongBreakInterval(longBreakIntervalStore);
    setNotificationSoundVolumePercent(notificationSoundVolumePercentStore);
    setNotificationSoundMuted(notificationSoundMutedStore);
    setAutoRunFeatureEnabled(autoRunFeatureEnabledStore);
    setPostBreakTransitionPolicy(postBreakTransitionPolicyStore);
    setWeeklyAutoRunConfig(weeklyConfigClone);
    setDayScheduleValidationErrors(validationResult.dayScheduleValidationErrors);
    setWeeklyScheduleValidationErrors(validationResult.weeklyScheduleValidationErrors);
    setNotificationSoundPreviewType("work");
    setNotificationSoundPreviewStatusText("");
  }, [
    isOpen,
    workDurationStore,
    shortBreakDurationStore,
    longBreakDurationStore,
    longBreakIntervalStore,
    notificationSoundVolumePercentStore,
    notificationSoundMutedStore,
    autoRunFeatureEnabledStore,
    postBreakTransitionPolicyStore,
    weeklyAutoRunConfigStore,
  ]);

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
  const hasDayScheduleValidationErrors =
    Object.keys(dayScheduleValidationErrors).length > 0;
  const hasWeeklyScheduleValidationErrors =
    weeklyScheduleValidationErrors.length > 0;

  const isFormValid =
    isWorkDurationValid &&
    isShortBreakValid &&
    isLongBreakValid &&
    isIntervalValid &&
    isNotificationSoundVolumeValid &&
    !hasDayScheduleValidationErrors &&
    !hasWeeklyScheduleValidationErrors;
  const isTimerIdle = timerStatus === TimerStatus.IDLE;

  const updateWeeklyConfig = (updater: (draftConfig: WeeklyAutoRunConfig) => void) => {
    setWeeklyAutoRunConfig((previousConfig) => {
      const nextConfig = cloneWeeklyConfig(previousConfig);
      updater(nextConfig);

      const validationResult = validateWeeklyAutoRunConfig(nextConfig);
      setDayScheduleValidationErrors(validationResult.dayScheduleValidationErrors);
      setWeeklyScheduleValidationErrors(validationResult.weeklyScheduleValidationErrors);

      return nextConfig;
    });
  };

  const handleSave = () => {
    const validationResult = validateWeeklyAutoRunConfig(weeklyAutoRunConfig);
    setDayScheduleValidationErrors(validationResult.dayScheduleValidationErrors);
    setWeeklyScheduleValidationErrors(validationResult.weeklyScheduleValidationErrors);

    if (
      Object.keys(validationResult.dayScheduleValidationErrors).length > 0 ||
      validationResult.weeklyScheduleValidationErrors.length > 0
    ) {
      return;
    }

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
      autoRunFeatureEnabled,
      post_break_transition_policy: postBreakTransitionPolicy,
      weeklyAutoRunConfig,
    });

    onClose();
  };

  const handleCancel = () => {
    const weeklyConfigClone = cloneWeeklyConfig(weeklyAutoRunConfigStore);
    const validationResult = validateWeeklyAutoRunConfig(weeklyConfigClone);

    setWorkDuration(workDurationStore);
    setShortBreakDuration(shortBreakDurationStore);
    setLongBreakDuration(longBreakDurationStore);
    setLongBreakInterval(longBreakIntervalStore);
    setNotificationSoundVolumePercent(notificationSoundVolumePercentStore);
    setNotificationSoundMuted(notificationSoundMutedStore);
    setAutoRunFeatureEnabled(autoRunFeatureEnabledStore);
    setPostBreakTransitionPolicy(postBreakTransitionPolicyStore);
    setWeeklyAutoRunConfig(weeklyConfigClone);
    setDayScheduleValidationErrors(validationResult.dayScheduleValidationErrors);
    setWeeklyScheduleValidationErrors(validationResult.weeklyScheduleValidationErrors);

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
    <div className="settingsModalOverlay" data-testid="settings-modal">
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
            disabled={!isTimerIdle}
          />

          <DurationInput
            label="短い休憩"
            value={shortBreakDuration}
            onChange={setShortBreakDuration}
            min={VALIDATION_RULES.shortBreakDuration.min}
            max={VALIDATION_RULES.shortBreakDuration.max}
            testId="settings-short-break"
            disabled={!isTimerIdle}
          />

          <DurationInput
            label="長い休憩"
            value={longBreakDuration}
            onChange={setLongBreakDuration}
            min={VALIDATION_RULES.longBreakDuration.min}
            max={VALIDATION_RULES.longBreakDuration.max}
            testId="settings-long-break"
            disabled={!isTimerIdle}
          />

          <DurationInput
            label="長い休憩の間隔"
            value={longBreakInterval}
            onChange={setLongBreakInterval}
            min={VALIDATION_RULES.longBreakInterval.min}
            max={VALIDATION_RULES.longBreakInterval.max}
            unit="回"
            testId="settings-interval"
            disabled={!isTimerIdle}
          />

          {!isTimerIdle && (
            <p
              className="text-xs text-amber-700"
              data-testid="settings-duration-edit-disabled-message"
            >
              タイマー実行中は時間長設定を変更できません。曜日別スケジュールのみ編集できます。
            </p>
          )}

          <section
            className="rounded-lg border border-gray-200 p-3"
            data-testid="settings-auto-run-section"
          >
            <div className="mb-3 flex items-center justify-between gap-3">
              <div>
                <h3 className="text-sm font-semibold text-gray-800">曜日別自動実行</h3>
                <p className="text-xs text-gray-600">
                  曜日ごとに開始時刻と終了時刻を設定します。
                </p>
              </div>
              <label className="inline-flex items-center gap-2 text-sm text-gray-700">
                <input
                  type="checkbox"
                  checked={autoRunFeatureEnabled}
                  onChange={(event) => setAutoRunFeatureEnabled(event.target.checked)}
                  data-testid="settings-auto-run-feature-toggle"
                  aria-label="曜日別自動実行の全体切り替え"
                />
                有効化
              </label>
            </div>

            {weeklyScheduleValidationErrors.length > 0 && (
              <div
                className="mb-3 rounded-md border border-red-200 bg-red-50 p-2"
                data-testid="settings-auto-run-weekly-errors"
                role="alert"
              >
                {weeklyScheduleValidationErrors.map((error) => (
                  <p
                    key={`${error.primaryWeekday}-${error.secondaryWeekday}`}
                    className="text-xs text-red-700"
                  >
                    {error.message}
                  </p>
                ))}
              </div>
            )}

            <div className="grid gap-2">
              {WEEKDAY_KEYS.map((weekday) => {
                const daySlot = weeklyAutoRunConfig[weekday];
                const dayError = dayScheduleValidationErrors[weekday];
                const isOvernight = daySlot.enabled && isOvernightDaySlot(daySlot);
                const isDayTimeInputDisabled =
                  !autoRunFeatureEnabled || !daySlot.enabled;
                const dayTimeInputClass = `rounded border px-2 py-1 text-sm transition-colors ${
                  isDayTimeInputDisabled
                    ? "cursor-not-allowed border-gray-200 bg-gray-100 text-gray-400"
                    : "border-gray-300 bg-white text-gray-900"
                }`;

                return (
                  <div
                    key={weekday}
                    className={`rounded-md border px-2 py-2 transition-colors ${
                      isDayTimeInputDisabled
                        ? "border-gray-100 bg-gray-50"
                        : "border-gray-100 bg-white"
                    }`}
                    data-testid={`settings-auto-run-day-row-${weekday}`}
                  >
                    <div className="grid grid-cols-[3rem_1fr_1fr_auto] items-center gap-2">
                      <label className="inline-flex items-center gap-1 text-xs text-gray-700">
                        <input
                          type="checkbox"
                          checked={daySlot.enabled}
                          disabled={!autoRunFeatureEnabled}
                          onChange={(event) =>
                            updateWeeklyConfig((draftConfig) => {
                              draftConfig[weekday].enabled = event.target.checked;
                            })
                          }
                          data-testid={`settings-auto-run-day-enabled-${weekday}`}
                          aria-label={`${WEEKDAY_LABEL_BY_KEY[weekday]}曜日の自動実行を有効にする`}
                        />
                        <span>{WEEKDAY_LABEL_BY_KEY[weekday]}</span>
                      </label>

                      <input
                        type="time"
                        step={60}
                        value={daySlot.startTime}
                        onChange={(event) =>
                          updateWeeklyConfig((draftConfig) => {
                            draftConfig[weekday].startTime = event.target.value;
                          })
                        }
                        disabled={isDayTimeInputDisabled}
                        className={dayTimeInputClass}
                        data-testid={`settings-auto-run-day-start-${weekday}`}
                        aria-label={`${WEEKDAY_LABEL_BY_KEY[weekday]}曜日の開始時刻`}
                      />

                      <input
                        type="time"
                        step={60}
                        value={daySlot.endTime}
                        onChange={(event) =>
                          updateWeeklyConfig((draftConfig) => {
                            draftConfig[weekday].endTime = event.target.value;
                          })
                        }
                        disabled={isDayTimeInputDisabled}
                        className={dayTimeInputClass}
                        data-testid={`settings-auto-run-day-end-${weekday}`}
                        aria-label={`${WEEKDAY_LABEL_BY_KEY[weekday]}曜日の終了時刻`}
                      />

                      <span
                        className={`text-xs ${
                          isOvernight ? "text-blue-700" : "text-transparent"
                        }`}
                        data-testid={`settings-auto-run-day-overnight-${weekday}`}
                      >
                        翌日まで
                      </span>
                    </div>
                    {dayError && (
                      <p
                        className="mt-1 text-xs text-red-700"
                        data-testid={`settings-auto-run-day-error-${weekday}`}
                        role="alert"
                      >
                        {dayError.message}
                      </p>
                    )}
                  </div>
                );
              })}
            </div>
          </section>

          <section className="settingsPostBreakPolicySection">
            <div>
              <div className="settingsPostBreakPolicyHeader">
                <h3 className="text-sm font-semibold text-gray-800">休憩後の遷移</h3>
                {postBreakTransitionPolicy === "start_next_work_automatically" && (
                  <span
                    className="settingsPostBreakPolicyEnabledLabel"
                    data-testid="settings-post-break-policy-enabled-label"
                  >
                    自動継続ON
                  </span>
                )}
              </div>
              <p className="text-xs text-gray-600">
                休憩が終わったあとの作業開始方法を選択します。
              </p>
            </div>
            <fieldset
              className="settingsPostBreakPolicyGroup"
              data-testid="settings-post-break-policy-group"
              aria-label="休憩後の遷移ポリシー"
            >
              <label className="settingsPostBreakPolicyOption">
                <input
                  type="radio"
                  name="settings-post-break-policy"
                  value="require_manual_continue"
                  checked={postBreakTransitionPolicy === "require_manual_continue"}
                  onChange={() =>
                    setPostBreakTransitionPolicy("require_manual_continue")
                  }
                  data-testid="settings-post-break-policy-manual"
                  aria-label="毎回確認する"
                />
                <span>毎回確認する</span>
              </label>
              <label className="settingsPostBreakPolicyOption">
                <input
                  type="radio"
                  name="settings-post-break-policy"
                  value="start_next_work_automatically"
                  checked={
                    postBreakTransitionPolicy === "start_next_work_automatically"
                  }
                  onChange={() =>
                    setPostBreakTransitionPolicy("start_next_work_automatically")
                  }
                  data-testid="settings-post-break-policy-auto"
                  aria-label="休憩後に自動で次の作業を開始する"
                />
                <span>休憩後に自動で次の作業を開始する</span>
              </label>
            </fieldset>
          </section>

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
                value={notificationSoundVolumePercent ?? 0}
                onChange={(event) =>
                  setNotificationSoundVolumePercent(Number(event.target.value))
                }
                className="h-2 w-full cursor-pointer appearance-none rounded-lg bg-gray-200 accent-blue-600"
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
