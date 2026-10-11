import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const script = fileURLToPath(
  new URL("../check-release-version.mjs", import.meta.url),
);

async function project(t, versions = {}) {
  const directory = await mkdtemp(join(tmpdir(), "pomolice-release-test-"));
  t.after(() => rm(directory, { recursive: true, force: true }));
  await mkdir(join(directory, "src-tauri", "src"), { recursive: true });
  await writeFile(
    join(directory, "package.json"),
    JSON.stringify({
      name: "pomolice",
      version: versions.package ?? "0.1.0",
    }),
  );
  await writeFile(
    join(directory, "package-lock.json"),
    JSON.stringify({
      version: versions.lock ?? "0.1.0",
      packages: { "": { version: versions.lockRoot ?? "0.1.0" } },
    }),
  );
  await writeFile(
    join(directory, "src-tauri", "tauri.conf.json"),
    JSON.stringify({
      version: versions.tauri ?? "0.1.0",
    }),
  );
  await writeFile(
    join(directory, "src-tauri", "Cargo.toml"),
    `[package]\nname = "pomolice"\nversion = "${versions.cargo ?? "0.1.0"}"\nedition = "2021"\n`,
  );
  await writeFile(
    join(directory, "src-tauri", "src", "main.rs"),
    "fn main() {}\n",
  );
  return directory;
}

function check(directory, ...args) {
  return spawnSync(process.execPath, [script, ...args], {
    cwd: directory,
    encoding: "utf8",
    windowsHide: true,
  });
}

test("accepts matching application versions without a release tag", async (t) => {
  const result = check(await project(t));
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "0.1.0");
});

test("accepts a matching stable release tag", async (t) => {
  const result = check(await project(t), "v0.1.0");
  assert.equal(result.status, 0, result.stderr);
  assert.equal(result.stdout.trim(), "0.1.0");
});

for (const source of ["package", "lock", "lockRoot", "tauri", "cargo"]) {
  test(`rejects a mismatched ${source} version`, async (t) => {
    const result = check(await project(t, { [source]: "0.2.0" }));
    assert.equal(result.status, 1);
    assert.match(result.stderr, /version mismatch/i);
  });
}

for (const tag of ["v0.2.0", "0.1.0", "v0.1.0-beta.1", "v00.1.0", ""]) {
  test(`rejects invalid or inconsistent release tag ${JSON.stringify(tag)}`, async (t) => {
    const result = check(await project(t), tag);
    assert.equal(result.status, 1);
    assert.match(result.stderr, /tag/i);
  });
}

test("rejects an unsupported prerelease even when every version agrees", async (t) => {
  const versions = Object.fromEntries(
    ["package", "lock", "lockRoot", "tauri", "cargo"].map((key) => [
      key,
      "0.1.0-beta.1",
    ]),
  );
  const result = check(await project(t, versions));
  assert.equal(result.status, 1);
  assert.match(result.stderr, /stable version/i);
});

test("rejects missing version metadata", async (t) => {
  const directory = await project(t);
  await writeFile(join(directory, "src-tauri", "tauri.conf.json"), "{}");
  const result = check(directory);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /version mismatch/i);
});

test("rejects extra arguments instead of silently ignoring them", async (t) => {
  const result = check(await project(t), "v0.1.0", "v0.2.0");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /usage/i);
});
