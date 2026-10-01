import { create } from "zustand";
import type {
  OperationRecord,
  OperationOutcome,
  TimerOperation,
  TimerOperationKind,
} from "@/timer/operation";
interface TimerOperations {
  operations: Partial<Record<TimerOperationKind, OperationRecord>>;
  nextId: number;
  begin: (request: TimerOperation) => number;
  complete: (
    kind: TimerOperationKind,
    id: number,
    outcome: OperationOutcome,
  ) => void;
  dismiss: (kind: TimerOperationKind, id: number) => void;
}
export const useTimerOperationStore = create<TimerOperations>((set, get) => ({
  operations: {},
  nextId: 1,
  begin: (request): number => {
    const id = get().nextId;
    set((state) => ({
      nextId: id + 1,
      operations: {
        ...state.operations,
        [request.kind]: {
          id,
          request,
          outcome: { status: "pending" },
          dismissed: false,
        },
      },
    }));
    return id;
  },
  complete: (kind, id, outcome): void => {
    const current = get().operations[kind];
    if (current?.id !== id) return;
    set((state) => ({
      operations: { ...state.operations, [kind]: { ...current, outcome } },
    }));
  },
  dismiss: (kind, id): void => {
    const current = get().operations[kind];
    if (current?.id !== id) return;
    set((state) => ({
      operations: {
        ...state.operations,
        [kind]: { ...current, dismissed: true },
      },
    }));
  },
}));
