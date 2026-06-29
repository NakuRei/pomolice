interface MotivationalMessageProps {
  message: string;
}

export function MotivationalMessage({
  message,
}: MotivationalMessageProps): React.ReactElement {
  return (
    <div className="mb-8 text-center">
      <p className="text-2xl font-medium text-text">{message}</p>
    </div>
  );
}
