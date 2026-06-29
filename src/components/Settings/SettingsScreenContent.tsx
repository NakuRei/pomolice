import type { AppScreen, SettingsScreen } from "../../types/appScreen";
import { SettingsAutoRunPage } from "./SettingsAutoRunPage";
import { SettingsNotificationPage } from "./SettingsNotificationPage";
import { SettingsScreenLayout } from "./SettingsScreenLayout";
import { SettingsTopPage } from "./SettingsTopPage";
import { SettingsTimerPage } from "./SettingsTimerPage";

interface SettingsScreenContentProps {
  currentScreen: SettingsScreen;
  moveToScreen: (nextScreen: AppScreen) => void;
}

function assertNeverSettingsScreen(screen: never): never {
  throw new Error(`Unknown settings screen: ${String(screen)}`);
}

export function SettingsScreenContent({
  currentScreen,
  moveToScreen,
}: SettingsScreenContentProps): React.ReactElement {
  switch (currentScreen) {
    case "settingsTop":
      return (
        <SettingsScreenLayout mainTestId="settings-top-main">
          <SettingsTopPage
            openHomeScreen={() => {
              moveToScreen("home");
            }}
            openTimerSettings={() => {
              moveToScreen("settingsTimer");
            }}
            openAutoRunSettings={() => {
              moveToScreen("settingsAutoRun");
            }}
            openNotificationSettings={() => {
              moveToScreen("settingsNotification");
            }}
          />
        </SettingsScreenLayout>
      );
    case "settingsTimer":
      return (
        <SettingsScreenLayout mainTestId="settings-timer-main">
          <SettingsTimerPage
            openSettingsTopScreen={() => {
              moveToScreen("settingsTop");
            }}
          />
        </SettingsScreenLayout>
      );
    case "settingsAutoRun":
      return (
        <SettingsScreenLayout mainTestId="settings-auto-run-main">
          <SettingsAutoRunPage
            openSettingsTopScreen={() => {
              moveToScreen("settingsTop");
            }}
          />
        </SettingsScreenLayout>
      );
    case "settingsNotification":
      return (
        <SettingsScreenLayout mainTestId="settings-notification-main">
          <SettingsNotificationPage
            openSettingsTopScreen={() => {
              moveToScreen("settingsTop");
            }}
          />
        </SettingsScreenLayout>
      );
    default:
      return assertNeverSettingsScreen(currentScreen);
  }
}
