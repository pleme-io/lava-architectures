# lava-architectures

Reusable infrastructure compositions for the lava suite.

Hand-authored typed architectures — `AwsVpcNetwork`, `AkeylessAwsIntegration`,
`EksScaleTest`, and others — built on the rest of the lava family.

The Pangea-architectures analog. The port produces **byte-equivalent
`terraform.json`**, so state files match between `pangea` + `tofu` and
`lava` + [`magma`](https://github.com/pleme-io/magma). That byte-equivalence
is the property that makes migrating an existing estate a non-event.

## Install

```toml
[dependencies]
lava-architectures = "0.1"
```

## The suite

This crate sits at the top of the DAG and consumes nearly all of it:

```
lava-types ─► lava-schema ─┐
lava-core ─┬─► lava-arch ──┼─► lava-eval ─► lava-test ─┐
           └─► lava-contracts ───────────────────────► lava-architectures
```

## License

MIT
