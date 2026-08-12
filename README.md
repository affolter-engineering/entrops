# prg320

Interfaces for reading entropy from a [PRG320](https://www.ingenieurbuero-bergmann.de/prg320.html)
hardware random number generator (HWRNG) by Ingenieurbüro Bergmann, connected via USB serial
(FTDI FT232R).

## Hardware

- Device: PRG320
- Interface: USB serial (appears as `/dev/ttyUSB0`, `/dev/ttyUSB1`, ...)
- Baud rate: 921600
- Output: raw bytes terminated by `\n`

Multiple devices are supported simultaneously.

## Development

```sh
$ nix-shell
$ cargo build --release
```

## Binaries

### One-shot CLI

Reads a single sample from `/dev/ttyUSB0` and prints JSON to stdout.

```sh
$ cargo run
```

```json
{
  "hex": "a3f1...",
  "int": 174832...,
  "raw": "[163, 241, ...]"
}
```

### `serve` - web server

Streams live entropy data from all connected USB serial devices.

```sh
$ cargo run --bin serve
```

Starts two listeners:

| Port | Protocol | Description |
|------|----------|-------------|
| 3003 | HTTP | Web UI, REST API and SSE stream |
| 3004 | TCP | Raw hex stream (newline-delimited) |

## Web UI

Open `http://localhost:3003` in a browser. Each connected device gets a live-updating panel showing the current hex output. A Stop/Start button pauses and resumes the stream.

## REST API

All endpoints return a JSON array with one entry per device. Samples are collected fresh on each request (up to 15 s timeout).

| Endpoint | Response fields |
|----------|----------------|
| `GET /api` | `timestamp`, `hex`, `int`, `raw` |
| `GET /api/hex` | `timestamp`, `hex` |
| `GET /api/int` | `timestamp`, `int` |
| `GET /api/raw` | `timestamp`, `raw` |

- `timestamp` is Unix epoch milliseconds recorded immediately after the port read completes.
- `raw` is the debug representation of the byte slice, e.g., `[163, 241, 10]`.

Example:

```sh
$ curl http://localhost:3003/api/hex
```

```json
[{"timestamp": 1750000000123, "hex": "a3f10a..."}]
```

## SSE stream

```sh
$ curl -N http://localhost:3003/stream
```

Emits one event per sample across all devices:

```
data: {"device":"/dev/ttyUSB0","hex":"a3f10a","int":"10743050"}
```

## TCP stream

Connect with `netcat` to receive a continuous newline-delimited HEX stream:

```sh
$ nc localhost 3004
```

```
a3f10a...
c72e91...
```

Each line is the hex-encoded output of one sample. Multiple concurrent clients are supported. The connection closes cleanly when the client disconnects.

## Feeding entropy to the kernel with rngd

`rngd` (from `rng-tools`) reads from a entropy source and writes into the kernel entropy pool (`/dev/random`), increasing the available entropy for cryptographic operations.

### Directly from the serial port

Configure the port baud rate and hand it to `rngd`:

```sh
$ stty -F /dev/ttyUSB0 921600 raw -echo
$ rngd -r /dev/ttyUSB0 -o /dev/random
```

For multiple devices, run one `rngd` instance per port.

### Via the TCP stream

The TCP endpoint emits hex-encoded samples. Decode on the fly and pipe into `rngd` using a named pipe:

```sh
$ mkfifo /tmp/entropy
$ nc localhost 3004 | xxd -r -p > /tmp/entropy &
$ rngd -r /tmp/entropy -o /dev/random -f
```

### Testing entropy quality with rngtest

`rngtest` (also from `rng-tools`) runs FIPS 140-2 statistical tests against the byte stream:

```sh
$ nix-shell -p rng-tools tinyxxd
$ nc localhost 3004 | xxd -r -p | rngtest -c 1000
rngtest 6.17
Copyright (c) 2004 by Henrique de Moraes Holschuh
This is free software; see the source for copying conditions.  There is NO warranty; not even for MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.

rngtest: starting FIPS tests...
rngtest: bits received from input: 10000032
rngtest: FIPS 140-2 successes: 500
rngtest: FIPS 140-2 failures: 0
rngtest: FIPS 140-2(2001-10-10) Monobit: 0
rngtest: FIPS 140-2(2001-10-10) Poker: 0
rngtest: FIPS 140-2(2001-10-10) Runs: 0
rngtest: FIPS 140-2(2001-10-10) Long run: 0
rngtest: FIPS 140-2(2001-10-10) Continuous run: 0
rngtest: input channel speed: (min=1.483; avg=41.093; max=19531250.000)Kibits/s
rngtest: FIPS tests speed: (min=60.551; avg=171.389; max=323.279)Mibits/s
rngtest: Program run time: 250446315 microseconds
```

A healthy PRG320 output should produce zero failures across all test categories.
