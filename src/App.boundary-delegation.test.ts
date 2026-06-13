import { describe, expect, it } from "vitest";
import appSource from "./App.tsx?raw";

describe("App boundary delegation", () => {
  it("delegates fullscreen and settings error boundaries to named components", () => {
    expect(appSource).toContain("<FullscreenBreakErrorBoundary");
    expect(appSource).toContain("<SettingsErrorBoundary");
    expect(appSource).not.toContain('boundary_scope: "fullscreen_break"');
    expect(appSource).not.toContain('boundary_scope: "settings"');
    expect(appSource).not.toContain('from "react-error-boundary"');
  });
});
