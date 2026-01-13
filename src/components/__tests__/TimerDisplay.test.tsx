import { describe, it, expect, beforeEach } from 'vitest';
import { render, screen } from '@testing-library/react';
import { TimerDisplay } from '../Timer/TimerDisplay';
import { useTimerStore, TimerStatus, SessionType } from '../../store/timerStore';

describe('TimerDisplay', () => {
  beforeEach(() => {
    // Reset store to initial state before each test
    useTimerStore.getState().reset();
  });

  it('should render the timer display', () => {
    render(<TimerDisplay />);
    const timerElement = screen.getByTestId('timer-display');
    expect(timerElement).toBeInTheDocument();
  });

  it('should display initial time as 25:00', () => {
    render(<TimerDisplay />);
    const timerElement = screen.getByTestId('timer-display');
    expect(timerElement).toHaveTextContent('25:00');
  });

  it('should display work session label', () => {
    render(<TimerDisplay />);
    expect(screen.getByText('作業時間')).toBeInTheDocument();
  });

  it('should display short break session label', () => {
    useTimerStore.setState({ sessionType: SessionType.SHORT_BREAK });
    render(<TimerDisplay />);
    expect(screen.getByText('短い休憩')).toBeInTheDocument();
  });

  it('should display long break session label', () => {
    useTimerStore.setState({ sessionType: SessionType.LONG_BREAK });
    render(<TimerDisplay />);
    expect(screen.getByText('長い休憩')).toBeInTheDocument();
  });

  it('should display idle status', () => {
    render(<TimerDisplay />);
    expect(screen.getByText('待機中')).toBeInTheDocument();
  });

  it('should display running status', () => {
    useTimerStore.setState({ status: TimerStatus.RUNNING });
    render(<TimerDisplay />);
    expect(screen.getByText('実行中')).toBeInTheDocument();
  });

  it('should display paused status', () => {
    useTimerStore.setState({ status: TimerStatus.PAUSED });
    render(<TimerDisplay />);
    expect(screen.getByText('一時停止')).toBeInTheDocument();
  });

  it('should update display when remaining seconds change', () => {
    const { rerender } = render(<TimerDisplay />);

    // Initial state
    expect(screen.getByTestId('timer-display')).toHaveTextContent('25:00');

    // Update remaining seconds
    useTimerStore.setState({ remainingSeconds: 1499 }); // 24:59
    rerender(<TimerDisplay />);
    expect(screen.getByTestId('timer-display')).toHaveTextContent('24:59');

    // Another update
    useTimerStore.setState({ remainingSeconds: 60 }); // 01:00
    rerender(<TimerDisplay />);
    expect(screen.getByTestId('timer-display')).toHaveTextContent('01:00');
  });

  it('should display 00:00 when time reaches zero', () => {
    useTimerStore.setState({ remainingSeconds: 0 });
    render(<TimerDisplay />);
    expect(screen.getByTestId('timer-display')).toHaveTextContent('00:00');
  });

  it('should have aria-live attribute for accessibility', () => {
    render(<TimerDisplay />);
    const timerElement = screen.getByTestId('timer-display');
    expect(timerElement).toHaveAttribute('aria-live', 'polite');
  });

  it('should have aria-atomic attribute for accessibility', () => {
    render(<TimerDisplay />);
    const timerElement = screen.getByTestId('timer-display');
    expect(timerElement).toHaveAttribute('aria-atomic', 'true');
  });
});
