use ihex::*;

pub trait IHexConversion {
    // fn from_ihex() -> Self;
    fn to_ihex(&self) -> Vec<Record>;
}

impl IHexConversion for Vec<u8> {
    fn to_ihex(&self) -> Vec<Record> {
        let mut result: Vec<Record> = vec![];
        for (i, chunk) in self.chunks(16).enumerate() {
            result.push(Record::Data {
                offset: (i as u16) * 16,
                value: chunk.to_vec(),
            });
        }
        result.push(Record::EndOfFile);
        return result;
    }
}

#[derive(Debug, Clone)]
pub enum VerificationError {
    ByteMismatch(usize, u8, u8),
    LengthMismatch(usize, usize),
}

impl VerificationError {
    pub fn to_message(&self) -> String {
        return match self {
            Self::LengthMismatch(expected, actual) => {
                format!("LENGTH MISMATCH {} {}", expected, actual)
            }
            Self::ByteMismatch(addr, expected, actual) => {
                format!(
                    "FIRMWARE MISMATCH @ 0x{:04x} --- {:02x} != {:02x}",
                    addr, expected, actual
                )
            }
        };
    }
}

pub fn verify(expected: &Vec<u8>, actual: &Vec<u8>) -> Result<(), VerificationError> {
    if expected.len() != actual.len() {
        return Err(VerificationError::LengthMismatch(
            expected.len(),
            actual.len(),
        ));
    }

    for i in 0..expected.len() {
        if expected[i] != actual[i] {
            return Err(VerificationError::ByteMismatch(i, expected[i], actual[i]));
        }
    }

    return Ok(());
}
