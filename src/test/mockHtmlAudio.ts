import { expect, vi } from "vitest";

const mockHtmlAudioInstances: MockHtmlAudio[] = [];
let queuedPlayError: Error | null = null;

export class MockHtmlAudio {
  public src: string;
  public volume = 1;
  public currentTime = 0;
  public pause = vi.fn();
  public play = vi.fn((): Promise<void> => {
    if (queuedPlayError !== null) {
      const playError = queuedPlayError;
      queuedPlayError = null;
      return Promise.reject(playError);
    }

    return Promise.resolve();
  });

  public constructor(src: string) {
    this.src = src;
    mockHtmlAudioInstances.push(this);
  }
}

export function installMockHtmlAudio(): void {
  globalThis.Audio = MockHtmlAudio as unknown as typeof Audio;
}

export function resetMockHtmlAudio(): void {
  mockHtmlAudioInstances.length = 0;
  queuedPlayError = null;
}

export function queueMockHtmlAudioPlayError(error: Error): void {
  queuedPlayError = error;
}

export function getMockHtmlAudioInstance(index: number): MockHtmlAudio {
  const audioInstance = mockHtmlAudioInstances[index];
  expect(audioInstance).toBeDefined();
  if (audioInstance === undefined) {
    throw new Error(`Expected audio instance at index ${String(index)}`);
  }

  return audioInstance;
}

export function getMockHtmlAudioInstanceCount(): number {
  return mockHtmlAudioInstances.length;
}
