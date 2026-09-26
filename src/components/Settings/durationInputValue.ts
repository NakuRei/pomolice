export function formatDurationInputText(value: number | null): string {
  return value !== null && Number.isFinite(value) ? String(value) : "";
}

/**
 * Reads the number typed into a settings input. Only the number's form is
 * checked here; whether it is in range is decided by the backend on save.
 */
export function parseDurationInputText(
  raw: string,
  allowDecimal: boolean,
): number | null {
  if (raw.trim() === "") return null;
  const parsedValue = Number(raw);
  if (!Number.isFinite(parsedValue)) return null;
  if (!allowDecimal && !Number.isInteger(parsedValue)) return null;
  return parsedValue;
}
