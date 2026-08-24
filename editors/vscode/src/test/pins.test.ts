import { test } from "node:test";
import * as assert from "node:assert/strict";
import { parseSums, platformArchive, pinnedSha256, SHA256SUMS, RELEASE_BASE_URL } from "../pins";

test("parseSums extracts all four pinned entries", () => {
  const sums = parseSums(SHA256SUMS);
  assert.equal(sums.size, 4);
  assert.equal(
    sums.get("aurora-agent-0.1.3-x86_64-pc-windows-msvc.zip"),
    "c8ccf580f2ebda241a10db42c87abeee170d403ffcbf35a0f3f6eb26233a8fa6"
  );
  assert.equal(
    sums.get("aurora-agent-0.1.3-aarch64-apple-darwin.tar.gz"),
    "bd6eaab534bf9a9cc33c16428f64e30cdec5f0f6382a5abc115641f7265601fb"
  );
  assert.equal(
    sums.get("aurora-agent-0.1.3-x86_64-apple-darwin.tar.gz"),
    "5653a3baacf1c08df89de7375909c9614940310e4116cd44eac3765f94586c64"
  );
  assert.equal(
    sums.get("aurora-agent-0.1.3-x86_64-unknown-linux-gnu.tar.gz"),
    "01ce74afc7f01184c477fa1d4861e0cde71646b318e57774eed1708b033ef205"
  );
});

test("parseSums ignores malformed lines", () => {
  const sums = parseSums("nonsense\nzz  file\n" + "a".repeat(64) + "  good.tar.gz\n");
  assert.equal(sums.size, 1);
  assert.equal(sums.get("good.tar.gz"), "a".repeat(64));
});

test("platformArchive maps the four supported platforms", () => {
  assert.equal(platformArchive("win32", "x64"), "aurora-agent-0.1.3-x86_64-pc-windows-msvc.zip");
  assert.equal(platformArchive("darwin", "arm64"), "aurora-agent-0.1.3-aarch64-apple-darwin.tar.gz");
  assert.equal(platformArchive("darwin", "x64"), "aurora-agent-0.1.3-x86_64-apple-darwin.tar.gz");
  assert.equal(platformArchive("linux", "x64"), "aurora-agent-0.1.3-x86_64-unknown-linux-gnu.tar.gz");
});

test("platformArchive refuses unsupported platforms", () => {
  assert.equal(platformArchive("linux", "arm64"), undefined);
  assert.equal(platformArchive("win32", "arm64"), undefined);
  assert.equal(platformArchive("freebsd", "x64"), undefined);
});

test("every mapped archive has a pinned hash and a release URL", () => {
  for (const [platform, arch] of [
    ["win32", "x64"],
    ["darwin", "arm64"],
    ["darwin", "x64"],
    ["linux", "x64"],
  ] as const) {
    const archive = platformArchive(platform, arch);
    assert.ok(archive !== undefined);
    const sha = pinnedSha256(archive);
    assert.ok(sha !== undefined && /^[0-9a-f]{64}$/.test(sha));
  }
  assert.ok(RELEASE_BASE_URL.startsWith("https://github.com/AURORA-NEURO/aurora-agent/releases/download/v0.1.3/"));
});
