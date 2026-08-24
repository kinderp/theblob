# Phase 2B validation checkpoint

This document exists to trigger and record CI validation of the first explicit WASIp2 grant implementation.

The test matrix must prove:

- no filesystem grant -> guest cannot access `/workspace`;
- read-only preopen -> guest can read the granted directory;
- read-only preopen -> guest cannot create/write output;
- read-write preopen -> guest can write inside the granted directory;
- parent traversal through `..` cannot escape the preopened directory;
- the original empty-linker deny-by-default Component runtime remains green;
- core and renderer jobs remain green.

The guest fixture is compiled with the Rust `wasm32-wasip2` target and executed through Wasmtime's synchronous WASIp2 Component bindings. The host does not inherit environment, argv, stdio or network authority.

This is a validation artifact, not a new authority model. Blob policy/BindingLease remains the authority source; the runtime only materializes already-authorized resources.
