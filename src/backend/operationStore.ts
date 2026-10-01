import { create } from "zustand";
import type {
  OperationRecord,
  OperationOutcome,
  Operation,
  OperationKind,
} from "./operation";
interface Operations {
  operations: Partial<Record<OperationKind, OperationRecord>>;
  nextId: number;
  begin: (request: Operation) => number;
  complete: (
    kind: OperationKind,
    id: number,
    outcome: OperationOutcome,
  ) => void;
  dismiss: (kind: OperationKind, id: number) => void;
}
export const useOperationStore = create<Operations>((set, get) => ({
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
