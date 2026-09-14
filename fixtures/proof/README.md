# Marketplace installation fixture

This SDK-authored Process plugin exposes `marketplace_proof`. It returns a
non-empty `text` argument unchanged. No account or network service is required.
It is trusted native test code, not a security sandbox.

Run `build-variants.py OUTPUT_DIR` with `LENSO` pointing to the CLI to create
immutable archives. Version 0.2.0 prefixes the actual response with `v2: `;
version 0.3.0 deliberately advertises an invalid Tool name and must fail before
Ready. The script creates and removes private build directories inside this
repository, without modifying the fixture source. Use a new output directory;
the pack command refuses to overwrite an existing immutable archive.
