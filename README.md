## Why ##
Running multiple applications that have websocket connections established from same IP address to public Hermes endpoint might get that IP address rate limited, therefore connections will be closed.

This can happen if amount of bytes received exceeds the limit per IP address.

Hermes websocket gateway establishes only one websocket connection to public Hermes endpoint, aggregates feed ids and streams corresponding Pyth price updates to subscribers. Applications can use gateway's endpoint instead.
