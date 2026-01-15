/**
 * Typing challenge phrases for skip flow
 * Phase 5: Multi-step skip confirmation
 *
 * Two difficulty levels:
 * - Short phrases: Used in step 5 (first typing challenge)
 * - Long phrases: Used in step 7 (second typing challenge)
 *
 * All phrases are in UPPERCASE to increase difficulty and require conscious typing.
 * These phrases are designed to make users reflect on their decision to skip breaks.
 */

/**
 * Short phrases for typing challenge step 5
 * Length: ~30-40 characters
 * Purpose: Initial typing challenge, relatively short but requires attention
 */
export const SHORT_TYPING_PHRASES = [
  "I CONSCIOUSLY CHOOSE TO SKIP THIS BREAK",
  "I AM AWARE OF THE CONSEQUENCES",
  "I PRIORITIZE WORK OVER MY HEALTH",
  "I WILL SKIP THIS IMPORTANT REST TIME",
  "I ACKNOWLEDGE THIS IS NOT RECOMMENDED",
] as const;

/**
 * Long phrases for typing challenge step 7
 * Length: ~60-80 characters
 * Purpose: More difficult typing challenge with longer message
 */
export const LONG_TYPING_PHRASES = [
  "I UNDERSTAND THAT SKIPPING BREAKS MAY HARM MY HEALTH AND PRODUCTIVITY",
  "I CONSCIOUSLY DECIDE TO IGNORE THE RECOMMENDATION TO TAKE A BREAK NOW",
  "I ACCEPT FULL RESPONSIBILITY FOR THE CONSEQUENCES OF SKIPPING THIS REST",
  "I ACKNOWLEDGE THAT CONTINUOUS WORK WITHOUT BREAKS REDUCES MY EFFICIENCY",
  "I REALIZE THAT TAKING BREAKS IS ESSENTIAL FOR SUSTAINABLE PRODUCTIVITY",
] as const;

/**
 * Confirmation messages for the confirmation steps (1-4)
 * These are displayed in Yes/No dialogs before the typing challenges
 */
export const CONFIRMATION_MESSAGES = [
  "Are you sure you want to skip this break?",
  "Breaks are important for your health. Do you still want to skip?",
  "Your body needs rest. Are you certain you want to skip?",
  "This is your last warning. Really skip this break?",
] as const;

/**
 * Type definitions for type safety
 */
export type ShortTypingPhrase = typeof SHORT_TYPING_PHRASES[number];
export type LongTypingPhrase = typeof LONG_TYPING_PHRASES[number];
export type ConfirmationMessage = typeof CONFIRMATION_MESSAGES[number];
