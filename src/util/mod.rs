use bytes::{Bytes, BytesMut};

pub mod md5;

pub fn crc64(buf: &[u8]) -> u64 {
    use crc_any::CRC;

    let mut crc64 = CRC::crc64we();
    crc64.digest(buf);
    crc64.get_crc()
}

pub fn zlib_decompress(buf: &[u8]) -> anyhow::Result<Bytes> {
    use flate2::read::ZlibDecoder;
    use std::io::Read;

    let mut decoder = ZlibDecoder::new(&buf[..]);
    let mut ret = BytesMut::with_capacity(buf.len());
    ret.resize(buf.len(), 0);
    // let mut ret = vec![0; buf.len()];
    decoder.read_exact(&mut ret)?;
    Ok(ret.freeze())
}

pub fn md5(buf: &[u8]) -> [u8; 16] {
    md5::compute(&buf, [0x67452301, 0xEFCDAB89, 0x98BADCFE, 0x10325476])
}
