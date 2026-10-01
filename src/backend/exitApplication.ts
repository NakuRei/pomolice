import { commands } from "@/ipc/generated";
import { receiveCommandResponse } from "@/ipc/receiveCommandResponse";
import { useTrayStore } from "./trayStore";

export async function exitApplication(): Promise<void> {
  if (useTrayStore.getState().exitStatus === "exiting") return;
  useTrayStore.setState({ exitStatus: "exiting" });
  const response = await receiveCommandResponse(commands.exitWithoutTray());
  if (response.status !== "accepted")
    useTrayStore.setState({ exitStatus: "failed" });
}
