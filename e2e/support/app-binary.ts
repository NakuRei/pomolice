import path from "node:path";

export const appBinaryPath = path.resolve(
  import.meta.dirname,
  "..",
  "..",
  "target",
  "e2e",
  "debug",
  "pomolice.exe",
);
