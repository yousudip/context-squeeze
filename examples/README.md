# Examples

A small corpus for trying Context Squeeze from the CLI. Build first with
`cargo build --release`, then run the `cx` binary against these files.

```bash
# Signature-only map of this directory
cx skeleton examples/

# Squeeze a file to fit a 60-token budget (richest representation that fits)
cx squeeze examples/orders.py --budget 60

# Distill a noisy log into a ranked error anatomy
cx logs examples/service.log

# JSON output for scripting
cx squeeze examples/orders.py --budget 200 --json
```

Each command prints a one-line stats summary to stderr (file count, token
reduction) and the result to stdout.
