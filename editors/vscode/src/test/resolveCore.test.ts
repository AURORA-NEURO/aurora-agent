import { test } from "node:test";
import * as assert from "node:assert/strict";
import * as fs from "node:fs";
import * as os from "node:os";
import * as path from "node:path";
import { resolveBinaries, exeName, findBinariesUnder, ResolveInputs } from "../resolveCore";

const PLATFORM = "win32";
const EXE = exeName("bioprism", PLATFORM);

function makeFakeExe(dir: string): void {
  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(path.join(dir, EXE), "fake");
  fs.writeFileSync(path.join(dir, exeName("bioprism-mcp", PLATFORM)), "fake");
  fs.writeFileSync(path.join(dir, exeName("bioprism-api", PLATFORM)), "fake");
}

interface Fixture {
  base: string;
  binaryDir: string;
  envRoot: string;
  workspace: string;
  home: string;
  cacheVersionDir: string;
}

function buildFixture(): Fixture {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "aurora-resolve-"));
  const fixture: Fixture = {
    base,
    binaryDir: path.join(base, "custom-bin"),
    envRoot: path.join(base, "envroot"),
    workspace: path.join(base, "workspace"),
    home: path.join(base, "home"),
    cacheVersionDir: path.join(base, "cache", "aurora-agent", "v0.1.3"),
  };
  makeFakeExe(fixture.binaryDir);
  makeFakeExe(path.join(fixture.envRoot, "target", "release"));
  makeFakeExe(path.join(fixture.workspace, "target", "release"));
  makeFakeExe(path.join(fixture.home, "aurora-agent", "target", "release"));
  makeFakeExe(path.join(fixture.home, "bioprism", "target", "release"));
  makeFakeExe(path.join(fixture.cacheVersionDir, "nested"));
  return fixture;
}

function inputsFor(fixture: Fixture, overrides: Partial<ResolveInputs> = {}): ResolveInputs {
  return {
    platform: PLATFORM,
    settingsBinaryDir: fixture.binaryDir,
    settingsRoot: undefined,
    envRoot: fixture.envRoot,
    workspaceFolders: [fixture.workspace],
    homeDir: fixture.home,
    cacheVersionDir: fixture.cacheVersionDir,
    existsSync: (p) => fs.existsSync(p),
    readdirSync: (p) => fs.readdirSync(p),
    isDirectory: (p) => {
      try {
        return fs.statSync(p).isDirectory();
      } catch {
        return false;
      }
    },
    ...overrides,
  };
}

test("resolution ordering: settings.binaryDir > env root > workspace > home > cache", (t) => {
  const fixture = buildFixture();
  t.after(() => fs.rmSync(fixture.base, { recursive: true, force: true }));

  const first = resolveBinaries(inputsFor(fixture));
  assert.ok(first);
  assert.equal(first.binarySource, "settings.binaryDir");
  assert.equal(first.bioprism, path.join(fixture.binaryDir, EXE));

  const second = resolveBinaries(inputsFor(fixture, { settingsBinaryDir: undefined }));
  assert.ok(second);
  assert.equal(second.binarySource, "env.AURORA_AGENT_ROOT");
  assert.equal(second.root, fixture.envRoot);
  assert.equal(second.rootSource, "checkout");

  const third = resolveBinaries(inputsFor(fixture, { settingsBinaryDir: undefined, envRoot: undefined }));
  assert.ok(third);
  assert.equal(third.binarySource, "workspace");
  assert.equal(third.root, fixture.workspace);

  const fourth = resolveBinaries(
    inputsFor(fixture, { settingsBinaryDir: undefined, envRoot: undefined, workspaceFolders: [] })
  );
  assert.ok(fourth);
  assert.equal(fourth.binarySource, "home");
  assert.equal(fourth.root, path.join(fixture.home, "aurora-agent"));

  fs.rmSync(path.join(fixture.home, "aurora-agent"), { recursive: true, force: true });
  const fifth = resolveBinaries(
    inputsFor(fixture, { settingsBinaryDir: undefined, envRoot: undefined, workspaceFolders: [] })
  );
  assert.ok(fifth);
  assert.equal(fifth.binarySource, "home");
  assert.equal(fifth.root, path.join(fixture.home, "bioprism"));

  fs.rmSync(path.join(fixture.home, "bioprism"), { recursive: true, force: true });
  const sixth = resolveBinaries(
    inputsFor(fixture, { settingsBinaryDir: undefined, envRoot: undefined, workspaceFolders: [] })
  );
  assert.ok(sixth);
  assert.equal(sixth.binarySource, "cache");
  assert.equal(sixth.root, fixture.cacheVersionDir);
  assert.equal(sixth.rootSource, "bundle");
  assert.equal(sixth.bioprism, path.join(fixture.cacheVersionDir, "nested", EXE));

  fs.rmSync(fixture.cacheVersionDir, { recursive: true, force: true });
  const seventh = resolveBinaries(
    inputsFor(fixture, { settingsBinaryDir: undefined, envRoot: undefined, workspaceFolders: [] })
  );
  assert.equal(seventh, undefined);
});

test("auroraAgent.root setting overrides any detected root", (t) => {
  const fixture = buildFixture();
  t.after(() => fs.rmSync(fixture.base, { recursive: true, force: true }));

  const overridden = resolveBinaries(inputsFor(fixture, { settingsRoot: "C:\\data\\my-root" }));
  assert.ok(overridden);
  assert.equal(overridden.root, "C:\\data\\my-root");
  assert.equal(overridden.rootSource, "setting");
});

test("a binaryDir shaped like <checkout>/target/release recovers the checkout root", (t) => {
  const fixture = buildFixture();
  t.after(() => fs.rmSync(fixture.base, { recursive: true, force: true }));

  const releaseDir = path.join(fixture.envRoot, "target", "release");
  const resolved = resolveBinaries(inputsFor(fixture, { settingsBinaryDir: releaseDir }));
  assert.ok(resolved);
  assert.equal(resolved.binarySource, "settings.binaryDir");
  assert.equal(resolved.checkoutRoot, fixture.envRoot);
  assert.equal(resolved.root, fixture.envRoot);
  assert.equal(resolved.rootSource, "checkout");
});

test("a plain binaryDir yields no root without the setting", (t) => {
  const fixture = buildFixture();
  t.after(() => fs.rmSync(fixture.base, { recursive: true, force: true }));

  const resolved = resolveBinaries(
    inputsFor(fixture, { envRoot: undefined, workspaceFolders: [], homeDir: path.join(fixture.base, "empty-home") })
  );
  assert.ok(resolved);
  assert.equal(resolved.binarySource, "settings.binaryDir");
  assert.equal(resolved.checkoutRoot, undefined);
  assert.equal(resolved.root, undefined);
  assert.equal(resolved.rootSource, "none");
});

test("findBinariesUnder stops at depth and finds nested layouts", (t) => {
  const base = fs.mkdtempSync(path.join(os.tmpdir(), "aurora-scan-"));
  t.after(() => fs.rmSync(base, { recursive: true, force: true }));
  makeFakeExe(path.join(base, "a", "b"));
  const inputs = {
    existsSync: (p: string) => fs.existsSync(p),
    readdirSync: (p: string) => fs.readdirSync(p),
    isDirectory: (p: string) => {
      try {
        return fs.statSync(p).isDirectory();
      } catch {
        return false;
      }
    },
  };
  const found = findBinariesUnder(base, PLATFORM, inputs);
  assert.ok(found);
  assert.equal(found.bioprism, path.join(base, "a", "b", EXE));
  assert.equal(findBinariesUnder(path.join(base, "missing"), PLATFORM, inputs), undefined);
});
