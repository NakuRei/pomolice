import { useEffect, useState } from "react";
import { getRandomMessage } from "../../data/motivationalMessages";

/**
 * MotivationalMessage Component
 *
 * Displays a random motivational message during break time
 * to encourage users to take proper breaks.
 */
export function MotivationalMessage() {
  const [message, setMessage] = useState<string>("");

  useEffect(() => {
    // Set a random message when component mounts
    setMessage(getRandomMessage());
  }, []);

  return (
    <div className="text-center mb-8">
      <p className="text-2xl font-medium text-gray-100">
        {message}
      </p>
    </div>
  );
}
