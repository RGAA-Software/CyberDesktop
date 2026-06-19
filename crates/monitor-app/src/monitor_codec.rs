use std::io::{Read, Write};

use flate2::read::ZlibDecoder;
use flate2::write::ZlibEncoder;
use flate2::Compression;

use crate::sys_info::SysInfo;

pub fn encode_telemetry(info: &SysInfo) -> anyhow::Result<Vec<u8>> {
    let bytes = bincode::serialize(info)?;
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(&bytes)?;
    Ok(encoder.finish()?)
}

pub fn decode_telemetry(bytes: &[u8]) -> anyhow::Result<SysInfo> {
    let mut decoder = ZlibDecoder::new(bytes);
    let mut decoded = Vec::new();
    decoder.read_to_end(&mut decoded)?;
    Ok(bincode::deserialize(&decoded)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sys_info::SysInfo;

    #[test]
    fn test_encode_decode_roundtrip() {
        let info = SysInfo::default();
        let encoded = encode_telemetry(&info).expect("encode");
        let decoded = decode_telemetry(&encoded).expect("decode");
        assert_eq!(info.timestamp, decoded.timestamp);
    }

    #[test]
    fn test_compression_reduces_size_for_repetitive_data() {
        let mut info = SysInfo::default();
        for i in 0..100 {
            let mut process = crate::sys_info::SysProcessInfo::default();
            process.pid = i;
            process.name = format!("process_{}", i % 10);
            process.exe = "C:\\Windows\\System32\\notepad.exe".to_string();
            info.processes.push(process);
        }
        let bincode_bytes = bincode::serialize(&info).expect("bincode");
        let compressed = encode_telemetry(&info).expect("compress");
        assert!(
            compressed.len() < bincode_bytes.len(),
            "compressed {} should be smaller than bincode {}",
            compressed.len(),
            bincode_bytes.len()
        );
        let decoded = decode_telemetry(&compressed).expect("decode");
        assert_eq!(info.processes.len(), decoded.processes.len());
    }
}
