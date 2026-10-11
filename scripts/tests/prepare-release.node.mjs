import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { test } from "node:test";

const script = fileURLToPath(
  new URL("../prepare-release.mjs", import.meta.url),
);
const windows = "Pomolice_0.1.0_x64-setup.exe";
const ubuntu = "Pomolice_0.1.0_amd64.deb";

async function artifacts({ omitUbuntu = false } = {}) {
  const root = await mkdtemp(join(tmpdir(), "pomolice-release-assets-"));
  for (const [platform, filename] of [
    ["windows", windows],
    ["ubuntu", ubuntu],
  ]) {
    if (omitUbuntu && platform === "ubuntu") continue;
    const directory = join(root, platform);
    await mkdir(directory);
    const content = Buffer.from(`installer: ${platform}`);
    await writeFile(join(directory, filename), content);
    const hash = createHash("sha256").update(content).digest("hex");
    await writeFile(
      join(directory, "SHA256SUMS"),
      `${platform === "windows" ? "\uFEFF" : ""}${hash}  ${filename}\r\n`,
    );
  }
  return root;
}

function prepare(root, tag = "v0.1.0", ...extra) {
  return spawnSync(
    process.execPath,
    [script, tag, root, join(root, "release"), ...extra],
    {
      encoding: "utf8",
      windowsHide: true,
    },
  );
}

test("prepares both installers unchanged and combines verified checksums", async () => {
  const root = await artifacts();
  await writeFile(
    join(root, "ubuntu", "shared-libraries.txt"),
    "diagnostic data",
  );
  const result = prepare(root);
  assert.equal(result.status, 0, result.stderr);
  const output = join(root, "release");
  assert.deepEqual(
    (await readdir(output)).sort(),
    [windows, ubuntu, "SHA256SUMS", "notes.md"].sort(),
  );
  for (const [platform, filename] of [
    ["windows", windows],
    ["ubuntu", ubuntu],
  ]) {
    assert.deepEqual(
      await readFile(join(output, filename)),
      await readFile(join(root, platform, filename)),
    );
  }
  const sums = await readFile(join(output, "SHA256SUMS"), "utf8");
  assert.equal(
    sums,
    (
      await Promise.all(
        ["windows", "ubuntu"].map(async (platform) =>
          (await readFile(join(root, platform, "SHA256SUMS"), "utf8")).trim(),
        ),
      )
    ).join("\n") + "\n",
  );
  const notes = await readFile(join(output, "notes.md"), "utf8");
  assert.match(notes, /Windows 11/);
  assert.match(notes, /Ubuntu 26\.04/);
  assert.match(notes, /手動/);
  assert.ok(notes.includes(windows) && notes.includes(ubuntu));
});

test("rejects a missing platform before creating release output", async () => {
  const root = await artifacts({ omitUbuntu: true });
  assert.equal(prepare(root).status, 1);
  await assert.rejects(readdir(join(root, "release")), { code: "ENOENT" });
});

test("rejects altered installer bytes", async () => {
  const root = await artifacts();
  await writeFile(join(root, "windows", windows), "altered");
  const result = prepare(root);
  assert.equal(result.status, 1);
  assert.match(result.stderr, /checksum/i);
  await assert.rejects(readdir(join(root, "release")), { code: "ENOENT" });
});

test("rejects checksums naming a different version or path", async () => {
  for (const filename of [
    "Pomolice_0.2.0_amd64.deb",
    "../Pomolice_0.1.0_amd64.deb",
  ]) {
    const root = await artifacts();
    await writeFile(
      join(root, "ubuntu", "SHA256SUMS"),
      `${"a".repeat(64)}  ${filename}\n`,
    );
    assert.equal(prepare(root).status, 1);
  }
});

test("rejects malformed and extra checksum entries", async () => {
  for (const manifest of [
    "",
    `bad-hash  ${ubuntu}\n`,
    `${"a".repeat(64)}  ${ubuntu}\n${"b".repeat(64)}  extra.deb\n`,
  ]) {
    const root = await artifacts();
    await writeFile(join(root, "ubuntu", "SHA256SUMS"), manifest);
    assert.equal(prepare(root).status, 1);
  }
});

test("rejects invalid tags and artifacts for a different version", async () => {
  const root = await artifacts();
  for (const tag of [
    "v0.2.0",
    "0.1.0",
    "v0.1.0-beta.1",
    "v00.1.0",
    "../0.1.0",
  ]) {
    assert.equal(prepare(root, tag).status, 1, tag);
  }
});

test("refuses to overwrite existing output", async () => {
  const root = await artifacts();
  await mkdir(join(root, "release"));
  await writeFile(join(root, "release", "keep"), "existing content");
  assert.equal(prepare(root).status, 1);
  assert.deepEqual(await readdir(join(root, "release")), ["keep"]);
});

test("rejects extra command arguments", async () => {
  const result = prepare(await artifacts(), "v0.1.0", "unexpected");
  assert.equal(result.status, 1);
  assert.match(result.stderr, /usage/i);
});
