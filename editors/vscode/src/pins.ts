export const RELEASE_VERSION = "0.1.3";

export const RELEASE_BASE_URL = `https://github.com/AURORA-NEURO/aurora-agent/releases/download/v${RELEASE_VERSION}/`;

export const SHA256SUMS = `bd6eaab534bf9a9cc33c16428f64e30cdec5f0f6382a5abc115641f7265601fb  aurora-agent-0.1.3-aarch64-apple-darwin.tar.gz
5653a3baacf1c08df89de7375909c9614940310e4116cd44eac3765f94586c64  aurora-agent-0.1.3-x86_64-apple-darwin.tar.gz
c8ccf580f2ebda241a10db42c87abeee170d403ffcbf35a0f3f6eb26233a8fa6  aurora-agent-0.1.3-x86_64-pc-windows-msvc.zip
01ce74afc7f01184c477fa1d4861e0cde71646b318e57774eed1708b033ef205  aurora-agent-0.1.3-x86_64-unknown-linux-gnu.tar.gz
`;

export function parseSums(text: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const line of text.split(/\r?\n/)) {
    const match = /^([0-9a-f]{64})[ \t*]+(\S+)\s*$/.exec(line.trim());
    if (match) {
      out.set(match[2], match[1]);
    }
  }
  return out;
}

export function platformArchive(platform: string, arch: string): string | undefined {
  if (platform === "win32" && arch === "x64") {
    return `aurora-agent-${RELEASE_VERSION}-x86_64-pc-windows-msvc.zip`;
  }
  if (platform === "darwin" && arch === "arm64") {
    return `aurora-agent-${RELEASE_VERSION}-aarch64-apple-darwin.tar.gz`;
  }
  if (platform === "darwin" && arch === "x64") {
    return `aurora-agent-${RELEASE_VERSION}-x86_64-apple-darwin.tar.gz`;
  }
  if (platform === "linux" && arch === "x64") {
    return `aurora-agent-${RELEASE_VERSION}-x86_64-unknown-linux-gnu.tar.gz`;
  }
  return undefined;
}

export function pinnedSha256(archiveName: string): string | undefined {
  return parseSums(SHA256SUMS).get(archiveName);
}
