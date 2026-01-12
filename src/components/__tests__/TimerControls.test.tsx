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

    it('should render stop button when timer is running', () => {
      render(<TimerControls />);
      const stopButton = screen.getByTestId('stop-button');
      expect(stopButton).toBeInTheDocument();
      expect(stopButton).toHaveTextContent('停止');
    });

    it('should not render start button when timer is running', () => {
      render(<TimerControls />);
      const startButton = screen.queryByTestId('start-button');
      expect(startButton).not.toBeInTheDocument();
    });

    it('should stop timer when stop button is clicked', async () => {
      const user = userEvent.setup();
      render(<TimerControls />);

      const stopButton = screen.getByTestId('stop-button');
      await user.click(stopButton);

      expect(useTimerStore.getState().status).toBe(TimerStatus.IDLE);
    });
  });

  describe('Button Accessibility', () => {
    it('should have aria-label on start button', () => {
      render(<TimerControls />);
      const startButton = screen.getByTestId('start-button');
      expect(startButton).toHaveAttribute('aria-label', 'タイマーを開始');
    });

    it('should have aria-label on stop button', () => {
      useTimerStore.getState().start();
      render(<TimerControls />);
      const stopButton = screen.getByTestId('stop-button');
      expect(stopButton).toHaveAttribute('aria-label', 'タイマーを停止');
    });
  });

  describe('State Transitions', () => {
    it('should switch from start button to stop button after starting', async () => {
      const user = userEvent.setup();
      const { rerender } = render(<TimerControls />);

      // Initially shows start button
      expect(screen.getByTestId('start-button')).toBeInTheDocument();
      expect(screen.queryByTestId('stop-button')).not.toBeInTheDocument();

      // Click start button
      await user.click(screen.getByTestId('start-button'));

      // Rerender to reflect state change
      rerender(<TimerControls />);

      // Now shows stop button
      expect(screen.queryByTestId('start-button')).not.toBeInTheDocument();
      expect(screen.getByTestId('stop-button')).toBeInTheDocument();
    });

    it('should switch from stop button to start button after stopping', async () => {
      const user = userEvent.setup();
      useTimerStore.getState().start();
      const { rerender } = render(<TimerControls />);

      // Initially shows stop button
      expect(screen.getByTestId('stop-button')).toBeInTheDocument();
      expect(screen.queryByTestId('start-button')).not.toBeInTheDocument();

      // Click stop button
      await user.click(screen.getByTestId('stop-button'));

      // Rerender to reflect state change
      rerender(<TimerControls />);

      // Now shows start button
      expect(screen.getByTestId('start-button')).toBeInTheDocument();
      expect(screen.queryByTestId('stop-button')).not.toBeInTheDocument();
    });
  });
});
