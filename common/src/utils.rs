use std::{
    fs::File,
    io::{self, BufReader},
    path::Path,
};

use crate::reader::{IntoReader, PacketWriter};

pub fn read_purchase_file<P: AsRef<Path>>(path: P) -> io::Result<Vec<String>> {
    // Read data
    let path = path.as_ref();
    let file = File::open(path).unwrap();
    let mut r = BufReader::new(file);
    let mut buf = Vec::new();
    std::io::Read::read_to_end(&mut r, &mut buf)?;

    // Decode
    let decoded: Vec<u8> = buf
        .iter()
        .enumerate()
        .map(|(i, b)| b ^ ((i % 212) as u8))
        .collect();

    let mut values = Vec::new();
    let mut r = decoded.into_reader();
    loop {
        let value = r.read_string()?;
        if value.is_empty() {
            break;
        }
        values.push(value);
    }
    Ok(values)
}

pub fn write_purchase_file<P: AsRef<Path>>(path: P, values: &[String]) -> io::Result<()> {
    let mut w = PacketWriter::new();
    for value in values {
        w.write_string(value);
    }
    w.write_string("");
    let data = w.finish();
    let data_encoded = data
        .into_iter()
        .enumerate()
        .map(|(i, byte)| byte ^ (i % 212) as u8)
        .collect::<Vec<u8>>();

    std::fs::write(path, data_encoded)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_read_purchase_file() {
        let mut data = read_purchase_file("../dumps/secureNew").unwrap();
        data[0] = "fef7aa12198affe44bf90a1fecc0f92ff4304a51".to_string();
        write_purchase_file("../dumps/secureNewModified", &data).unwrap();
        println!("{:?}", data);
        println!(
            "{:?}",
            read_purchase_file("../dumps/secureNewModified").unwrap()
        );
    }
}
