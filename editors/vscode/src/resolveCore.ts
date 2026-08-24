import * as path from "path";

export type BinarySource =
  | "settings.binaryDir"
  | "env.AURORA_AGENT_ROOT"
  | "workspace"
  | "home"
  | "cache";

export type RootSource = "setting" | "checkout" | "bundle" | "none";

export interface ResolveInputs {
  platform: string;
  settingsBinaryDir?: string;
  settingsRoot?: string;
  envRoot?: string;
  workspaceFolders: string[];
  homeDir: string;
  cacheVersionDir: string;
  existsSync(candidate: string): boolean;
  readdirSync(dir: string): string[];
  isDirectory(candidate: string): boolean;
}

export interface ResolvedBinaries {
  bioprism: string;
  bioprismMcp: string;
  bioprismApi: string;
  binarySource: BinarySource;
  checkoutRoot?: string;
  root?: string;
  rootSource: RootSource;
}

export function exeName(base: string, platform: string): string {
  return platform === "win32" ? `${base}.exe` : base;
}

function binariesInDir(dir: string, platform: string, existsSync: (p: string) => boolean) {
  const bioprism = path.join(dir, exeName("bioprism", platform));
  if (!existsSync(bioprism)) {
    return undefined;
  }
  return {
    bioprism,
    bioprismMcp: path.join(dir, exeName("bioprism-mcp", platform)),
    bioprismApi: path.join(dir, exeName("bioprism-api", platform)),
  };
}

function binariesInCheckout(root: string, platform: string, existsSync: (p: string) => boolean) {
  return binariesInDir(path.join(root, "target", "release"), platform, existsSync);
}

function checkoutRootFromBinaryDir(binaryDir: string): string | undefined {
  const normalized = path.normalize(binaryDir);
  const release = path.basename(normalized);
  const targetDir = path.dirname(normalized);
  if (release === "release" && path.basename(targetDir) === "target") {
    return path.dirname(targetDir);
  }
  return undefined;
}

export function findBinariesUnder(
  dir: string,
  platform: string,
  inputs: Pick<ResolveInputs, "existsSync" | "readdirSync" | "isDirectory">,
  maxDepth = 3
): { bioprism: string; bioprismMcp: string; bioprismApi: string } | undefined {
  if (!inputs.existsSync(dir) || !inputs.isDirectory(dir)) {
    return undefined;
  }
  const direct = binariesInDir(dir, platform, inputs.existsSync);
  if (direct) {
    return direct;
  }
  if (maxDepth <= 0) {
    return undefined;
  }
  let entries: string[];
  try {
    entries = inputs.readdirSync(dir);
  } catch {
    return undefined;
  }
  for (const entry of entries.sort()) {
    const child = path.join(dir, entry);
    if (!inputs.isDirectory(child)) {
      continue;
    }
    const found = findBinariesUnder(child, platform, inputs, maxDepth - 1);
    if (found) {
      return found;
    }
  }
  return undefined;
}

function applyRoot(
  partial: Omit<ResolvedBinaries, "root" | "rootSource">,
  settingsRoot: string | undefined,
  bundleDir: string | undefined
): ResolvedBinaries {
  if (settingsRoot && settingsRoot.trim() !== "") {
    return { ...partial, root: settingsRoot, rootSource: "setting" };
  }
  if (partial.checkoutRoot) {
    return { ...partial, root: partial.checkoutRoot, rootSource: "checkout" };
  }
  if (bundleDir) {
    return { ...partial, root: bundleDir, rootSource: "bundle" };
  }
  return { ...partial, root: undefined, rootSource: "none" };
}

export function resolveBinaries(inputs: ResolveInputs): ResolvedBinaries | undefined {
  const { platform, existsSync } = inputs;

  if (inputs.settingsBinaryDir && inputs.settingsBinaryDir.trim() !== "") {
    const dir = inputs.settingsBinaryDir;
    const found = binariesInDir(dir, platform, existsSync);
    if (found) {
      return applyRoot(
        {
          ...found,
          binarySource: "settings.binaryDir",
          checkoutRoot: checkoutRootFromBinaryDir(dir),
        },
        inputs.settingsRoot,
        undefined
      );
    }
  }

  if (inputs.envRoot && inputs.envRoot.trim() !== "") {
    const found = binariesInCheckout(inputs.envRoot, platform, existsSync);
    if (found) {
      return applyRoot(
        { ...found, binarySource: "env.AURORA_AGENT_ROOT", checkoutRoot: inputs.envRoot },
        inputs.settingsRoot,
        undefined
      );
    }
  }

  for (const folder of inputs.workspaceFolders) {
    const found = binariesInCheckout(folder, platform, existsSync);
    if (found) {
      return applyRoot(
        { ...found, binarySource: "workspace", checkoutRoot: folder },
        inputs.settingsRoot,
        undefined
      );
    }
  }

  for (const name of ["aurora-agent", "bioprism"]) {
    const root = path.join(inputs.homeDir, name);
    const found = binariesInCheckout(root, platform, existsSync);
    if (found) {
      return applyRoot(
        { ...found, binarySource: "home", checkoutRoot: root },
        inputs.settingsRoot,
        undefined
      );
    }
  }

  const cached = findBinariesUnder(inputs.cacheVersionDir, platform, inputs);
  if (cached) {
    return applyRoot(
      { ...cached, binarySource: "cache", checkoutRoot: undefined },
      inputs.settingsRoot,
      inputs.cacheVersionDir
    );
  }

  return undefined;
}
