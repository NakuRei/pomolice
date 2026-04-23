/**
 * Returns IDs from an aria-describedby attribute, or an empty list if absent.
 */
export function getAriaDescribedByIds(element: HTMLElement): string[] {
  return (
    element
      .getAttribute("aria-describedby")
      ?.split(/\s+/u)
      .filter((id) => id.length > 0) ?? []
  );
}
