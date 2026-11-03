# Published history privacy

On 2026-11-03, published branch/tag history and release packages were sanitized.
Private lab management endpoints, personal paths and machine-specific access
instructions were replaced with generic examples. Historical test outcomes and
known failures were retained. Old commit identities were normalized to the public
project identity in the rewritten history. No credential or private-key exposure
was confirmed.

The v0.1.0 executable bytes are unchanged. Source/documentation packages and
checksum signatures were replaced; download the complete current set rather
than mixing an old manifest with a new archive. Package provenance retains the
original binary build revision and identifies the cleaned source revision
separately. This cleanup is not a new build or qualification result.

Existing clones can retain the old history. Re-clone or carefully rebase local
work onto the rewritten branches; do not merge old history back into the project.
GitHub may retain read-only PR references and cached commits until server-side
cleanup is completed. Rewriting branches alone does not purge those copies.
