import { TimerDisplay, TimerControls } from './components/Timer';
import { useTimer } from './hooks/useTimer';

function App() {
  // Initialize timer interval management
  useTimer();

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-100">
      <div className="bg-white rounded-lg shadow-lg p-8 space-y-8">
        <h1 className="text-3xl font-bold text-center text-gray-800">
          Pomolice
        </h1>

        <TimerDisplay />

        <TimerControls />
      </div>
    </div>
  );
}

export default App;
