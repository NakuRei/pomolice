import type { ReactNode } from "react";
import { InfoMessageBanner } from "@/features/timer/ui/InfoMessageBanner";
import { OperationNotice } from "@/components/shared/OperationNotice";
import { SettingsFileNotice } from "./SettingsFileNotice";

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
      <SettingsFileNotice />
      <InfoMessageBanner />
      <OperationNotice operation="update_settings" />

      <main className="flex-1" data-testid={mainTestId}>
        {children}
      </main>
    </div>
  );
}
