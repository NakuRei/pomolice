export function formatDurationInputText(value: number | null): string {
  return value !== null && Number.isFinite(value) ? String(value) : "";
}

export function parseDurationInputText(
  raw: string,
  min: number,
  max: number,
  allowDecimal: boolean,
): number | null {
  if (raw.trim() === "") return null;
  const parsedValue = Number(raw);
  if (!Number.isFinite(parsedValue)) return null;
  if (!allowDecimal && !Number.isInteger(parsedValue)) return null;
  if (parsedValue < min || parsedValue > max) return null;
  return parsedValue;
}
