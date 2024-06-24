use crate::part::Part;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq)]
pub enum VerificationError {
    #[error("Firmware Mismatch @ {addr:#06x} --- {expected:#04x} != {actual:#04x}")]
    ByteMismatch {
        addr: usize,
        expected: u8,
        actual: u8,
    },
    #[error("Length Mismatch {expected} {actual}")]
    LengthMismatch { expected: usize, actual: usize },
}

pub fn verify(expected: &[u8], actual: &[u8]) -> Result<(), VerificationError> {
    if expected.len() != actual.len() {
        return Err(VerificationError::LengthMismatch {
            expected: expected.len(),
            actual: actual.len(),
        });
    }

    for i in 0..expected.len() {
        if expected[i] != actual[i] {
            return Err(VerificationError::ByteMismatch {
                addr: i,
                expected: expected[i],
                actual: actual[i],
            });
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum PayloadConversionError {
    #[error("Expected LJMP not found at {addr:#06x}")]
    LJMPNotFoundError { addr: u16 },
    #[error("Unexpected addr at {source_addr:#06x} pointing to {target_addr:#06x}")]
    UnexpectedAddressError { source_addr: u16, target_addr: u16 },
}

pub fn convert_to_jtag_payload(
    input: &mut [u8],
    part: Part,
) -> Result<(), PayloadConversionError> {
    if input[0] != 0x02 {
        return Err(PayloadConversionError::LJMPNotFoundError { addr: 0x0000 });
    }

    let main_fw_address = u16::from_le_bytes(input[1..2].try_into().unwrap());
    if main_fw_address > 0xefff {
        return Err(PayloadConversionError::UnexpectedAddressError {
            source_addr: 0x0001,
            target_addr: main_fw_address,
        });
    }

    let bootloader_ljmp_addr = part.firmware_size;
    let ljmp_addr = part.firmware_size - 5;

    input[1..2].copy_from_slice(&bootloader_ljmp_addr.to_le_bytes());
    input[ljmp_addr] = 0x02;
    input[ljmp_addr + 1..ljmp_addr + 2].copy_from_slice(&main_fw_address.to_le_bytes());

    Ok(())
}

pub fn convert_to_isp_payload(
    input: &mut [u8],
    part: Part,
) -> Result<(), PayloadConversionError> {
    if input[0] != 0x02 {
        return Err(PayloadConversionError::LJMPNotFoundError { addr: 0 });
    }

    let ljmp_addr = part.firmware_size - 5;
    if input[ljmp_addr] != 0x02 {
        return Err(PayloadConversionError::LJMPNotFoundError { addr: 0x0000 });
    }

    let main_fw_address =
        u16::from_le_bytes(input[ljmp_addr + 1..ljmp_addr + 2].try_into().unwrap());
    if main_fw_address > 0xefff {
        return Err(PayloadConversionError::UnexpectedAddressError {
            source_addr: (ljmp_addr + 1) as u16,
            target_addr: main_fw_address,
        });
    }

    input[1..2].copy_from_slice(&main_fw_address.to_le_bytes());
    input[ljmp_addr..ljmp_addr + 2].fill(0x00);

    Ok(())
}

#[test]
fn test_verify_success() {
    assert!(verify(&vec![1, 2, 3, 4], &vec![1, 2, 3, 4]).is_ok());
}

#[test]
fn test_verify_error_length_mismatch() {
    assert_eq!(
        verify(&vec![1, 2, 3, 4], &vec![1, 2, 3]),
        Err(VerificationError::LengthMismatch {
            expected: 4,
            actual: 3
        })
    );
}

#[test]
fn test_verify_error_byte_mismatch() {
    assert_eq!(
        verify(&vec![1, 2, 3, 4], &vec![1, 2, 4, 3]),
        Err(VerificationError::ByteMismatch {
            addr: 2,
            expected: 3,
            actual: 4
        })
    );
}
