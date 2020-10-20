use std::io::{BufWriter, Write};

use common::reader::PacketWriter;
fn main() {
    // Check args
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("Usage: blah <c array file> <raw out>");
        return;
    }

    // Read input file
    let input = std::fs::read_to_string(args.get(1).unwrap()).unwrap();

    // Parse input file
    // Output from
    // `tshark -r AmongUsDump2.pcapng -Y 'udp.port == 22023' -Tfields -e 'udp.srcport' -e 'data.data' > dump.txt`
    let mut packets = Vec::new();
    for line in input.lines() {
        if line.is_empty() {
            continue;
        }
        let mut split = line.split_ascii_whitespace();
        let port_str = split.next().unwrap();
        let data_str = split.next().unwrap();
        let port = u16::from_str_radix(port_str, 10).unwrap();
        let to_server = port != 22023;
        let data = decode_hex(data_str);
        packets.push((to_server, "", data));
    }

    // Write to output file
    let out_file = std::fs::File::create(args.get(2).unwrap()).unwrap();
    let mut file_w = BufWriter::new(out_file);
    println!("{}", packets.len());
    for (to_server, _name, bytes) in packets.into_iter() {
        let mut w = PacketWriter::new();

        // Start header
        w.write_bool(to_server);

        // Get padded data
        let bytes_len = bytes.len();
        let mut pad_w = PacketWriter::new();
        pad_w.write_bytes_raw(&bytes);
        pad_w.write_bytes_raw(&vec![0; 16 - (bytes.len() % 16)]);
        let padded_len = pad_w.len();

        // Finish header
        w.write_u32(bytes_len as u32);
        w.write_u32(padded_len as u32);
        w.write_bytes_raw(&[0; 7]);

        // Write Data
        w.write_bytes_raw(&pad_w.finish());
        file_w.write_all(&w.finish()).unwrap();
    }

    println!("Done");
}

pub fn decode_hex(s: &str) -> Vec<u8> {
    if s.len() % 2 != 0 {
        panic!("Wrong length hex input");
    } else {
        (0..s.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&s[i..i + 2], 16).unwrap())
            .collect()
    }
}
