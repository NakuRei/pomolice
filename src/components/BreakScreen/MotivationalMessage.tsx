import { useState } from "react";
import { getRandomMessage } from "../../data/motivationalMessages";

/**
 * MotivationalMessage Component
 *
 * Displays a random motivational message during break time
 * to encourage users to take proper breaks.
 */
export function MotivationalMessage(): React.ReactElement {
  const [message] = useState<string>(() => getRandomMessage());

  return (
    <div className="mb-8 text-center">
      <p className="text-2xl font-medium text-text">{message}</p>
    </div>
  );
}
