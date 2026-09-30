import type { CommandResponse } from "@/ipc/generated";

export function acceptedCommand<T>(value: T): CommandResponse<T> {
  return { status: "accepted", value };
}
