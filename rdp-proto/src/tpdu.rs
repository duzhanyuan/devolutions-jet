#[cfg(test)]
mod tests;

use std::io;

use byteorder::{BigEndian, LittleEndian, ReadBytesExt, WriteBytesExt};
use bytes::BytesMut;
use num_derive::{FromPrimitive, ToPrimitive};
use num_traits::{FromPrimitive, ToPrimitive};

pub const TPKT_HEADER_LENGTH: usize = 4;
pub const TPDU_DATA_LENGTH: usize = TPKT_HEADER_LENGTH + TPDU_DATA_HEADER_LENGTH;
pub const TPDU_REQUEST_LENGTH: usize = TPKT_HEADER_LENGTH + TPDU_REQUEST_HEADER_LENGTH;

const TPDU_DATA_HEADER_LENGTH: usize = 3;
const TPDU_REQUEST_HEADER_LENGTH: usize = 7;

#[derive(Copy, Clone, Debug, PartialEq, FromPrimitive, ToPrimitive)]
pub enum X224TPDUType {
    ConnectionRequest = 0xE0,
    ConnectionConfirm = 0xD0,
    DisconnectRequest = 0x80,
    Data = 0xF0,
    Error = 0x70,
}

pub fn decode_x224(input: &mut BytesMut) -> io::Result<(X224TPDUType, BytesMut)> {
    let mut stream = input.as_ref();
    let len = read_tpkt_len(&mut stream)? as usize;

    if input.len() < len {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "The buffer length is less then the real length",
        ));
    }

    let (_, code) = parse_tdpu_header(&mut stream)?;

    let mut tpdu = input.split_to(len as usize);
    let header_len = match code {
        X224TPDUType::Data => TPDU_DATA_LENGTH,
        _ => TPDU_REQUEST_LENGTH,
    };
    if header_len <= tpdu.len() {
        tpdu.advance(header_len);

        Ok((code, tpdu))
    } else {
        Err(io::Error::new(io::ErrorKind::UnexpectedEof, "TPKT len is too small"))
    }
}

pub fn encode_x224(code: X224TPDUType, data: BytesMut, output: &mut BytesMut) -> io::Result<()> {
    let length = TPDU_REQUEST_LENGTH + data.len();
    let mut output_slice = output.as_mut();
    write_tpkt_header(&mut output_slice, length as u16)?;
    write_tpdu_header(&mut output_slice, length as u8 - TPKT_HEADER_LENGTH as u8, code, 0)?;

    output.extend_from_slice(&data);

    Ok(())
}

pub fn write_tpkt_header(mut stream: impl io::Write, length: u16) -> io::Result<()> {
    let version = 3;

    stream.write_u8(version)?;
    stream.write_u8(0)?; // reserved
    stream.write_u16::<BigEndian>(length)?;

    Ok(())
}

pub fn read_tpkt_len(mut stream: impl io::Read) -> io::Result<u64> {
    let version = stream.read_u8()?;
    if version != 3 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a tpkt header"));
    }

    let _reserved = stream.read_u8()?;
    let len = u64::from(stream.read_u16::<BigEndian>()?);

    Ok(len)
}

fn write_tpdu_header(mut stream: impl io::Write, length: u8, code: X224TPDUType, src_ref: u16) -> io::Result<()> {
    // tpdu header length field doesn't include the length of the length field
    stream.write_u8(length - 1)?;
    stream.write_u8(code.to_u8().unwrap())?;

    if code == X224TPDUType::Data {
        let eot = 0x80;
        stream.write_u8(eot)?;
    } else {
        let dst_ref = 0;
        stream.write_u16::<LittleEndian>(dst_ref)?;
        stream.write_u16::<LittleEndian>(src_ref)?;
        let class = 0;
        stream.write_u8(class)?;
    }

    Ok(())
}

fn parse_tdpu_header(mut stream: impl io::Read) -> io::Result<(u8, X224TPDUType)> {
    let length = stream.read_u8()?;
    let code = X224TPDUType::from_u8(stream.read_u8()?)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "invalid X224 TPDU type"))?;

    if code == X224TPDUType::Data {
        let _eof = stream.read_u8()?;
    } else {
        let _dst_ref = stream.read_u16::<LittleEndian>()?;
        let _src_ref = stream.read_u16::<LittleEndian>()?;
        let _class = stream.read_u8()?;
    }

    Ok((length, code))
}
