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
  "I CHOOSE TO END THIS BREAK EARLY",
  "I UNDERSTAND THAT TAKING A BREAK IS RECOMMENDED",
  "I AM MAKING THIS CHOICE INTENTIONALLY",
  "I WILL PAY ATTENTION TO FATIGUE AFTER THIS",
  "I UNDERSTAND THIS DOES NOT COMPLETE THE BREAK",
] as const;

/**
 * Long phrases for typing challenge step 7
 * Length: ~60-80 characters
 * Purpose: More difficult typing challenge with longer message
 */
export const LONG_TYPING_PHRASES = [
  "I UNDERSTAND THAT CONTINUING THIS BREAK IS THE RECOMMENDED OPTION",
  "I CHOOSE TO END THIS BREAK EARLY AND RETURN TO WORK NOW",
  "I WILL PAY ATTENTION TO FATIGUE AND TAKE REST SERIOUSLY LATER",
  "I UNDERSTAND THAT THIS ACTION ENDS THE BREAK BEFORE THE TIMER FINISHES",
  "I AM CHOOSING TO CONTINUE WITH CARE AFTER ENDING THIS BREAK EARLY",
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
export type ShortTypingPhrase = (typeof SHORT_TYPING_PHRASES)[number];
export type LongTypingPhrase = (typeof LONG_TYPING_PHRASES)[number];
export type ConfirmationMessage = (typeof CONFIRMATION_MESSAGES)[number];
