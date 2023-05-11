# Tests

This directory contains
- Simple tests that push packets through compiled P4 pipelines and validate
  thaat the results are what we expect.
- A softnpu testing harness for writing automated tests. This harness may be
  imported by other crates.


## Testing

Many of the tests rely on waiting for packets to arrive on a particular
port. If the test is not working, this may never happen. Moreover, you will not
see printed output guiding to to what may be wrong as pipeline observability is
built on DTrace. You can however, use DTrace to ge ta sense for what is going on
with a test. For example

```
pfexec dtrace -x strsize=4k  -Z -s $P4_REPO/p4/dtrace/softnpu-monitor.d -c 'cargo test'
```

Where `$P4_REPO` is an environment variable pointing the top level directory of
the `p4` repo.
