import { useEffect, useRef, type RefObject } from "react";

/**
 * Settings page headings use this hook to receive focus immediately after mount.
 *
 * Usage:
 * const headingRef = useFocusHeadingOnMount();
 * <SettingsPageHeader headingRef={headingRef} ... />
 */
export function useFocusHeadingOnMount(): RefObject<HTMLHeadingElement | null> {
  const headingRef = useRef<HTMLHeadingElement | null>(null);

  useEffect(() => {
    headingRef.current?.focus();
  }, []);

  return headingRef;
}
