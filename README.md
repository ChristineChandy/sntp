sntp

A simple NTP client in Rust. It asks a time server for the time and shows how far off your computer's clock is.

$ cargo run -- time.cloudflare.com
server: time.cloudflare.com
offset: -627.626 ms (your clock is ahead)
delay: 11.531 ms
Usage
bash
cargo run                    # uses time.google.com
cargo run -- pool.ntp.org    # any NTP server
cargo test                   # run the tests
How it works

The program sends a 48-byte UDP packet to the server and records four timestamps:

T1 – client sends the request
T2 – server receives it
T3 – server sends the reply
T4 – client receives the reply
offset = ((T2 - T1) + (T3 - T4)) / 2
delay  = (T4 - T1) - (T3 - T2)

This assumes the packet takes the same time each way. If it doesn't, the offset is off by half the difference.

Tests

The tests use hand-calculated examples, including one where the network path is uneven. They caught a sign error that made the program report half the round-trip delay as the clock offset.

What I learned
How NTP works, and why it uses UDP: a resent packet would have an old timestamp.
Why the symmetric-delay assumption limits accuracy.
My WSL clock was drifting by hundreds of milliseconds, while three different servers agreed with each other. Fixing the time once isn't enough; the clock's rate has to be corrected too.
