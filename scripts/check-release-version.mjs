import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";

try {
  const [tag, ...extra] = process.argv.slice(2);
  if (extra.length > 0) {
    throw new Error(
      "Usage: node scripts/check-release-version.mjs [vMAJOR.MINOR.PATCH]",
    );
  }
  const packageJson = JSON.parse(await readFile("package.json", "utf8"));
  const lock = JSON.parse(await readFile("package-lock.json", "utf8"));
  const tauri = JSON.parse(await readFile("src-tauri/tauri.conf.json", "utf8"));
  const metadata = JSON.parse(
    execFileSync(
      "cargo",
      [
        "metadata",
        "--format-version",
        "1",
        "--no-deps",
        "--offline",
        "--manifest-path",
        "src-tauri/Cargo.toml",
      ],
      { encoding: "utf8", windowsHide: true },
    ),
  );
  const version = packageJson.version;
  const stableVersion = /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/;
  if (typeof version !== "string" || !stableVersion.test(version)) {
    throw new Error(
      "package.json must contain a stable version (MAJOR.MINOR.PATCH).",
    );
  }
  const versions = {
    "package-lock.json": lock.version,
    "package-lock.json packages[root]": lock.packages?.[""]?.version,
    "src-tauri/tauri.conf.json": tauri.version,
    "src-tauri/Cargo.toml": metadata.packages.find(
      (entry) => entry.name === "pomolice",
    )?.version,
  };
  for (const [source, actual] of Object.entries(versions)) {
    if (actual !== version) {
      throw new Error(
        `Version mismatch: ${source} has ${String(actual)}; expected ${version}.`,
      );
    }
  }
  if (tag !== undefined && tag !== `v${version}`) {
    throw new Error(
      `Release tag mismatch: expected v${version}, got ${JSON.stringify(tag)}.`,
    );
  }
  console.info(version);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
