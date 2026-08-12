//! Get data from PRG320 and print it as JSON.

use num_bigint::BigUint;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use std::time::Duration;

fn main() {
    let port = serialport::new("/dev/ttyUSB0", 921600)
        .timeout(Duration::from_secs(10))
        .open()
        .expect("Failed to open serial port");

    let mut data = Vec::new();
    BufReader::new(port)
        .read_until(b'\n', &mut data)
        .expect("Failed to read from serial port");

    let mut output: BTreeMap<&str, serde_json::Value> = BTreeMap::new();
    output.insert("hex", hex::encode(&data).into());
    output.insert(
        "int",
        serde_json::Value::Number(
            BigUint::from_bytes_be(&data)
                .to_string()
                .parse()
                .unwrap(),
        ),
    );
    output.insert("raw", format!("{:?}", data).into());

    println!("{}", serde_json::to_string_pretty(&output).unwrap());
}
