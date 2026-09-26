import type { ReactNode } from "react";
import { ConnectionBanner } from "../shared/ConnectionBanner";
import { InfoMessageBanner } from "../shared/InfoMessageBanner";
import { InlineError } from "../shared/InlineError";
import { SettingsFileNotice } from "../shared/SettingsFileNotice";

interface SettingsScreenLayoutProps {
  children: ReactNode;
  mainTestId: string;
}

export function SettingsScreenLayout({
  children,
  mainTestId,
}: SettingsScreenLayoutProps): React.ReactElement {
  return (
    <div className="flex min-h-screen flex-col space-y-6 bg-background p-8 text-on-background">
      <ConnectionBanner />
      <SettingsFileNotice />
      <InfoMessageBanner />
      <InlineError errorId="update_settings" />

      <main className="flex-1" data-testid={mainTestId}>
        {children}
      </main>
    </div>
  );
}
