//! Just enough protobuf to read and rewrite Discord's settings blob. We
//! never decode a whole message into a struct: unknown fields must survive a
//! round trip untouched, so everything works on raw field slices.

use crate::Error;

/// Bounds against a corrupt or hostile blob.
const MAX_FIELDS: usize = 4096;

/// One field of a message, with its bytes as they appeared on the wire.
#[derive(Clone, Copy, Debug)]
pub struct Field<'a> {
    pub number: u32,
    wire_type: u8,
    /// Tag and value together, for copying the field unchanged.
    pub raw: &'a [u8],
    value: &'a [u8],
}

impl<'a> Field<'a> {
    /// The payload of a length-delimited field.
    pub fn message(&self) -> Result<&'a [u8], Error> {
        if self.wire_type == 2 {
            Ok(self.value)
        } else {
            Err(Error::Malformed("expected a nested message"))
        }
    }

    pub fn varint(&self) -> Result<u64, Error> {
        if self.wire_type != 0 {
            return Err(Error::Malformed("expected a varint"));
        }
        let mut value = self.value;
        read_varint(&mut value)
    }

    /// The payload of a length-delimited field read as text.
    pub fn text(&self) -> Result<&'a str, Error> {
        std::str::from_utf8(self.message()?).map_err(|_| Error::Malformed("text is not utf-8"))
    }

    /// A single fixed64 value.
    pub fn fixed64(&self) -> Result<u64, Error> {
        match (self.wire_type, <[u8; 8]>::try_from(self.value)) {
            (1, Ok(bytes)) => Ok(u64::from_le_bytes(bytes)),
            _ => Err(Error::Malformed("expected a fixed64")),
        }
    }

    /// Repeated fixed64 values, packed in one field or sent one by one.
    pub fn fixed64s(&self) -> Result<Vec<u64>, Error> {
        match self.wire_type {
            1 | 2 => {
                let (chunks, rest) = self.value.as_chunks::<8>();
                if !rest.is_empty() {
                    return Err(Error::Malformed("bad fixed64 run"));
                }
                Ok(chunks.iter().map(|c| u64::from_le_bytes(*c)).collect())
            }
            _ => Err(Error::Malformed("expected fixed64 values")),
        }
    }
}

pub fn read_varint(input: &mut &[u8]) -> Result<u64, Error> {
    let mut value = 0u64;
    for shift in (0..70).step_by(7) {
        let (&byte, rest) = input
            .split_first()
            .ok_or(Error::Malformed("truncated varint"))?;
        *input = rest;
        if shift == 63 && byte > 1 {
            return Err(Error::Malformed("varint overflows 64 bits"));
        }
        value |= u64::from(byte & 0x7f) << shift;
        if byte < 0x80 {
            return Ok(value);
        }
    }
    Err(Error::Malformed("varint too long"))
}

pub fn write_varint(mut value: u64, out: &mut Vec<u8>) {
    while value >= 0x80 {
        out.push((value as u8 & 0x7f) | 0x80);
        value >>= 7;
    }
    out.push(value as u8);
}

/// Splits a message into its fields, in order.
pub fn fields(mut bytes: &[u8]) -> Result<Vec<Field<'_>>, Error> {
    let mut out = Vec::new();
    while !bytes.is_empty() {
        if out.len() >= MAX_FIELDS {
            return Err(Error::Malformed("too many fields"));
        }
        let start = bytes;
        let tag = read_varint(&mut bytes)?;
        let number = u32::try_from(tag >> 3)
            .ok()
            .filter(|n| (1..=0x1fff_ffff).contains(n))
            .ok_or(Error::Malformed("bad field number"))?;
        let wire_type = (tag & 7) as u8;

        let value = match wire_type {
            0 => {
                let before = bytes;
                read_varint(&mut bytes)?;
                &before[..before.len() - bytes.len()]
            }
            1 | 2 | 5 => {
                let len = match wire_type {
                    1 => 8,
                    5 => 4,
                    _ => usize::try_from(read_varint(&mut bytes)?)
                        .map_err(|_| Error::Malformed("length too large"))?,
                };
                let value = bytes.get(..len).ok_or(Error::Malformed("truncated field"))?;
                bytes = &bytes[len..];
                value
            }
            _ => return Err(Error::Malformed("unsupported wire type")),
        };
        let raw = &start[..start.len() - bytes.len()];
        out.push(Field {
            number,
            wire_type,
            raw,
            value,
        });
    }
    Ok(out)
}

/// A varint field.
pub fn put_varint(number: u32, value: u64, out: &mut Vec<u8>) {
    write_varint(u64::from(number) << 3, out);
    write_varint(value, out);
}

/// A fixed64 field.
pub fn put_fixed64(number: u32, value: u64, out: &mut Vec<u8>) {
    write_varint(u64::from(number) << 3 | 1, out);
    out.extend_from_slice(&value.to_le_bytes());
}

/// A length-delimited field.
pub fn put_message(number: u32, payload: &[u8], out: &mut Vec<u8>) {
    write_varint(u64::from(number) << 3 | 2, out);
    write_varint(payload.len() as u64, out);
    out.extend_from_slice(payload);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn varints_round_trip() {
        for value in [0, 1, 127, 128, 300, u32::MAX as u64, u64::MAX] {
            let mut bytes = Vec::new();
            write_varint(value, &mut bytes);
            let mut slice = bytes.as_slice();
            assert_eq!(read_varint(&mut slice).unwrap(), value);
            assert!(slice.is_empty());
        }
    }

    #[test]
    fn fields_keep_their_raw_bytes() {
        let mut bytes = Vec::new();
        put_varint(3, 150, &mut bytes);
        put_message(5, b"hi", &mut bytes);

        let parsed = fields(&bytes).unwrap();
        assert_eq!(parsed.len(), 2);
        assert_eq!((parsed[0].number, parsed[0].varint().unwrap()), (3, 150));
        assert_eq!(parsed[1].message().unwrap(), b"hi");
        let joined: Vec<u8> = parsed.iter().flat_map(|f| f.raw.iter().copied()).collect();
        assert_eq!(joined, bytes);
    }

    #[test]
    fn garbage_is_rejected_not_trusted() {
        assert!(fields(&[0xff]).is_err());
        // Field 1, length 9, but only one byte follows.
        assert!(fields(&[0x0a, 0x09, 0x00]).is_err());
        // Field number zero.
        assert!(fields(&[0x00, 0x01]).is_err());
    }
}
