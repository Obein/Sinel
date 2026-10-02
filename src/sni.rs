//! High-performance, panic-free TLS SNI (Server Name Indication) parser.
//!
//! This module inspects incoming TLS ClientHello records without decrypting them,
//! extracting the target hostname requested by the client according to RFC 5246 and RFC 6066.

use std::fmt;

/// Errors that can occur while parsing TLS ClientHello or inspecting records.
#[derive(Debug, PartialEq, Eq)]
pub enum SniError {
    /// Buffer does not yet contain enough bytes to parse the record header or payload.
    Incomplete { expected: usize, available: usize },
    /// The incoming packet is not a TLS Handshake record (ContentType != 0x16).
    NotHandshake,
    /// Handshake message type is not ClientHello (type != 0x01).
    NotClientHello,
    /// Malformed TLS packet structure (invalid lengths or offsets).
    MalformedPacket(&'static str),
    /// The ClientHello record does not include an SNI extension.
    NoSniFound,
    /// The SNI hostname is not valid UTF-8.
    InvalidUtf8,
}

impl fmt::Display for SniError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Incomplete { expected, available } => {
                write!(f, "Incomplete TLS record: need {expected} bytes, have {available}")
            }
            Self::NotHandshake => write!(f, "Not a TLS Handshake record (ContentType != 0x16)"),
            Self::NotClientHello => write!(f, "Not a TLS ClientHello message"),
            Self::MalformedPacket(reason) => write!(f, "Malformed TLS packet: {reason}"),
            Self::NoSniFound => write!(f, "No SNI extension found in ClientHello"),
            Self::InvalidUtf8 => write!(f, "SNI hostname contains invalid UTF-8 bytes"),
        }
    }
}

impl std::error::Error for SniError {}

/// Checks the TLS record header (5 bytes) to determine the full expected record length.
///
/// Returns `Ok(total_record_len)` where `total_record_len = 5 + payload_len`.
pub fn get_record_expected_length(buf: &[u8]) -> Result<usize, SniError> {
    if buf.len() < 5 {
        return Err(SniError::Incomplete {
            expected: 5,
            available: buf.len(),
        });
    }

    // Byte 0: ContentType (0x16 for Handshake)
    if buf[0] != 0x16 {
        return Err(SniError::NotHandshake);
    }

    // Bytes 3..5: Record payload length (u16 big-endian)
    let payload_len = u16::from_be_bytes([buf[3], buf[4]]) as usize;
    let total_len = 5 + payload_len;

    // Sanity check: TLS record payload maximum size is 16384 bytes (2^14) + padding
    if !(4..=18432).contains(&payload_len) {
        return Err(SniError::MalformedPacket("TLS record length out of standard bounds"));
    }

    Ok(total_len)
}

/// Parses the SNI hostname from a complete TLS ClientHello record buffer.
///
/// If successful, returns the lowercase hostname string.
pub fn parse_sni(buf: &[u8]) -> Result<String, SniError> {
    let expected_len = get_record_expected_length(buf)?;
    if buf.len() < expected_len {
        return Err(SniError::Incomplete {
            expected: expected_len,
            available: buf.len(),
        });
    }

    let mut pos = 5;

    // Handshake Header:
    // 0: HandshakeType (0x01 = ClientHello)
    // 1..4: Handshake Length (u24 big-endian)
    if pos + 4 > buf.len() {
        return Err(SniError::MalformedPacket("Truncated handshake header"));
    }
    if buf[pos] != 0x01 {
        return Err(SniError::NotClientHello);
    }

    let handshake_len = ((buf[pos + 1] as usize) << 16)
        | ((buf[pos + 2] as usize) << 8)
        | (buf[pos + 3] as usize);
    pos += 4;

    let handshake_end = pos + handshake_len;
    if handshake_end > buf.len() {
        return Err(SniError::MalformedPacket("ClientHello length exceeds record length"));
    }

    // ClientVersion (2 bytes) + Random (32 bytes)
    if pos + 34 > handshake_end {
        return Err(SniError::MalformedPacket("Truncated ClientVersion/Random"));
    }
    pos += 34;

    // SessionID: 1 byte length + N bytes
    if pos + 1 > handshake_end {
        return Err(SniError::MalformedPacket("Missing SessionID length"));
    }
    let session_id_len = buf[pos] as usize;
    pos += 1;
    if pos + session_id_len > handshake_end {
        return Err(SniError::MalformedPacket("Truncated SessionID"));
    }
    pos += session_id_len;

    // CipherSuites: 2 bytes length (u16) + M bytes
    if pos + 2 > handshake_end {
        return Err(SniError::MalformedPacket("Missing CipherSuites length"));
    }
    let cipher_suites_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2;
    if pos + cipher_suites_len > handshake_end {
        return Err(SniError::MalformedPacket("Truncated CipherSuites"));
    }
    pos += cipher_suites_len;

    // CompressionMethods: 1 byte length (u8) + K bytes
    if pos + 1 > handshake_end {
        return Err(SniError::MalformedPacket("Missing CompressionMethods length"));
    }
    let compression_len = buf[pos] as usize;
    pos += 1;
    if pos + compression_len > handshake_end {
        return Err(SniError::MalformedPacket("Truncated CompressionMethods"));
    }
    pos += compression_len;

    // Extensions: If there are remaining bytes in the handshake, extensions follow
    if pos >= handshake_end {
        return Err(SniError::NoSniFound);
    }

    if pos + 2 > handshake_end {
        return Err(SniError::MalformedPacket("Missing Extensions length"));
    }
    let extensions_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
    pos += 2;

    let extensions_end = pos + extensions_len;
    if extensions_end > handshake_end {
        return Err(SniError::MalformedPacket("Extensions length exceeds ClientHello"));
    }

    // Iterate through individual TLS extensions
    while pos + 4 <= extensions_end {
        let ext_type = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let ext_len = u16::from_be_bytes([buf[pos + 2], buf[pos + 3]]) as usize;
        pos += 4;

        let ext_end = pos + ext_len;
        if ext_end > extensions_end {
            return Err(SniError::MalformedPacket("Extension length out of bounds"));
        }

        // Extension 0x0000 = server_name (SNI, RFC 6066)
        if ext_type == 0 {
            if pos + 2 > ext_end {
                return Err(SniError::MalformedPacket("Truncated ServerNameList length"));
            }
            let list_len = u16::from_be_bytes([buf[pos], buf[pos + 1]]) as usize;
            let mut list_pos = pos + 2;
            let list_end = (list_pos + list_len).min(ext_end);

            while list_pos + 3 <= list_end {
                let name_type = buf[list_pos]; // 0 = host_name
                let name_len = u16::from_be_bytes([buf[list_pos + 1], buf[list_pos + 2]]) as usize;
                list_pos += 3;

                if list_pos + name_len <= list_end {
                    if name_type == 0 {
                        let name_bytes = &buf[list_pos..list_pos + name_len];
                        let hostname = std::str::from_utf8(name_bytes)
                            .map_err(|_| SniError::InvalidUtf8)?
                            .to_lowercase();
                        return Ok(hostname);
                    }
                    list_pos += name_len;
                } else {
                    return Err(SniError::MalformedPacket("Truncated HostName in SNI"));
                }
            }
        }

        pos = ext_end;
    }

    Err(SniError::NoSniFound)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Constructs a minimal valid TLS ClientHello with the given SNI hostname.
    fn make_test_client_hello(hostname: &str) -> Vec<u8> {
        let mut extensions = Vec::new();
        // SNI extension:
        let host_bytes = hostname.as_bytes();
        let name_len = host_bytes.len() as u16;
        let list_len = name_len + 3; // 1 byte type + 2 bytes len + name
        let ext_len = list_len + 2;  // 2 bytes list_len + list

        extensions.extend_from_slice(&0u16.to_be_bytes()); // ext type = 0
        extensions.extend_from_slice(&ext_len.to_be_bytes());
        extensions.extend_from_slice(&list_len.to_be_bytes());
        extensions.push(0); // host_name type
        extensions.extend_from_slice(&name_len.to_be_bytes());
        extensions.extend_from_slice(host_bytes);

        let mut ch_payload = Vec::new();
        ch_payload.extend_from_slice(&[0x03, 0x03]); // client version 1.2
        ch_payload.extend_from_slice(&[0xaa; 32]);   // random 32 bytes
        ch_payload.push(0x00);                       // session id len = 0
        ch_payload.extend_from_slice(&2u16.to_be_bytes()); // cipher suites len = 2
        ch_payload.extend_from_slice(&[0xc0, 0x2f]);       // 1 cipher suite
        ch_payload.push(0x01);                       // compression methods len = 1
        ch_payload.push(0x00);                       // null compression
        ch_payload.extend_from_slice(&(extensions.len() as u16).to_be_bytes());
        ch_payload.extend_from_slice(&extensions);

        let mut record = Vec::new();
        record.push(0x16); // Handshake
        record.extend_from_slice(&[0x03, 0x01]); // TLS 1.0 record version

        let ch_len = ch_payload.len();
        let handshake_len = ch_len;
        let record_payload_len = 4 + handshake_len;
        record.extend_from_slice(&(record_payload_len as u16).to_be_bytes());

        // Handshake header
        record.push(0x01); // ClientHello
        record.push(((handshake_len >> 16) & 0xff) as u8);
        record.push(((handshake_len >> 8) & 0xff) as u8);
        record.push((handshake_len & 0xff) as u8);
        record.extend_from_slice(&ch_payload);

        record
    }

    #[test]
    fn test_parse_valid_sni() {
        let packet = make_test_client_hello("chat.signal.org");
        let result = parse_sni(&packet);
        assert_eq!(result, Ok("chat.signal.org".to_string()));
    }

    #[test]
    fn test_parse_uppercase_sni_is_lowercased() {
        let packet = make_test_client_hello("STORAGE.SIGNAL.ORG");
        let result = parse_sni(&packet);
        assert_eq!(result, Ok("storage.signal.org".to_string()));
    }

    #[test]
    fn test_incomplete_record() {
        let packet = make_test_client_hello("chat.signal.org");
        // Truncate to less than record length
        let truncated = &packet[..packet.len() - 5];
        let result = parse_sni(truncated);
        assert!(matches!(result, Err(SniError::Incomplete { .. })));
    }

    #[test]
    fn test_not_handshake() {
        let mut packet = make_test_client_hello("chat.signal.org");
        packet[0] = 0x17; // Application Data
        let result = parse_sni(&packet);
        assert_eq!(result, Err(SniError::NotHandshake));
    }

    #[test]
    fn test_empty_buffer() {
        let result = parse_sni(&[]);
        assert!(matches!(result, Err(SniError::Incomplete { .. })));
    }
}
