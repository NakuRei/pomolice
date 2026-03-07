import type { ReactNode } from "react";
import { ConnectionBanner } from "../shared/ConnectionBanner";
import { InfoMessageBanner } from "../shared/InfoMessageBanner";

interface SettingsScreenLayoutProps {
  children: ReactNode;
  mainTestId: string;
}

export function SettingsScreenLayout({
  children,
  mainTestId,
}: SettingsScreenLayoutProps) {
  return (
    <div className="flex min-h-screen flex-col p-8 space-y-6">
      <ConnectionBanner />
      <InfoMessageBanner />

      <main className="flex-1" data-testid={mainTestId}>
        {children}
      </main>
    </div>
  );
}
