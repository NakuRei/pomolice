import { beforeEach, describe, expect, it, vi } from "vitest";

const attachConsole = vi.fn().mockResolvedValue(() => undefined);

vi.mock("@tauri-apps/plugin-log", () => ({
  attachConsole,
}));

describe("initializeFrontendLogDevtools()", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.resetModules();
  });

  it("calls attachConsole only in development", async () => {
    const {
      setDevelopmentEnvironmentOverrideForTest,
      initializeFrontendLogDevtools,
    } = await import("./devtools");

    setDevelopmentEnvironmentOverrideForTest(true);

    await initializeFrontendLogDevtools();

    expect(attachConsole).toHaveBeenCalledTimes(1);
  });

  it("does not initialize twice", async () => {
    const {
      setDevelopmentEnvironmentOverrideForTest,
      initializeFrontendLogDevtools,
    } = await import("./devtools");

    setDevelopmentEnvironmentOverrideForTest(true);

    await initializeFrontendLogDevtools();
    await initializeFrontendLogDevtools();

    expect(attachConsole).toHaveBeenCalledTimes(1);
  });

  it("does not call attachConsole outside development", async () => {
    const {
      setDevelopmentEnvironmentOverrideForTest,
      initializeFrontendLogDevtools,
    } = await import("./devtools");

    setDevelopmentEnvironmentOverrideForTest(false);

    await initializeFrontendLogDevtools();

    expect(attachConsole).not.toHaveBeenCalled();
  });

  it("allows retry after attachConsole fails once", async () => {
    attachConsole
      .mockRejectedValueOnce(new Error("transient failure"))
      .mockResolvedValueOnce(() => undefined);

    const {
      setDevelopmentEnvironmentOverrideForTest,
      initializeFrontendLogDevtools,
    } = await import("./devtools");

    setDevelopmentEnvironmentOverrideForTest(true);

    await initializeFrontendLogDevtools();
    await initializeFrontendLogDevtools();

    expect(attachConsole).toHaveBeenCalledTimes(2);
  });
});
