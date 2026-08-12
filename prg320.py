"""Get data from PRG320 and print it as JSON."""
import asyncio
import binascii
import json

import serialx


async def main():
    """Get the output of PRG320."""

    reader, writer = await serialx.open_serial_connection(
        url="/dev/ttyUSB0", baudrate=921600
    )
    data = await reader.readline()
    output = {
        "raw": str(repr(data)),
        "hex": str(binascii.hexlify(bytearray(data))),
        "int": int(bytearray(data).hex(), 16),
    }
    print(json.dumps(output, sort_keys=True, indent=2))
    writer.close()

asyncio.run(main())
