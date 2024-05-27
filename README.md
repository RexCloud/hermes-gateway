[![Build Status](https://img.shields.io/github/actions/workflow/status/RexCloud/hermes-gateway/rust.yml?style=for-the-badge)](https://github.com/RexCloud/hermes-gateway/actions)

## Why ##
Running multiple applications that have websocket connections established from same IP address to public Hermes endpoint might get that IP address rate limited, therefore connections will be closed.

This can happen if amount of bytes received exceeds the limit per IP address.

Hermes websocket gateway establishes only one websocket connection to public Hermes endpoint, aggregates feed ids and streams corresponding Pyth price updates to subscribers. Applications can use gateway's endpoint instead.

## Usage ##

Compile using this command:

```shell
cargo build --release
```

Move compiled file or `cd` into `target/release`

```shell
./hermes-gateway
```

```
Usage: hermes-gateway <COMMAND>

Commands:
  ws    Run on websocket
  ipc   Run on Unix socket
  help  Print this message or the help of the given subcommand(s)

Options:
  -h, --help  Print help
```

Run with custom address or path (Default: `127.0.0.1:7071` for `ws` and `/tmp/hermes_gateway.ipc` for `ipc`):

```shell
./hermes-gateway ws 127.0.0.1:7081
```

```shell
./hermes-gateway ipc /path/to/socket
```

Reading from Unix socket connection:

1. Read u16, this value represents price update size

2. Create buffer with this size `vec![0; len]`

3. Read exact size to this buffer

4. Parse buffer into a struct using serde_json
