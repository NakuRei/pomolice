/**
 * Builds a component that throws an Error with the given message during render,
 * so an ancestor error boundary catches it. Any props passed are ignored.
 */
export function throwOnRender(
  message: string,
): React.ComponentType<Record<string, unknown>> {
  return function ThrowOnRender(): React.ReactElement {
    throw new Error(message);
  };
}
