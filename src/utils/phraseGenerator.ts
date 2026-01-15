/**
 * Phrase generator utility for skip flow typing challenges
 * Phase 5: Multi-step skip confirmation
 *
 * Randomly selects phrases from predefined phrase banks
 */

import {
  SHORT_TYPING_PHRASES,
  LONG_TYPING_PHRASES,
  CONFIRMATION_MESSAGES,
  type ShortTypingPhrase,
  type LongTypingPhrase,
  type ConfirmationMessage,
} from "../data/typingPhrases";

/**
 * Generate a random integer between 0 and max (exclusive)
 * @param max - Maximum value (exclusive)
 * @returns Random integer
 */
function randomInt(max: number): number {
  return Math.floor(Math.random() * max);
}

/**
 * Get a random short typing phrase for step 5
 * @returns Random short typing phrase
 */
export function getRandomShortPhrase(): ShortTypingPhrase {
  const index = randomInt(SHORT_TYPING_PHRASES.length);
  return SHORT_TYPING_PHRASES[index];
}

/**
 * Get a random long typing phrase for step 7
 * @returns Random long typing phrase
 */
export function getRandomLongPhrase(): LongTypingPhrase {
  const index = randomInt(LONG_TYPING_PHRASES.length);
  return LONG_TYPING_PHRASES[index];
}

/**
 * Get confirmation message for a specific step (1-4)
 * @param step - Step number (1-4)
 * @returns Confirmation message for the step
 */
export function getConfirmationMessage(step: number): ConfirmationMessage {
  // Steps 1-4 correspond to confirmation messages at indices 0-3
  const index = Math.max(0, Math.min(step - 1, CONFIRMATION_MESSAGES.length - 1));
  return CONFIRMATION_MESSAGES[index];
}

/**
 * Validate typed input against target phrase
 * Case-sensitive comparison (phrases are in UPPERCASE)
 *
 * @param input - User's typed input
 * @param target - Target phrase to match
 * @returns True if exact match, false otherwise
 */
export function validateTypedPhrase(input: string, target: string): boolean {
  return input === target;
}

/**
 * Get character-by-character validation result for real-time feedback
 * Returns an array indicating whether each character is correct
 *
 * @param input - User's current input
 * @param target - Target phrase
 * @returns Array of booleans (true = correct, false = incorrect, null = not typed yet)
 */
export function getCharacterValidation(
  input: string,
  target: string
): (boolean | null)[] {
  return Array.from(target).map((char, index) => {
    if (index >= input.length) {
      return null; // Not typed yet
    }
    return input[index] === char; // Compare character
  });
}
