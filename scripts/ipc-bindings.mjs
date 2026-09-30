import { execFileSync } from "node:child_process";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { format, resolveConfig } from "prettier";
import { ESLint } from "eslint";

const root = fileURLToPath(new URL("../", import.meta.url));
const destination = join(root, "src/ipc/generated.ts");
const temporary = await mkdtemp(join(tmpdir(), "pomolice-ipc-"));
try {
  const output = join(temporary, "generated.ts");
  execFileSync(
    "cargo",
    [
      "test",
      "--manifest-path",
      "src-tauri/Cargo.toml",
      "--lib",
      "-j",
      "1",
      "ipc::bindings::tests::export_contract",
      "--",
      "--exact",
      "--test-threads=1",
    ],
    {
      cwd: root,
      env: { ...process.env, POMOLICE_IPC_EXPORT: output },
      stdio: "inherit",
      windowsHide: true,
    },
  );
  const raw = await readFile(output, "utf8");
  const [linted] = await new ESLint({ cwd: root, fix: true }).lintText(raw, {
    filePath: destination,
  });
  if (!linted || linted.errorCount > 0) {
    throw new Error(
      "Generated IPC bindings failed lint: " + JSON.stringify(linted?.messages),
    );
  }
  const generated = await format(linted.output ?? raw, {
    ...(await resolveConfig(destination)),
    filepath: destination,
  });
  if (process.argv.includes("--check")) {
    if ((await readFile(destination, "utf8")) !== generated) {
      throw new Error("IPC bindings are stale. Run npm run ipc:generate.");
    }
  } else {
    await writeFile(destination, generated);
  }
} finally {
  await rm(temporary, { recursive: true, force: true });
}
