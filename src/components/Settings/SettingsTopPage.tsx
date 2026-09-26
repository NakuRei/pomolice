import { WEEKDAY_LABELS_BY_KEY } from "@/constants/weekdayLabels";
import { useFocusHeadingOnMount } from "@/hooks/useFocusHeadingOnMount";
import { useTimerStore } from "@/store/timerStore";
import type { PostBreakTransitionPolicy } from "@/types/sessionTransitionPolicy";
import type { Settings } from "@/types/settings";
import { WEEKDAY_KEYS, type WeeklyAutoRunConfig } from "@/types/weeklyAutoRun";
import { Button } from "../shared/Button";
import { formatDurationText } from "../shared/durationText";
import { SettingsPageHeader } from "./SettingsPageHeader";

/** Shown in place of every value until the backend has sent the settings. */
const SETTINGS_NOT_LOADED_TEXT = "読み込み中";

interface SettingsTopPageProps {
  settings: Settings | null;
  openHomeScreen: () => void;
  openTimerSettings: () => void;
  openAutoRunSettings: () => void;
  openNotificationSettings: () => void;
}

function buildTimerSummaryText(settings: Settings): string {
  return `作業${formatDurationText(settings.workDurationSecs)} / 短休憩${formatDurationText(settings.shortBreakDurationSecs)} / 長休憩${formatDurationText(settings.longBreakDurationSecs)} / 間隔${String(settings.longBreakInterval)}回`;
}

function buildAutoRunSummaryText(
  autoRunFeatureEnabled: boolean,
  weeklyAutoRunConfig: WeeklyAutoRunConfig,
): string {
  if (!autoRunFeatureEnabled) {
    return "OFF";
  }

  const enabledWeekdays = WEEKDAY_KEYS.filter(
    (weekday) => weeklyAutoRunConfig[weekday].enabled,
  );

  if (enabledWeekdays.length === 0) {
    return "ON（有効曜日なし）";
  }

  const [baseWeekday, ...remainingEnabledWeekdays] = enabledWeekdays;
  if (baseWeekday === undefined) {
    return "ON（有効曜日なし）";
  }
  const baseSlot = weeklyAutoRunConfig[baseWeekday];
  const allWeekdaysShareOneWindow = remainingEnabledWeekdays.every(
    (weekday) => {
      const daySlot = weeklyAutoRunConfig[weekday];
      return (
        daySlot.startTime === baseSlot.startTime &&
        daySlot.endTime === baseSlot.endTime
      );
    },
  );

  if (allWeekdaysShareOneWindow) {
    const weekdayLabelSummary = enabledWeekdays
      .map((weekday) => WEEKDAY_LABELS_BY_KEY[weekday].short)
      .join("");
    return `${weekdayLabelSummary} ${baseSlot.startTime}-${baseSlot.endTime}`;
  }

  return `有効${String(enabledWeekdays.length)}日（詳細参照）`;
}

function buildPostBreakPolicySummaryText(
  postBreakTransitionPolicy: PostBreakTransitionPolicy,
): string {
  if (postBreakTransitionPolicy === "start_next_work_automatically") {
    return "休憩後に自動で次の作業を開始する";
  }

  return "毎回確認する";
}

function getSwitchedPostBreakTransitionPolicy(
  currentPolicy: PostBreakTransitionPolicy,
): PostBreakTransitionPolicy {
  if (currentPolicy === "start_next_work_automatically") {
    return "require_manual_continue";
  }

  return "start_next_work_automatically";
}

function buildNotificationSummaryText(
  notificationSoundMuted: boolean,
  notificationSoundVolumePercent: number,
): string {
  return `ミュート${notificationSoundMuted ? "ON" : "OFF"} / 音量${String(notificationSoundVolumePercent)}%`;
}

interface SettingsTopRowButtonProps {
  label: string;
  summaryText: string;
  testId: string;
  openCategoryPage: () => void;
  disabled: boolean;
}

function SettingsTopRowButton({
  label,
  summaryText,
  testId,
  openCategoryPage,
  disabled,
}: SettingsTopRowButtonProps): React.ReactElement {
  return (
    <button
      type="button"
      onClick={openCategoryPage}
      disabled={disabled}
      data-testid={testId}
      className="w-full rounded-lg border border-border bg-surface px-4 py-3 text-left transition-colors hover:border-border-strong hover:bg-surface-raised focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent-ring disabled:cursor-not-allowed disabled:border-disabled-border disabled:bg-disabled-bg disabled:text-disabled-text"
    >
      <span className="block text-sm font-semibold text-text">{label}</span>
      <span className="mt-1 block truncate text-sm text-text-muted">
        {summaryText}
      </span>
    </button>
  );
}

interface SettingsTopPostBreakPolicyRowProps {
  postBreakPolicySummaryText: string;
  postBreakTransitionPolicy: PostBreakTransitionPolicy | null;
  switchPostBreakPolicyOnTopScreen: () => void;
}

function SettingsTopPostBreakPolicyRow({
  postBreakPolicySummaryText,
  postBreakTransitionPolicy,
  switchPostBreakPolicyOnTopScreen,
}: SettingsTopPostBreakPolicyRowProps): React.ReactElement {
  const autoContinueIsEnabled =
    postBreakTransitionPolicy === "start_next_work_automatically";
  const postBreakToggleButtonText = autoContinueIsEnabled
    ? "毎回確認に戻す"
    : "自動継続をONにする";

  return (
    <section
      className="w-full rounded-lg border border-border bg-surface px-4 py-3"
      data-testid="settings-top-row-post-break-policy"
    >
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <span className="block text-sm font-semibold text-text">
            休憩後遷移
          </span>
          <span className="mt-1 block truncate text-sm text-text-muted">
            {postBreakPolicySummaryText}
          </span>
        </div>
        <Button
          appearance="secondary"
          size="sm"
          onClick={switchPostBreakPolicyOnTopScreen}
          disabled={postBreakTransitionPolicy === null}
          testId="settings-top-post-break-policy-toggle"
          aria-pressed={autoContinueIsEnabled}
          className="shrink-0"
        >
          {postBreakToggleButtonText}
        </Button>
      </div>
    </section>
  );
}

export function SettingsTopPage({
  settings,
  openHomeScreen,
  openTimerSettings,
  openAutoRunSettings,
  openNotificationSettings,
}: SettingsTopPageProps): React.ReactElement {
  const topHeadingReference = useFocusHeadingOnMount();
  const savePostBreakTransitionPolicy = useTimerStore(
    (state) => state.savePostBreakTransitionPolicy,
  );

  const timerSummaryText =
    settings === null
      ? SETTINGS_NOT_LOADED_TEXT
      : buildTimerSummaryText(settings);
  const autoRunSummaryText =
    settings === null
      ? SETTINGS_NOT_LOADED_TEXT
      : buildAutoRunSummaryText(
          settings.autoRunFeatureEnabled,
          settings.weeklyAutoRunConfig,
        );
  const postBreakPolicySummaryText =
    settings === null
      ? SETTINGS_NOT_LOADED_TEXT
      : buildPostBreakPolicySummaryText(settings.postBreakTransitionPolicy);
  const notificationSummaryText =
    settings === null
      ? SETTINGS_NOT_LOADED_TEXT
      : buildNotificationSummaryText(
          settings.notificationSoundMuted,
          settings.notificationSoundVolumePercent,
        );
  const switchPostBreakPolicyOnTopScreen = (): void => {
    if (settings === null) {
      return;
    }
    void savePostBreakTransitionPolicy(
      getSwitchedPostBreakTransitionPolicy(settings.postBreakTransitionPolicy),
    );
  };

  return (
    <section
      className="mx-auto w-full max-w-2xl"
      data-testid="settings-top-page"
    >
      <SettingsPageHeader
        title="設定"
        description="ここでは現在値の要約を表示します。編集したいカテゴリを選択してください。"
        headingRef={topHeadingReference}
        headingTestId="settings-top-heading"
        backButtonLabel="ホームへ戻る"
        backButtonTestId="settings-top-back-home"
        onBackButtonClick={openHomeScreen}
      />

      <div className="space-y-3">
        <SettingsTopRowButton
          label="タイマー設定"
          summaryText={timerSummaryText}
          testId="settings-top-row-timer"
          openCategoryPage={openTimerSettings}
          disabled={settings === null}
        />
        <SettingsTopRowButton
          label="曜日別自動実行"
          summaryText={autoRunSummaryText}
          testId="settings-top-row-auto-run"
          openCategoryPage={openAutoRunSettings}
          disabled={settings === null}
        />
        <SettingsTopPostBreakPolicyRow
          postBreakPolicySummaryText={postBreakPolicySummaryText}
          postBreakTransitionPolicy={
            settings?.postBreakTransitionPolicy ?? null
          }
          switchPostBreakPolicyOnTopScreen={switchPostBreakPolicyOnTopScreen}
        />
        <SettingsTopRowButton
          label="通知"
          summaryText={notificationSummaryText}
          testId="settings-top-row-notification"
          openCategoryPage={openNotificationSettings}
          disabled={settings === null}
        />
      </div>
    </section>
  );
}
