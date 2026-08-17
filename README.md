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

## Rendering a dashboard: `lava-render`

The `dashboards/` catalogue holds `(deflava-dashboard …)` sources that render
to Grafana JSON. `lava-render` does that render with **no pangea-operator in
the loop**, so the shipped bundle is regenerable from a clean checkout rather
than only reproducible inside a cluster.

```sh
cargo run --bin lava-render -- --list

cargo run --bin lava-render -- workload-overview \
  --param env=camelot --param service=auth --param job=auth \
  --param namespace=default \
  --param datasource=mimir --param logs_datasource=vlogs \
  --out workload-overview.json
```

A bare name resolves against `dashboards/`; an argument containing a path
separator or ending in `.tlisp` is used as-is. Output is 2-space-indented with
a trailing newline, so a regenerated bundle diffs line-by-line.

It exits non-zero — writing nothing — on an unknown dashboard, an unreadable
file, an evaluation failure, a `{placeholder}` the source requires that no
`--param` supplied, a `--param` the source does not use, and an unbound
placeholder that survives into the rendered output. That last one is the point:
an unbound `{service}` renders perfectly and ships a literal brace into a live
dashboard title, which nothing downstream would have caught.

## The suite

This crate sits at the top of the DAG and consumes nearly all of it:

```
lava-types ─► lava-schema ─┐
lava-core ─┬─► lava-arch ──┼─► lava-eval ─► lava-test ─┐
           └─► lava-contracts ───────────────────────► lava-architectures
```

## License

MIT
