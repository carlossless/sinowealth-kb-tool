use std::{
    fs,
    io::{self, Read},
    process::ExitCode,
};

use clap::*;
use log::*;
use simple_logger::SimpleLogger;
use thiserror::Error;

mod isp;
mod part;
// mod hid;
mod ihex;
mod util;

// pub use crate::hid::*;
pub use crate::{ihex::*, isp::*, part::*, util::*};

#[derive(Debug, Error)]
pub enum CLIError {
    #[error(transparent)]
    IOError(#[from] io::Error),
    #[error(transparent)]
    ISPError(#[from] ISPError),
    #[error(transparent)]
    IHEXError(#[from] ConversionError),
}

const PART_CUSTOM: &str = "custom";

fn cli() -> Command {
    return Command::new("sinowealth-kb-tool")
        .about("A programming tool for Sinowealth Gaming KB devices")
        .version(env!("CARGO_PKG_VERSION"))
        .subcommand_required(true)
        .arg_required_else_help(true)
        .author("Karolis Stasaitis")
        .subcommand(
            Command::new("read")
                .short_flag('r')
                .about("Read flash contents. (Intel HEX)")
                .arg(arg!(output_file: <OUTPUT_FILE> "file to write flash contents to"))
                .arg(
                    arg!(-p --part <PART>)
                        .value_parser(PARTS.keys().copied().collect::<Vec<_>>())
                        .required(true),
                )
                .arg(arg!(-b --bootloader "read only booloader").conflicts_with("full"))
                .arg(
                    arg!(-f --full "read complete flash (including the bootloader)")
                        .conflicts_with("bootloader"),
                ),
        )
        .subcommand(
            Command::new("write")
                .short_flag('w')
                .about("Write file (Intel HEX) into flash.")
                .arg(arg!(input_file: <INPUT_FILE> "payload to write into flash"))
                .arg(
                    arg!(-p --part <PART>)
                        .value_parser(available_part_options())
                        .required_unless_present_all([
                            "flash_size",
                            "bootloader_size",
                            "page_size",
                            "vendor_id",
                            "product_id",
                        ]),
                )
                .arg(arg!(--flash_size <SIZE>).required_if_eq("part", PART_CUSTOM))
                .arg(arg!(--bootloader_size <SIZE>).required_if_eq("part", PART_CUSTOM))
                .arg(arg!(--page_size <SIZE>).required_if_eq("part", PART_CUSTOM))
                .arg(arg!(--vendor_id <VID>).required_if_eq("part", PART_CUSTOM))
                .arg(arg!(--product_id <PID>).required_if_eq("part", PART_CUSTOM)),
        );
}

fn available_part_options() -> Vec<&'static str> {
    let mut parts = Part::available_parts();
    parts.push(PART_CUSTOM);
    parts
}

fn err_main() -> Result<(), CLIError> {
    SimpleLogger::new().init().unwrap();

    let matches = cli().get_matches();

    match matches.subcommand() {
        Some(("read", sub_matches)) => {
            let part_name = sub_matches
                .get_one::<String>("part")
                .map(|s| s.as_str())
                .unwrap();

            let output_file = sub_matches
                .get_one::<String>("output_file")
                .map(|s| s.as_str())
                .unwrap();

            let full = sub_matches.get_flag("full");

            let bootloader = sub_matches.get_flag("bootloader");

            let part = *PARTS.get(part_name).unwrap();

            let read_type = match (full, bootloader) {
                (true, _) => ReadType::Full,
                (_, true) => ReadType::Bootloader,
                _ => ReadType::Normal,
            };

            let isp = ISPDevice::new(part).map_err(CLIError::from)?;
            let result = isp.read_cycle(read_type).map_err(CLIError::from)?;

            let digest = md5::compute(&result);
            info!("MD5: {:x}", digest);

            let ihex = to_ihex(result).map_err(CLIError::from)?;
            fs::write(output_file, ihex).map_err(CLIError::from)?;
        }
        Some(("write", sub_matches)) => {
            let input_file = sub_matches
                .get_one::<String>("input_file")
                .map(|s| s.as_str())
                .unwrap();

            let part_name = sub_matches
                .get_one::<String>("part")
                .map(|s| s.as_str())
                .unwrap();

            let mut part = if part_name != "custom" {
                *PARTS.get(part_name).unwrap()
            } else {
                Part::default()
            };

            let flash_size = sub_matches.get_one::<usize>("flash_size");
            let bootloader_size = sub_matches.get_one::<usize>("bootloader_size");
            let page_size = sub_matches.get_one::<usize>("page_size");
            let vendor_id = sub_matches.get_one::<u16>("vendor_id");
            let product_id = sub_matches.get_one::<u16>("product_id");

            if let Some(flash_size) = flash_size {
                part.flash_size = *flash_size;
            }
            if let Some(bootloader_size) = bootloader_size {
                part.bootloader_size = *bootloader_size;
            }
            if let Some(page_size) = page_size {
                part.page_size = *page_size;
            }
            if let Some(vendor_id) = vendor_id {
                part.vendor_id = *vendor_id;
            }
            if let Some(product_id) = product_id {
                part.product_id = *product_id;
            }

            let mut file = fs::File::open(input_file).map_err(CLIError::from)?;
            let mut file_buf = Vec::new();
            file.read_to_end(&mut file_buf).map_err(CLIError::from)?;
            let file_str = String::from_utf8_lossy(&file_buf[..]);
            let mut firmware = from_ihex(&file_str, part.flash_size).map_err(CLIError::from)?;

            if firmware.len() < part.flash_size {
                firmware.resize(part.flash_size, 0);
            }

            let isp = ISPDevice::new(part).map_err(CLIError::from)?;
            isp.write_cycle(&mut firmware).map_err(CLIError::from)?;
        }
        Some(("erase", sub_matches)) => {
            let part_name = sub_matches
                .get_one::<String>("part")
                .map(|s| s.as_str())
                .unwrap();

            let part = *PARTS.get(part_name).unwrap();

            let isp = ISPDevice::new(part).map_err(CLIError::from)?;
            isp.erase_cycle().map_err(CLIError::from)?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn main() -> ExitCode {
    match err_main() {
        Ok(_) => ExitCode::SUCCESS,
        Err(e) => {
            error!("{}", e.to_string());
            ExitCode::FAILURE
        }
    }
}
