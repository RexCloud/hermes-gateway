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

Run using this command:

```shell
./hermes-gateway
```

Gateway will start accepting connections at `localhost:7071`

Run with custom address, e.g. `localhost:7081`:

```shell
./hermes-gateway 127.0.0.1:7081
```
