import { WEEKDAY_LABELS_BY_KEY } from "@/constants/weekdayLabels";
import { useFocusHeadingOnMount } from "@/hooks/useFocusHeadingOnMount";
import {
  useSettingsStore,
  WEEKDAY_KEYS,
  type WeeklyAutoRunConfig,
} from "@/store/settingsStore";
import type { PostBreakTransitionPolicy } from "@/types/sessionTransitionPolicy";
import { Button } from "../shared/Button";
import { SettingsPageHeader } from "./SettingsPageHeader";

interface SettingsTopPageProps {
  openHomeScreen: () => void;
  openTimerSettings: () => void;
  openAutoRunSettings: () => void;
  openNotificationSettings: () => void;
}

function buildTimerSummaryText(
  workDuration: number,
  shortBreakDuration: number,
  longBreakDuration: number,
  longBreakInterval: number,
): string {
  return `作業${String(workDuration)}分 / 短休憩${String(shortBreakDuration)}分 / 長休憩${String(longBreakDuration)}分 / 間隔${String(longBreakInterval)}回`;
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

  const baseWeekday = enabledWeekdays[0];
  const baseSlot = weeklyAutoRunConfig[baseWeekday];
  const allWeekdaysShareOneWindow = enabledWeekdays.every((weekday) => {
    const daySlot = weeklyAutoRunConfig[weekday];
    return (
      daySlot.startTime === baseSlot.startTime &&
      daySlot.endTime === baseSlot.endTime
    );
  });

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
}

function SettingsTopRowButton({
  label,
  summaryText,
  testId,
  openCategoryPage,
}: SettingsTopRowButtonProps): React.ReactElement {
  return (
    <button
      type="button"
      onClick={openCategoryPage}
      data-testid={testId}
      className="w-full rounded-lg border border-border bg-surface px-4 py-3 text-left transition-colors hover:border-border-strong hover:bg-surface-raised focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-primary-600"
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
  postBreakTransitionPolicy: PostBreakTransitionPolicy;
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
  openHomeScreen,
  openTimerSettings,
  openAutoRunSettings,
  openNotificationSettings,
}: SettingsTopPageProps): React.ReactElement {
  const topHeadingReference = useFocusHeadingOnMount();

  const workDuration = useSettingsStore((state) => state.workDuration);
  const shortBreakDuration = useSettingsStore(
    (state) => state.shortBreakDuration,
  );
  const longBreakDuration = useSettingsStore(
    (state) => state.longBreakDuration,
  );
  const longBreakInterval = useSettingsStore(
    (state) => state.longBreakInterval,
  );
  const autoRunFeatureEnabled = useSettingsStore(
    (state) => state.autoRunFeatureEnabled,
  );
  const weeklyAutoRunConfig = useSettingsStore(
    (state) => state.weeklyAutoRunConfig,
  );
  const postBreakTransitionPolicy = useSettingsStore(
    (state) => state.postBreakTransitionPolicy,
  );
  const setPostBreakTransitionPolicy = useSettingsStore(
    (state) => state.setPostBreakTransitionPolicy,
  );
  const notificationSoundMuted = useSettingsStore(
    (state) => state.notificationSoundMuted,
  );
  const notificationSoundVolumePercent = useSettingsStore(
    (state) => state.notificationSoundVolumePercent,
  );

  const timerSummaryText = buildTimerSummaryText(
    workDuration,
    shortBreakDuration,
    longBreakDuration,
    longBreakInterval,
  );
  const autoRunSummaryText = buildAutoRunSummaryText(
    autoRunFeatureEnabled,
    weeklyAutoRunConfig,
  );
  const postBreakPolicySummaryText = buildPostBreakPolicySummaryText(
    postBreakTransitionPolicy,
  );
  const switchPostBreakPolicyOnTopScreen = (): void => {
    const switchedPolicy = getSwitchedPostBreakTransitionPolicy(
      postBreakTransitionPolicy,
    );
    setPostBreakTransitionPolicy(switchedPolicy);
  };
  const notificationSummaryText = buildNotificationSummaryText(
    notificationSoundMuted,
    notificationSoundVolumePercent,
  );

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
        />
        <SettingsTopRowButton
          label="曜日別自動実行"
          summaryText={autoRunSummaryText}
          testId="settings-top-row-auto-run"
          openCategoryPage={openAutoRunSettings}
        />
        <SettingsTopPostBreakPolicyRow
          postBreakPolicySummaryText={postBreakPolicySummaryText}
          postBreakTransitionPolicy={postBreakTransitionPolicy}
          switchPostBreakPolicyOnTopScreen={switchPostBreakPolicyOnTopScreen}
        />
        <SettingsTopRowButton
          label="通知"
          summaryText={notificationSummaryText}
          testId="settings-top-row-notification"
          openCategoryPage={openNotificationSettings}
        />
      </div>
    </section>
  );
}
