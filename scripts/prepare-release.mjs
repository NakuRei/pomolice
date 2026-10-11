import { createHash } from "node:crypto";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

try {
  const [tag, artifacts, output, ...extra] = process.argv.slice(2);
  if (!tag || !artifacts || !output || extra.length > 0) {
    throw new Error(
      "Usage: node scripts/prepare-release.mjs <tag> <artifacts-directory> <output-directory>",
    );
  }
  if (!/^v(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)$/.test(tag)) {
    throw new Error("Release tag must be vMAJOR.MINOR.PATCH.");
  }
  const version = tag.slice(1);
  const windows = `Pomolice_${version}_x64-setup.exe`;
  const ubuntu = `Pomolice_${version}_amd64.deb`;
  const installers = [];
  for (const [platform, filename] of [
    ["windows", windows],
    ["ubuntu", ubuntu],
  ]) {
    const directory = join(artifacts, platform);
    const manifest = (
      await readFile(join(directory, "SHA256SUMS"), "utf8")
    ).trim();
    const entry = /^([a-f0-9]{64})  (\S+)$/.exec(manifest);
    if (!entry || entry[2] !== filename) {
      throw new Error(`Expected one checksum entry for ${filename}.`);
    }
    const bytes = await readFile(join(directory, filename));
    const hash = createHash("sha256").update(bytes).digest("hex");
    if (hash !== entry[1]) {
      throw new Error(`Checksum mismatch: ${filename}.`);
    }
    installers.push({ filename, bytes, hash });
  }

  // A fresh directory prevents stale assets from entering a release on reruns.
  await mkdir(output);
  for (const { filename, bytes } of installers) {
    await writeFile(join(output, filename), bytes);
  }
  await writeFile(
    join(output, "SHA256SUMS"),
    installers.map(({ filename, hash }) => `${hash}  ${filename}\n`).join(""),
  );
  await writeFile(
    join(output, "notes.md"),
    `## 対応環境

- Windows 11 x64: \`${windows}\`
- Ubuntu 26.04 LTS amd64: \`${ubuntu}\`

## インストール・手動更新

- Windows: 添付のインストーラーを実行してください。
- Ubuntu: ダウンロード先で \`sudo apt install ./${ubuntu}\` を実行してください。
- 更新時はタイマーを停止し、トレイから Pomolice を終了してから新版をインストールしてください。
- ダウンロードしたファイルの確認には、添付の \`SHA256SUMS\` を使用できます。

## 既知の制限

- アプリ内更新はありません。新版を手動でダウンロードして更新してください。
- macOS・ARM64 は対応対象外です。

## 公開前の確認

- [ ] Windows 11 と Ubuntu 26.04 で新規導入・起動・アンインストールを確認
- [ ] 手動更新後の設定・保存状態の保持を確認
- [ ] 休憩画面・入力検知・緊急解除・トレイの動作を確認
- [ ] Windows コード署名の扱いと既知の制限を確認
- [ ] 変更内容を確認し、この公開前チェックリストを削除

`,
  );
  console.info(`Prepared release assets for ${tag} in ${output}.`);
} catch (error) {
  console.error(error instanceof Error ? error.message : String(error));
  process.exitCode = 1;
}
