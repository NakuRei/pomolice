import { describe, it, expect } from 'vitest';
import { formatTime } from '../timeFormatter';

describe('formatTime', () => {
  it('should format zero seconds as 00:00', () => {
    expect(formatTime(0)).toBe('00:00');
  });

  it('should format seconds less than a minute correctly', () => {
    expect(formatTime(1)).toBe('00:01');
    expect(formatTime(30)).toBe('00:30');
    expect(formatTime(59)).toBe('00:59');
  });

  it('should format exactly one minute as 01:00', () => {
    expect(formatTime(60)).toBe('01:00');
  });

  it('should format minutes and seconds correctly', () => {
    expect(formatTime(65)).toBe('01:05');
    expect(formatTime(90)).toBe('01:30');
    expect(formatTime(125)).toBe('02:05');
  });

  it('should format 25 minutes (default work duration) correctly', () => {
    expect(formatTime(1500)).toBe('25:00');
  });

  it('should format times over 60 minutes correctly', () => {
    expect(formatTime(3661)).toBe('61:01');
    expect(formatTime(7200)).toBe('120:00');
  });

  it('should handle negative numbers by treating them as zero', () => {
    expect(formatTime(-1)).toBe('00:00');
    expect(formatTime(-100)).toBe('00:00');
  });

  it('should handle decimal numbers by rounding down', () => {
    expect(formatTime(65.7)).toBe('01:05');
    expect(formatTime(59.9)).toBe('00:59');
  });

  it('should pad single digit minutes and seconds with zeros', () => {
    expect(formatTime(5)).toBe('00:05');
    expect(formatTime(305)).toBe('05:05');
  });
});
