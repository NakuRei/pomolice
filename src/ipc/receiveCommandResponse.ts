import type { CommandResponse } from "./generated";

export type CommandFailure =
  | Extract<CommandResponse<never>, { status: "rejected" }>
  | { status: "unavailable"; cause: unknown };

export async function receiveCommandResponse<T>(
  request: Promise<CommandResponse<T>>,
): Promise<
  CommandResponse<T> | Extract<CommandFailure, { status: "unavailable" }>
> {
  try {
    return await request;
  } catch (cause) {
    return { status: "unavailable", cause };
  }
}
