# lenso.marketplace.echo

Ordinary Rust source compiled as a trusted native Process Plugin. The SDK owns the protocol bridge and runtime descriptor. Process Plugins are not sandboxed, so install only trusted bundles.

```sh
lenso plugin check
lenso plugin dev --operation execute --request-json '{"name":"lenso.marketplace.echo","arguments_json":"{\"text\":\"hello\"}"}'
lenso plugin pack
```
