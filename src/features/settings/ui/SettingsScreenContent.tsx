import { useBackendObservationStore } from "@/backend/observationStore";
import type { AppScreen, SettingsScreen } from "@/types/appScreen";
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
  const settings = useBackendObservationStore((state) => state.settings);
  const openSettingsTopScreen = (): void => {
    moveToScreen("settingsTop");
  };

  // Settings come from the backend; until it has answered there is nothing
  // to edit, so only the overview is shown.
  if (settings === null || currentScreen === "settingsTop") {
    return (
      <SettingsScreenLayout mainTestId="settings-top-main">
        <SettingsTopPage
          settings={settings}
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
  }

  switch (currentScreen) {
    case "settingsTimer":
      return (
        <SettingsScreenLayout mainTestId="settings-timer-main">
          <SettingsTimerPage
            settings={settings}
            openSettingsTopScreen={openSettingsTopScreen}
          />
        </SettingsScreenLayout>
      );
    case "settingsAutoRun":
      return (
        <SettingsScreenLayout mainTestId="settings-auto-run-main">
          <SettingsAutoRunPage
            settings={settings}
            openSettingsTopScreen={openSettingsTopScreen}
          />
        </SettingsScreenLayout>
      );
    case "settingsNotification":
      return (
        <SettingsScreenLayout mainTestId="settings-notification-main">
          <SettingsNotificationPage
            settings={settings}
            openSettingsTopScreen={openSettingsTopScreen}
          />
        </SettingsScreenLayout>
      );
    default:
      return assertNeverSettingsScreen(currentScreen);
  }
}
