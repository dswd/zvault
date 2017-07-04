pub fn to_hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{:02x}", b)).collect::<Vec<String>>().join("")
}

pub fn parse_hex(hex: &str) -> Result<Vec<u8>, ()> {
    let mut b = Vec::with_capacity(hex.len() / 2);
    let mut modulus = 0;
    let mut buf = 0;
    for (_, byte) in hex.bytes().enumerate() {
        buf <<= 4;
        match byte {
            b'A'...b'F' => buf |= byte - b'A' + 10,
            b'a'...b'f' => buf |= byte - b'a' + 10,
            b'0'...b'9' => buf |= byte - b'0',
            b' '|b'\r'|b'\n'|b'\t' => {
                buf >>= 4;
                continue
            }
            _ => return Err(()),
        }
        modulus += 1;
        if modulus == 2 {
            modulus = 0;
            b.push(buf);
        }
    }
    match modulus {
        0 => Ok(b.into_iter().collect()),
        _ => Err(()),
    }
}



mod tests {

    #[allow(unused_imports)]
    use super::*;


    #[test]
    fn test_to_hex() {
        assert_eq!(to_hex(&[0]), "00");
        assert_eq!(to_hex(&[1]), "01");
        assert_eq!(to_hex(&[15]), "0f");
        assert_eq!(to_hex(&[16]), "10");
        assert_eq!(to_hex(&[255]), "ff");
        assert_eq!(to_hex(&[5,255]), "05ff");
    }

    #[test]
    fn test_parse_hex() {
        assert_eq!(parse_hex("00"), Ok(vec![0]));
        assert_eq!(parse_hex("01"), Ok(vec![1]));
        assert_eq!(parse_hex("0f"), Ok(vec![15]));
        assert_eq!(parse_hex("0fff"), Ok(vec![15,255]));
        assert_eq!(parse_hex("0F"), Ok(vec![15]));
        assert_eq!(parse_hex("01 02\n03\t04"), Ok(vec![1,2,3,4]));
    }

}
