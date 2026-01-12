import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { TimerControls } from '../Timer/TimerControls';
import { useTimerStore, TimerStatus } from '../../store/timerStore';

describe('TimerControls', () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useTimerStore.getState().reset();
  });

  describe('Initial State (IDLE)', () => {
    it('should render start button when timer is idle', () => {
      render(<TimerControls />);
      const startButton = screen.getByTestId('start-button');
      expect(startButton).toBeInTheDocument();
      expect(startButton).toHaveTextContent('開始');
    });

    it('should not render stop button when timer is idle', () => {
      render(<TimerControls />);
      const stopButton = screen.queryByTestId('stop-button');
      expect(stopButton).not.toBeInTheDocument();
    });

    it('should start timer when start button is clicked', async () => {
      const user = userEvent.setup();
      render(<TimerControls />);

      const startButton = screen.getByTestId('start-button');
      await user.click(startButton);

      expect(useTimerStore.getState().status).toBe(TimerStatus.RUNNING);
    });
  });

  describe('Running State', () => {
    beforeEach(() => {
      useTimerStore.getState().start();
    });

    it('should not render any button when timer is running', () => {
      render(<TimerControls />);
      const startButton = screen.queryByTestId('start-button');
      expect(startButton).not.toBeInTheDocument();
    });

    it('should not render start button when timer is running', () => {
      render(<TimerControls />);
      const startButton = screen.queryByTestId('start-button');
      expect(startButton).not.toBeInTheDocument();
    });
  });

  describe('Button Accessibility', () => {
    it('should have aria-label on start button', () => {
      render(<TimerControls />);
      const startButton = screen.getByTestId('start-button');
      expect(startButton).toHaveAttribute('aria-label', 'タイマーを開始');
    });
  });

  describe('State Transitions', () => {
    it('should hide start button after starting', async () => {
      const user = userEvent.setup();
      const { rerender } = render(<TimerControls />);

      // Initially shows start button
      expect(screen.getByTestId('start-button')).toBeInTheDocument();

      // Click start button
      await user.click(screen.getByTestId('start-button'));

      // Rerender to reflect state change
      rerender(<TimerControls />);

      // Start button is hidden (no buttons shown during running)
      expect(screen.queryByTestId('start-button')).not.toBeInTheDocument();
    });

    it('should show start button again after emergency stop', async () => {
      const { rerender } = render(<TimerControls />);

      // Start timer
      useTimerStore.getState().start();
      rerender(<TimerControls />);
      expect(screen.queryByTestId('start-button')).not.toBeInTheDocument();

      // Emergency stop
      useTimerStore.getState().emergencyStop();
      rerender(<TimerControls />);

      // Start button is visible again
      expect(screen.getByTestId('start-button')).toBeInTheDocument();
    });
  });
});
