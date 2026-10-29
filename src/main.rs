//NTP is 48 bytes
//byte 0 is flags where leap indicator is 2 bit, version is 3 bits and mode is also 3 bits
//byte 1 is stratum which is server's distance from the reference clock if its 0 that mean server is refusing the client
//bytes 32-39 are for receiving the timestamp, T2 is when the server gets your request
//bytes 40-47 are for transmitting timestamp T3 is when the server is sent its reply

//each NTP timestamp is 8 bytes where the first 4 byte is seconds since Jan 1, 1900 and last 4 bytes is the fraction of a second out of 2^32
//Unix counts from 1970 so I need to subtract 2,208,988,800 from the time

use std::net::UdpSocket;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const NTP_PORT: u16 = 123;
const NTP_PACKET_SIZE: usize = 48;
const NTP_TO_UNIX_SECONDS: i64 = 2208988800;
const NANOS_PER_SEC: i64 = 1000000000;

fn now_nanos() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before 1970")
        .as_nanos() as i64
}

fn read_ntp_timestamp(bytes: &[u8]) -> i64 { //i64 instead of f64 because it has more precision
    let seconds = u32::from_be_bytes(bytes[0..4].try_into().unwrap()) as i64;
    let fractions = u32::from_be_bytes(bytes[4..8].try_into().unwrap()) as u64;

    let nanos = (fractions * NANOS_PER_SEC as u64) >> 32;

    (seconds - NTP_TO_UNIX_SECONDS) * NANOS_PER_SEC + nanos as i64
}

//t1: client send t2: server receive, t3: server send, t4: client receive
fn compute_offset_delay(t1: i64, t2: i64, t3: i64, t4: i64) -> (i64, i64) {
    let offset = ((t2 - t1) + (t3 - t4)) / 2; //average on length of each trip, it should be symmetric for this project if positive the server is ahead
    let delay = (t4 - t1) - (t3 -t2); // total round trip minus the amount of time server holds the packet
    (offset, delay)
}

fn query(server: &str) -> Result<(i64, i64), Box<dyn std::error::Error>> {
    let socket = UdpSocket::bind("0.0.0.0:0")?; //any local port
    socket.set_read_timeout(Some(Duration::from_secs(5)))?; //the ? passes the error up to the caller instead of crashing the program
    socket.connect((server, NTP_PORT))?;

    let mut request = [0u8; NTP_PACKET_SIZE];
    request[0] = 0x1B; //LI=0 VN=3, Mode=3 (client)

    let t1 = now_nanos();
    socket.send(&request)?;

    let mut response = [0u8; NTP_PACKET_SIZE];
    let len = socket.recv(&mut response)?;
    let t4 = now_nanos();

    if len < NTP_PACKET_SIZE {
        return Err("response too short".into());
    }
    if response[0] & 0b00000111 != 4 {
        return Err("reply is not in server mode".into());
    }
    if response[1] == 0 {
        return Err("server sent kiss-of-death (rate limited?)".into());
    }

    let t2 = read_ntp_timestamp(&response[32..40]);
    let t3 = read_ntp_timestamp(&response[40..48]);

    Ok(compute_offset_delay(t1, t2, t3, t4))

}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let server = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "time.google.com".to_string());
    let (offset, delay) = query(&server)?;
    let ms = |ns: i64| ns as f64 / 1_000_000.0;

    let direction = if offset > 0 {"behind"} else {"ahead"};
    println!("server: {server}");
    println!("offset: {:+.3} ms (your clock is {direction})", ms(offset));
    println!("delay: {:.3} ms", ms(delay));
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn half_second_fraction() {
        //NTP seconds exactly at the Unix epoch, plus 0x80000000 / 2^32 = 0.5 s
        let mut b = [0u8; 8];
        b[0..4].copy_from_slice(&(NTP_TO_UNIX_SECONDS as u32).to_be_bytes());
        b[4..8].copy_from_slice(&0x80000000u32.to_be_bytes());
        assert_eq!(read_ntp_timestamp(&b), 500000000);
    }

    #[test]
    fn symmetric_path() {
        //server is 100 ahead, each direction takes 10 server holds packet for 10
        //t1 = 0 -> arrives at local 10 = server 110; server replies at 120 = local 20; 
        //arrives at local 30
        assert_eq!(compute_offset_delay(0, 110, 120, 30), (100, 20));
    }

    #[test]
    fn asymmetric_path_causes_error() {
        //Same true offset of 100, but 20 out and 0 back
        //t1 = 0 -> arrives local 20 = server 120; replies at server 130 = local 30;
        //arrives at local 30
        let (offset, _) = compute_offset_delay(0,120,130,30);
        assert_eq!(offset, 110); //wrong by (20-0) / 2 = 10;
    }
}

