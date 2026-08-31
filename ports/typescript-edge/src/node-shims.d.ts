declare const process: {
  readonly versions?: { readonly node?: string };
};

declare module "node:crypto" {
  export function createHash(algorithm: "sha256"): {
    update(data: Uint8Array): {
      digest(encoding: "hex"): string;
    };
    digest(encoding: "hex"): string;
  };
}
