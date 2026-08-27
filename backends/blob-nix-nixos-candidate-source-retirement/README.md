# `blob-nix-nixos-candidate-source-retirement`

Quiescence-proved reclamation of trusted NixOS candidate source GC roots.

This module composes two independent safety boundaries:

- the materialization lifecycle proof from ADR-0040, which decides when candidate **selection** state may be retired;
- the shared enqueue lease/barrier protocol, which proves that no pre-publication enqueue can still depend on that source.

Source reclamation is allowed only after:

1. a durable `retiring` barrier prevents new enqueue access;
2. no active enqueue lease remains for the manifest;
3. ADR-0040 successfully retires the manifest and producer receipt;
4. quiescence is checked again;
5. the barrier becomes the permanent `retired` tombstone;
6. the exact source path is recovered from ADR-0040's root-owned `source-retained:` lifecycle evidence;
7. the candidate source GC root still points exactly to that path;
8. a root-owned source-retirement receipt is made durable before the GC root is removed.

A missing source GC root is never guessed to be successfully reclaimed. It is idempotently accepted only when exact prior source-retirement evidence already exists. Conflicting or malformed state fails closed.

This module does not release admitted output roots, delete completed materialization/admission state, change activation policy, or perform any live/persistent activation.
