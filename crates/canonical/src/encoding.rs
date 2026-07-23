use crate::CanonicalError;

pub struct CanonicalEncoder {
    bytes: Vec<u8>,
}

impl CanonicalEncoder {
    pub const fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn finish(self) -> Vec<u8> {
        self.bytes
    }

    pub fn write_u8(&mut self, value: u8) {
        self.bytes.push(value);
    }

    pub fn write_bool(&mut self, value: bool) {
        self.write_u8(u8::from(value));
    }

    pub fn write_u32(&mut self, value: u32) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_u64(&mut self, value: u64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_i64(&mut self, value: i64) {
        self.bytes.extend_from_slice(&value.to_be_bytes());
    }

    pub fn write_bytes(&mut self, value: &[u8]) -> Result<(), CanonicalError> {
        self.write_length(value.len())?;
        self.bytes.extend_from_slice(value);
        Ok(())
    }

    pub fn write_str(&mut self, value: &str) -> Result<(), CanonicalError> {
        self.write_bytes(value.as_bytes())
    }

    pub fn write_optional_str(&mut self, value: Option<&str>) -> Result<(), CanonicalError> {
        match value {
            None => self.write_u8(0x00),
            Some(value) => {
                self.write_u8(0x01);
                self.write_str(value)?;
            }
        }
        Ok(())
    }

    pub fn write_collection<T>(
        &mut self,
        values: &[T],
        encode: fn(&mut Self, &T) -> Result<(), CanonicalError>,
    ) -> Result<(), CanonicalError> {
        self.write_length(values.len())?;
        for value in values {
            encode(self, value)?;
        }
        Ok(())
    }

    fn write_length(&mut self, length: usize) -> Result<(), CanonicalError> {
        let length = u32::try_from(length).map_err(|_| CanonicalError::LengthOverflow)?;
        self.write_u32(length);
        Ok(())
    }
}

impl Default for CanonicalEncoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::CanonicalEncoder;

    #[test]
    fn writes_signed_integer_boundaries_in_big_endian_twos_complement() {
        let cases = [
            (i64::MIN, [0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]),
            (-1, [0xff; 8]),
            (0, [0x00; 8]),
            (1, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
            (i64::MAX, [0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff]),
        ];

        for (value, expected) in cases {
            let mut encoder = CanonicalEncoder::new();
            encoder.write_i64(value);
            assert_eq!(encoder.finish(), expected);
        }
    }

    #[test]
    fn writes_unsigned_integer_boundaries_in_big_endian() {
        let cases = [
            (0, [0x00; 8]),
            (1, [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x01]),
            (
                1_u64 << 63,
                [0x80, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
            ),
            (u64::MAX, [0xff; 8]),
        ];

        for (value, expected) in cases {
            let mut encoder = CanonicalEncoder::new();
            encoder.write_u64(value);
            assert_eq!(encoder.finish(), expected);
        }
    }

    #[test]
    fn preserves_utf8_bytes_and_optional_presence() {
        let mut encoder = CanonicalEncoder::new();
        encoder.write_str("e\u{301}").unwrap();
        encoder.write_str("\u{e9}").unwrap();
        encoder.write_optional_str(None).unwrap();
        encoder.write_optional_str(Some("")).unwrap();

        assert_eq!(
            encoder.finish(),
            [
                0, 0, 0, 3, 0x65, 0xcc, 0x81, 0, 0, 0, 2, 0xc3, 0xa9, 0, 1, 0, 0, 0, 0,
            ]
        );
    }
}
