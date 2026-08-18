# Bundle signatures

The result-bundle layer has two intentionally separate authentication contracts:

| wrapper | primitive | verifier material | honest claim |
| --- | --- | --- | --- |
| `AttestedBundle` | HMAC-SHA256 | shared secret | a holder of the shared key produced the authenticated bytes; every verifier can also forge |
| `PubliclyAttestedBundle` | Ed25519 | public verification key | the holder of the corresponding private key signed the canonical bytes; a verifier does not receive signing material |

The distinction is represented in the Rust type names, serialized `scheme`, `repudiability`, and
the MCP result's `verification_mode`. An old HMAC bundle is never silently reinterpreted as an
Ed25519 bundle.

## Signed bytes

`PublicKeyAttestation` signs the canonical JSON object containing:

- the public-key attestation schema version;
- the narrow `AttestationPurpose`;
- the exact `subject_digest` (for a result bundle, the manifest digest);
- the `key_identity` label;
- the self-reported producer string;
- the caller-supplied nonce and display timestamp;
- the optional numeric `signed_at` Unix instant; and
- the `ed25519` authentication scheme.

The bundle verifier first recomputes every carried inline entry digest, checks the embedded
certificate, and computes the manifest digest. Only then does it compare the attestation subject and
verify the Ed25519 signature. This ordering prevents a valid signature over a stale manifest from
being treated as evidence for rewritten carried bytes.

The purpose is inside the signed preimage and is also checked by `verify_for`. A publisher
attestation therefore cannot be replayed as a hub receipt merely by changing the requested purpose.
This is purpose binding, not a key-registry policy requiring separate keys for separate roles.

## Key validity

`VerificationKey` carries an optional caller-declared `KeyValidity` window:

```json
{
  "key_identity": "publisher-ed25519",
  "public_key": "ed25519:<64 lowercase hex characters>",
  "validity": {"not_before": 1755552000, "not_after": 1787088000}
}
```

If either bound is present, the attestation must carry `signed_at`. Verification compares the
numeric values without reading the verifier's wall clock. Missing signing time, inverted windows,
pre-activation signatures, and expired signatures are separate fail-closed outcomes. An unbounded
key does not imply that the key is current or authorized; it only means this local window check was
not requested.

## MCP and SDK boundary

`bundle_verify` accepts either the legacy `bundle`, a root-confined `document`, or an inline
`publicly_attested_bundle`. The public-key form requires `verification_key` and returns:

- `verification_mode: "ed25519_public_key"`;
- the recomputed manifest and entry posture;
- serialized cryptographic authentication evidence;
- guarantee rows describing what was actually checked; and
- limitation rows describing what remains external.

The Python `BundleVerifyArgs`/`BundleVerifyReport` and TypeScript `BundleVerifyArgs`/
`BundleVerifyResult` surfaces preserve the same source exclusivity, key format, validity bounds,
success/refusal distinction, and public-verification mode. The REST client reaches the same MCP
tool through `/v1/tools/bundle_verify`; no second verification implementation exists in the HTTP
gateway.

## Security boundary

This implementation does establish:

- Ed25519 signature verification against the supplied public key;
- binding of the signature to the manifest digest and attestation purpose;
- detection of changed carried content before authentication;
- distinction between wrong key, wrong purpose, invalid key window, malformed input, and signature
  mismatch; and
- verifier-side non-forgeability under Ed25519's cryptographic assumptions.

It does not establish:

- that `key_identity` belongs to the named producer or organization;
- that the key was authorized for publisher, builder, or hub use;
- a certificate chain, cross-signature, HSM custody, compromise history, rotation record, CRL, or
  revocation status;
- an independently observed timestamp (`recorded_at` and `signed_at` are caller inputs);
- remote retrieval of referenced entries or provider logs; or
- scientific, clinical, deployment, release, or publication authority.

Those claims require separate key-registry, identity, transparency, execution, and governance
contracts. Keeping them out of `PubliclyAttestedBundle` is deliberate: a valid cryptographic
signature is evidence about bytes and a private key, not a universal approval signal.
