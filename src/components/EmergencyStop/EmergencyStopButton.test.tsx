import { useTimerObservationStore } from "@/store/timerObservationStore";
import { createTimerSnapshotForTest } from "@/test/timerEffectsFixture";
import { beforeEach, describe, expect, it, vi } from "vitest";
import {
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { EmergencyStopButton } from "./EmergencyStopButton";
import { useTimerOperationStore } from "@/operations/operationStore";

// Mock Tauri invoke
vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockImplementation(() =>
    Promise.resolve({
      status: "accepted",
      value: createTimerSnapshotForTest(),
    }),
  ),
}));

function createRootFixture(): HTMLDivElement {
  document.body.innerHTML = "";

  const root = document.createElement("div");
  root.id = "root";
  document.body.appendChild(root);

  return root;
}

function isRootInert(root: HTMLElement): boolean {
  const inertValue = (root as HTMLElement & { inert?: boolean }).inert;

  return inertValue || root.hasAttribute("inert");
}

function expectRootHidden(root: HTMLElement, isHidden: boolean): void {
  if (isHidden) {
    expect(root).toHaveAttribute("aria-hidden", "true");
  } else {
    expect(root).not.toHaveAttribute("aria-hidden");
  }

  expect(isRootInert(root)).toBe(isHidden);
}

function renderEmergencyStopButton(): HTMLDivElement {
  const root = createRootFixture();
  render(<EmergencyStopButton />, { container: root });
  return root;
}

function renderWorkingEmergencyStopButton(): HTMLDivElement {
  useTimerObservationStore.setState({ status: "working" });
  return renderEmergencyStopButton();
}

async function openEmergencyStopDialog(
  user: ReturnType<typeof userEvent.setup>,
): Promise<HTMLElement> {
  await user.click(screen.getByRole("button", { name: "緊急停止" }));
  return screen.getByRole("dialog", { name: "緊急停止しますか？" });
}

describe("EmergencyStopButton", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    // Reset store to initial state
    useTimerOperationStore.setState(
      useTimerOperationStore.getInitialState(),
      true,
    );
    useTimerObservationStore.setState({
      status: "idle",
      remainingSeconds: 25 * 60,
      sessionType: "work",
      currentCycle: 0,
      completedPomodoros: 0,
      breakInteractionCount: 0,
      requiresManualContinueDecision: false,
    });
  });
  describe("Visibility", () => {
    it("should not render when timer is idle", () => {
      renderEmergencyStopButton();
      const button = screen.queryByRole("button", { name: "緊急停止" });
      expect(button).not.toBeInTheDocument();
    });

    it("should render a named emergency stop button when timer is working", () => {
      renderWorkingEmergencyStopButton();
      const button = screen.getByRole("button", { name: "緊急停止" });
      expect(button).toHaveTextContent("緊急停止");
      expect(button).toHaveAttribute("aria-label", "緊急停止");
    });
  });

  describe("Confirmation Dialog", () => {
    it("should open an accessible confirmation dialog with warning text and actions", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);

      expect(dialog).toHaveAttribute("aria-modal", "true");
      expect(
        within(dialog).getByText(/現在のポモドーロセッションは記録されません/u),
      ).toBeInTheDocument();
      expect(
        within(dialog).getByRole("button", { name: "キャンセル" }),
      ).toBeInTheDocument();
      expect(
        within(dialog).getByRole("button", { name: "停止する" }),
      ).toBeInTheDocument();
    });

    it("keeps Tab and Shift+Tab focus cycling inside the dialog after portal migration", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);
      const cancelButton = within(dialog).getByRole("button", {
        name: "キャンセル",
      });
      const confirmButton = within(dialog).getByRole("button", {
        name: "停止する",
      });

      await waitFor(() => {
        expect(cancelButton).toHaveFocus();
      });

      await user.tab();
      expect(confirmButton).toHaveFocus();

      await user.tab();
      expect(cancelButton).toHaveFocus();

      await user.tab({ shift: true });
      expect(confirmButton).toHaveFocus();
    });
  });

  describe("Dialog Actions", () => {
    it("should close dialog when cancel is clicked", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);

      await user.click(
        within(dialog).getByRole("button", { name: "キャンセル" }),
      );

      await waitFor(() => {
        expect(
          screen.queryByRole("dialog", { name: "緊急停止しますか？" }),
        ).not.toBeInTheDocument();
      });
    });

    it("returns focus to the emergency stop trigger after cancel", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();
      const trigger = screen.getByRole("button", { name: "緊急停止" });

      const dialog = await openEmergencyStopDialog(user);
      await user.click(
        within(dialog).getByRole("button", { name: "キャンセル" }),
      );

      await waitFor(() => {
        expect(trigger).toHaveFocus();
      });
    });

    it("should focus the cancel button when the dialog opens", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);

      await waitFor(() => {
        expect(
          within(dialog).getByRole("button", { name: "キャンセル" }),
        ).toHaveFocus();
      });
    });

    it("should close the dialog on Escape without calling the backend", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      await openEmergencyStopDialog(user);
      fireEvent.keyDown(window, { key: "Escape" });

      await waitFor(() => {
        expect(
          screen.queryByRole("dialog", { name: "緊急停止しますか？" }),
        ).not.toBeInTheDocument();
      });
      expect(invoke).not.toHaveBeenCalled();
    });

    it("returns focus to the emergency stop trigger after Escape", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();
      const trigger = screen.getByRole("button", { name: "緊急停止" });

      await openEmergencyStopDialog(user);
      fireEvent.keyDown(window, { key: "Escape" });

      await waitFor(() => {
        expect(trigger).toHaveFocus();
      });
    });

    it("should not change timer status when cancel is clicked", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);
      await user.click(
        within(dialog).getByRole("button", { name: "キャンセル" }),
      );

      expect(useTimerObservationStore.getState().status).toBe("working");
    });

    it("should close dialog after confirming", async () => {
      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);
      await user.click(
        within(dialog).getByRole("button", { name: "停止する" }),
      );

      await waitFor(() => {
        expect(
          screen.queryByRole("dialog", { name: "緊急停止しますか？" }),
        ).not.toBeInTheDocument();
      });
    });

    it("should keep the dialog open and show an error when confirm fails", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      vi.mocked(invoke).mockResolvedValueOnce({
        status: "rejected",
        error: {
          code: "rate_limited",
          message: "please wait 300ms",
        },
      });

      const user = userEvent.setup();
      renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);
      await user.click(
        within(dialog).getByRole("button", { name: "停止する" }),
      );

      await waitFor(() => {
        expect(
          screen.getByRole("dialog", { name: "緊急停止しますか？" }),
        ).toBeInTheDocument();
      });
      expect(
        within(
          screen.getByRole("dialog", { name: "緊急停止しますか？" }),
        ).getByRole("alert"),
      ).toHaveTextContent("少々お待ちください");
    });

    it("marks #root inert while the dialog is open and removes inert after the dialog closes", async () => {
      const user = userEvent.setup();
      const root = renderWorkingEmergencyStopButton();

      const dialog = await openEmergencyStopDialog(user);
      expectRootHidden(root, true);

      await user.click(
        within(dialog).getByRole("button", { name: "キャンセル" }),
      );

      await waitFor(() => {
        expectRootHidden(root, false);
      });
    });
  });

  describe("Emergency Stop Behavior", () => {
    it("should call backend emergency_stop command", async () => {
      const { invoke } = await import("@tauri-apps/api/core");
      const user = userEvent.setup();
      useTimerObservationStore.setState({
        status: "working",
        remainingSeconds: 500,
        currentCycle: 2,
      });
      render(<EmergencyStopButton />);

      const dialog = await openEmergencyStopDialog(user);
      await user.click(
        within(dialog).getByRole("button", { name: "停止する" }),
      );

      expect(invoke).toHaveBeenCalledWith("emergency_stop");
    });

    it("should preserve pomodoro count (handled by backend)", async () => {
      const user = userEvent.setup();
      useTimerObservationStore.setState({
        status: "working",
        completedPomodoros: 3,
        currentCycle: 3,
      });
      render(<EmergencyStopButton />);

      const dialog = await openEmergencyStopDialog(user);
      await user.click(
        within(dialog).getByRole("button", { name: "停止する" }),
      );

      // The backend handles state, so pomodoro count is preserved until backend responds
      const state = useTimerObservationStore.getState();
      expect(state.completedPomodoros).toBe(3);
    });
  });
});
