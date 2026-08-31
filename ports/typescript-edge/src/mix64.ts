/**
 * SplitMix64 finalizer and rendezvous (highest-random-weight) shard placement, transcribed
 * from integrations/agent-fabric/src/ids.rs (mix64) and src/shard.rs (score,
 * preference_order, home_shard).
 *
 * u64 arithmetic uses BigInt because JavaScript numbers lose integer precision above 2^53
 * and placement must be bit-exact with the Rust fabric. Every value entering or leaving this
 * module is a BigInt masked to 64 bits; nothing here accepts `number` for keys or scores so a
 * precision-silent coercion cannot happen at a call site.
 */

const MASK_64 = 0xffffffffffffffffn;
const GOLDEN = 0x9e3779b97f4a7c15n;
const MUL_A = 0xbf58476d1ce4e5b9n;
const MUL_B = 0x94d049bb133111ebn;
const SHARD_SEED = 0x517cc1b727220a95n;

/** SplitMix64 finalizer: the exact avalanche mixer shard scoring and key derivation share. */
export function mix64(x: bigint): bigint {
  let v = x & MASK_64;
  v = (v + GOLDEN) & MASK_64;
  v = ((v ^ (v >> 30n)) * MUL_A) & MASK_64;
  v = ((v ^ (v >> 27n)) * MUL_B) & MASK_64;
  return (v ^ (v >> 31n)) & MASK_64;
}

function score(key: bigint, shard: bigint): bigint {
  const shifted = (shard + SHARD_SEED) & MASK_64;
  return mix64(mix64(key) ^ mix64(shifted));
}

/**
 * Full preference order over `shardCount` shards for `key`, best first. A zero or absurd
 * shard count is a configuration bug, not a runtime condition, so it throws rather than
 * degrading to an empty order that callers might read as "no placement possible".
 */
export function preferenceOrder(shardCount: bigint, key: bigint): bigint[] {
  if (shardCount <= 0n || shardCount > 4096n) {
    throw new RangeError(`shard count must be in 1..=4096, got ${shardCount}`);
  }
  const ranked: Array<[bigint, bigint]> = [];
  for (let s = 0n; s < shardCount; s += 1n) {
    ranked.push([score(key, s), s]);
  }
  ranked.sort((a, b) => {
    if (a[0] !== b[0]) return a[0] > b[0] ? -1 : 1;
    return a[1] < b[1] ? -1 : 1;
  });
  return ranked.map(([, shard]) => shard);
}

/** The single best shard for `key` — equivalent to `preferenceOrder(..)[0]`. */
export function homeShard(shardCount: bigint, key: bigint): bigint {
  return preferenceOrder(shardCount, key)[0] as bigint;
}

/** Renders a u64 as the decimal-string form parity vectors use to survive JSON tooling. */
export function u64String(v: bigint): string {
  return (v & MASK_64).toString(10);
}
