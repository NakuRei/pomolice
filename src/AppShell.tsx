import type { ReactNode } from "react";
import { FullscreenBreakErrorBoundary } from "./components/ErrorBoundary/FullscreenBreakErrorBoundary";
import { SettingsErrorBoundary } from "./components/ErrorBoundary/SettingsErrorBoundary";
import {
  SETTINGS_SCREEN_VALUES,
  type AppScreen,
  type SettingsScreen,
} from "./types/appScreen";

interface AppShellProps {
  currentScreen: AppScreen;
  isFullscreenBreak: boolean;
  onBackToHome: () => void;
  renderFullscreenBreakScreen: () => ReactNode;
  renderHomeScreen: () => ReactNode;
  renderSettingsScreen: (currentScreen: SettingsScreen) => ReactNode;
}

interface ScreenContentRendererProps {
  renderScreen: () => ReactNode;
}

interface SettingsScreenContentRendererProps {
  currentScreen: SettingsScreen;
  renderSettingsScreen: (currentScreen: SettingsScreen) => ReactNode;
}

function isSettingsScreen(screen: AppScreen): screen is SettingsScreen {
  return SETTINGS_SCREEN_VALUES.includes(screen as SettingsScreen);
}

function ScreenContentRenderer({
  renderScreen,
}: ScreenContentRendererProps): React.ReactElement {
  return <>{renderScreen()}</>;
}

function SettingsScreenContentRenderer({
  currentScreen,
  renderSettingsScreen,
}: SettingsScreenContentRendererProps): React.ReactElement {
  return <>{renderSettingsScreen(currentScreen)}</>;
}

export function AppShell({
  currentScreen,
  isFullscreenBreak,
  onBackToHome,
  renderFullscreenBreakScreen,
  renderHomeScreen,
  renderSettingsScreen,
}: AppShellProps): React.ReactElement {
  if (isFullscreenBreak) {
    return (
      <FullscreenBreakErrorBoundary>
        <ScreenContentRenderer renderScreen={renderFullscreenBreakScreen} />
      </FullscreenBreakErrorBoundary>
    );
  }

  if (isSettingsScreen(currentScreen)) {
    return (
      <SettingsErrorBoundary
        currentScreen={currentScreen}
        onBackToHome={onBackToHome}
      >
        <SettingsScreenContentRenderer
          currentScreen={currentScreen}
          renderSettingsScreen={renderSettingsScreen}
        />
      </SettingsErrorBoundary>
    );
  }

  return <ScreenContentRenderer renderScreen={renderHomeScreen} />;
}
