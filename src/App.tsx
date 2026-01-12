import { TimerDisplay, TimerControls } from './components/Timer';
import { EmergencyStopButton } from './components/EmergencyStop';
import { useTimer } from './hooks/useTimer';

function App() {
  // Initialize timer interval management
  useTimer();

  return (
    <div className="flex flex-col items-center justify-center min-h-screen bg-gray-100">
      <div className="bg-white rounded-lg shadow-lg p-8 space-y-8">
        <div className="flex justify-between items-center">
          <h1 className="text-3xl font-bold text-gray-800">Pomolice</h1>
          <EmergencyStopButton />
        </div>

        <TimerDisplay />

        <TimerControls />
      </div>
    </div>
  );
}

export default App;
