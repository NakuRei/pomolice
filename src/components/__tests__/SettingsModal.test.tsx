import { describe, it, expect, beforeEach, vi } from "vitest";
import { render, screen, fireEvent } from "@testing-library/react";
import { SettingsModal } from "../Settings/SettingsModal";
import { useSettingsStore } from "../../store/settingsStore";

describe("SettingsModal", () => {
  const mockOnClose = vi.fn();

  beforeEach(() => {
    useSettingsStore.getState().reset();
    mockOnClose.mockClear();
  });

  describe("Visibility", () => {
    it("should not render when isOpen is false", () => {
      render(<SettingsModal isOpen={false} onClose={mockOnClose} />);
      const modal = screen.queryByTestId("settings-modal");
      expect(modal).not.toBeInTheDocument();
    });

    it("should render when isOpen is true", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toBeInTheDocument();
    });
  });

  describe("Initial Values", () => {
    it("should display current settings from store", () => {
      useSettingsStore.setState({
        workDuration: 30,
        shortBreakDuration: 10,
        longBreakDuration: 20,
        longBreakInterval: 3,
      });

      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;

      expect(workInput).toHaveValue(30);
      expect(shortBreakInput).toHaveValue(10);
      expect(longBreakInput).toHaveValue(20);
      expect(intervalInput).toHaveValue(3);
    });

    it("should display default settings", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;

      expect(workInput).toHaveValue(25);
      expect(shortBreakInput).toHaveValue(5);
      expect(longBreakInput).toHaveValue(15);
      expect(intervalInput).toHaveValue(4);
    });
  });

  describe("Modal Content", () => {
    it("should display modal title", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(screen.getByText("設定")).toBeInTheDocument();
    });

    it("should display all input labels", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(screen.getByText("作業時間")).toBeInTheDocument();
      expect(screen.getByText("短い休憩")).toBeInTheDocument();
      expect(screen.getByText("長い休憩")).toBeInTheDocument();
      expect(screen.getByText("長い休憩の間隔")).toBeInTheDocument();
    });

    it("should have save and cancel buttons", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      expect(screen.getByTestId("settings-save")).toBeInTheDocument();
      expect(screen.getByTestId("settings-cancel")).toBeInTheDocument();
    });
  });

  describe("Input Changes", () => {
    it("should update work duration input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });

      expect(workInput).toHaveValue(30);
    });

    it("should update short break input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      fireEvent.change(shortBreakInput, { target: { value: "10" } });

      expect(shortBreakInput).toHaveValue(10);
    });

    it("should update long break input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      fireEvent.change(longBreakInput, { target: { value: "20" } });

      expect(longBreakInput).toHaveValue(20);
    });

    it("should update interval input", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;
      fireEvent.change(intervalInput, { target: { value: "3" } });

      expect(intervalInput).toHaveValue(3);
    });
  });

  describe("Save Button", () => {
    it("should be enabled with valid inputs", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).not.toBeDisabled();
    });

    it("should be disabled with invalid work duration", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "0" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should be disabled with invalid short break", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      fireEvent.change(shortBreakInput, { target: { value: "100" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should be disabled with invalid long break", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      fireEvent.change(longBreakInput, { target: { value: "3" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should be disabled with invalid interval", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;
      fireEvent.change(intervalInput, { target: { value: "1" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).toBeDisabled();
    });

    it("should save settings and close modal", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });

      const saveButton = screen.getByTestId("settings-save");
      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(30);
      expect(mockOnClose).toHaveBeenCalledTimes(1);
    });

    it("should save all settings correctly", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const shortBreakInput = screen.getByTestId("settings-short-break-input") as HTMLInputElement;
      const longBreakInput = screen.getByTestId("settings-long-break-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;

      fireEvent.change(workInput, { target: { value: "40" } });
      fireEvent.change(shortBreakInput, { target: { value: "8" } });
      fireEvent.change(longBreakInput, { target: { value: "20" } });
      fireEvent.change(intervalInput, { target: { value: "3" } });

      const saveButton = screen.getByTestId("settings-save");
      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(40);
      expect(state.shortBreakDuration).toBe(8);
      expect(state.longBreakDuration).toBe(20);
      expect(state.longBreakInterval).toBe(3);
    });
  });

  describe("Cancel Button", () => {
    it("should close modal without saving", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });

      const cancelButton = screen.getByTestId("settings-cancel");
      fireEvent.click(cancelButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(25); // Original value
      expect(mockOnClose).toHaveBeenCalledTimes(1);
    });

    it("should reset form values on cancel", () => {
      const { rerender } = render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "30" } });
      expect(workInput).toHaveValue(30);

      const cancelButton = screen.getByTestId("settings-cancel");
      fireEvent.click(cancelButton);

      // Reopen modal
      rerender(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInputAfter = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      expect(workInputAfter).toHaveValue(25); // Reset to original
    });
  });

  describe("Modal Styling", () => {
    it("should have overlay background", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toHaveClass("bg-black/50");
    });

    it("should be positioned as fixed overlay", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toHaveClass("fixed");
      expect(modal).toHaveClass("inset-0");
    });

    it("should have high z-index", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);
      const modal = screen.getByTestId("settings-modal");
      expect(modal).toHaveClass("z-50");
    });
  });

  describe("Edge Cases", () => {
    it("should not save when save button is disabled", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      fireEvent.change(workInput, { target: { value: "0" } });

      const saveButton = screen.getByTestId("settings-save");
      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(25); // Unchanged
      expect(mockOnClose).not.toHaveBeenCalled();
    });

    it("should handle boundary values correctly", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;

      // Set to minimum valid values
      fireEvent.change(workInput, { target: { value: "1" } });
      fireEvent.change(intervalInput, { target: { value: "2" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).not.toBeDisabled();

      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(1);
      expect(state.longBreakInterval).toBe(2);
    });

    it("should handle maximum valid values", () => {
      render(<SettingsModal isOpen={true} onClose={mockOnClose} />);

      const workInput = screen.getByTestId("settings-work-duration-input") as HTMLInputElement;
      const intervalInput = screen.getByTestId("settings-interval-input") as HTMLInputElement;

      // Set to maximum valid values
      fireEvent.change(workInput, { target: { value: "120" } });
      fireEvent.change(intervalInput, { target: { value: "10" } });

      const saveButton = screen.getByTestId("settings-save");
      expect(saveButton).not.toBeDisabled();

      fireEvent.click(saveButton);

      const state = useSettingsStore.getState();
      expect(state.workDuration).toBe(120);
      expect(state.longBreakInterval).toBe(10);
    });
  });
});
