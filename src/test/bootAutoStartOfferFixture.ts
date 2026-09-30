import type { BootAutoStartOfferPayload } from "@/ipc/generated";

export function createBootAutoStartOfferForTest(): BootAutoStartOfferPayload {
  return {
    anchor_local_date: "2025-02-03",
    window: {
      weekday_key: "mon",
      start_time: "09:00",
      end_time: "18:00",
      crosses_midnight: false,
    },
  };
}
