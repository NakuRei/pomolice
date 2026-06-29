import { useState } from "react";
import { getRandomMessage } from "@/data/motivationalMessages";
import { MotivationalMessage } from "./MotivationalMessage";

interface BreakMotivationalMessageProps {
  selectMessage?: () => string;
}

export function BreakMotivationalMessage({
  selectMessage = getRandomMessage,
}: BreakMotivationalMessageProps): React.ReactElement {
  const [message] = useState<string>(() => selectMessage());

  return <MotivationalMessage message={message} />;
}
