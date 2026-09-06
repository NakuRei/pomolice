import { useSettingsStore } from "@/store/settingsStore";

export function resetSettingsStoreForTest(): void {
  useSettingsStore.setState(useSettingsStore.getInitialState(), true);
}
