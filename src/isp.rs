use hidapi::DeviceInfo;
use log::*;
use std::{thread, time};

use crate::VerificationError;

use super::part::*;
use super::util;

extern crate hidapi;

use hidapi::{HidApi, HidDevice};

pub struct ISPDevice<'a> {
    request_device: HidDevice,
    read_device: HidDevice,
    write_device: HidDevice,
    part: &'a Part,
}

const MAX_RETRIES: usize = 10;

const GAMING_KB_VENDOR_ID: u16 = 0x0603;
const GAMING_KB_PRODUCT_ID: u16 = 0x1020;

const COMMAND_LENGTH: usize = 6;

const REPORT_ID_CMD: u8 = 0x05;
const REPORT_ID_XFER: u8 = 0x06;

const CMD_ISP_MODE: u8 = 0x75;
const CMD_MAGIC_SAUCE: u8 = 0x55; // unsure how this command works, hence the name
const CMD_INIT_READ: u8 = 0x52;
const CMD_INIT_WRITE: u8 = 0x57;
const CMD_ERASE: u8 = 0x45;

const XFER_READ_PAGE: u8 = 0x72;
const XFER_WRITE_PAGE: u8 = 0x77;

const LJMP_OPCODE: u8 = 0x02;

#[derive(Debug, Clone)]
pub enum ReadType {
    Normal,
    Bootloader,
    Full,
}

impl ISPDevice<'static> {
    pub fn new(part: &'static Part) -> Self {
        let (request_device, read_device, write_device) = Self::find_isp_device(part);
        return Self {
            request_device: request_device,
            read_device: read_device,
            write_device: write_device,
            part: &part,
        };
    }

    fn request_device_predicate(device_info: &&DeviceInfo) -> bool {
        if device_info.vendor_id() != GAMING_KB_VENDOR_ID || device_info.product_id() != GAMING_KB_PRODUCT_ID {
            return false;
        }
        #[cfg(target_os = "windows")] {
            return String::from_utf8_lossy(device_info.path().to_bytes()).to_string().contains("Col02");
        };
        return true;
    }

    fn read_device_predicate(device_info: &&DeviceInfo) -> bool {
        if device_info.vendor_id() != GAMING_KB_VENDOR_ID || device_info.product_id() != GAMING_KB_PRODUCT_ID {
            return false;
        }
        #[cfg(target_os = "windows")] {
            return String::from_utf8_lossy(device_info.path().to_bytes()).to_string().contains("Col03");
        };
        return true;
    }

    fn write_device_predicate(device_info: &&DeviceInfo) -> bool {
        if device_info.vendor_id() != GAMING_KB_VENDOR_ID || device_info.product_id() != GAMING_KB_PRODUCT_ID {
            return false;
        }
        #[cfg(target_os = "windows")] {
            return String::from_utf8_lossy(device_info.path().to_bytes()).to_string().contains("Col01");
        };
        return true;
    }

    fn open_isp_device() -> Option<(HidDevice, HidDevice, HidDevice)> {
        let api = HidApi::new().unwrap();

        #[cfg(target_os = "macos")]
        api.set_open_exclusive(false); // macOS will throw a privilege violation error otherwise

        let request_device_info = api.device_list()
            .filter(Self::request_device_predicate)
            .next();

        let Some(request_device_info) = request_device_info else {
            info!("Request Device didn't come up...");
            return None;
        };

        let read_device_info = api.device_list()
            .filter(Self::read_device_predicate)
            .next();

        let Some(read_device_info) = read_device_info else {
            info!("Read Device didn't come up...");
            return None;
        };

        let write_device_info = api.device_list()
            .filter(Self::write_device_predicate)
            .next();

        let Some(write_device_info) = write_device_info else {
            info!("Write Device didn't come up...");
            return None;
        };

        println!("Opening: {:?}", read_device_info.path());
        println!("Opening: {:?}", write_device_info.path());

        return Some((
            api.open_path(request_device_info.path()).unwrap(),
            api.open_path(read_device_info.path()).unwrap(),
            api.open_path(write_device_info.path()).unwrap()
        ));
    }

    fn switch_kb_device(part: &Part) -> Option<(HidDevice, HidDevice, HidDevice)> {
        let api = HidApi::new().unwrap();

        #[cfg(target_os = "macos")]
        api.set_open_exclusive(false); // macOS will throw a privilege violation error otherwise

        let device_info = api.device_list()
            .filter(|d| d.vendor_id() == part.vendor_id && d.product_id() == part.product_id && d.interface_number() == 1)
            .filter(|d| {
                #[cfg(target_os = "windows")] {
                    return String::from_utf8_lossy(d.path().to_bytes()).to_string().contains("Col05");
                }
                return true;
            })
            .next();

        let Some(device_info) = device_info else {
            info!("Device didn't come up...");
            return None;
        };

        println!("Opening: {:?}", device_info.path());

        let device = api.open_path(device_info.path()).unwrap();

        info!("Found Device. Entering ISP mode...");
        Self::enter_isp_mode(&device);

        info!("Waiting for bootloader device...");
        thread::sleep(time::Duration::from_millis(1000));

        let Some(isp_device) = Self::open_isp_device() else {
            info!("ISP Device didn't come up...");
            return None;
        };

        return Some(isp_device);
    }

    fn find_isp_device(part: &Part) -> (HidDevice, HidDevice, HidDevice) {
        for attempt in 1..MAX_RETRIES + 1 {
            if attempt > 1 {
                info!("Retrying... Attempt {}/{}", attempt, MAX_RETRIES);
            }

            if let Some(device) = Self::switch_kb_device(part) {
                info!("Connected!");
                return device;
            }
            info!("No KB found. Trying bootloader directly...");
            if let Some(device) = Self::open_isp_device() {
                info!("Connected!");
                return device;
            }
        }
        panic!("Couldn't find ISP device");
    }

    fn enter_isp_mode(handle: &HidDevice) {
        let cmd: [u8; COMMAND_LENGTH] = [REPORT_ID_CMD, CMD_ISP_MODE, 0x00, 0x00, 0x00, 0x00];
        let _ = handle.send_feature_report(&cmd); // ignore errors, many might be encountered here
    }

    pub fn read_cycle(&self, read_type: ReadType) -> Vec<u8> {
        self.magic_sauce();

        return match read_type {
            ReadType::Normal => self.read(0, self.part.flash_size),
            ReadType::Bootloader => self.read(self.part.flash_size, self.part.bootloader_size),
            ReadType::Full => self.read(0, self.part.flash_size + self.part.bootloader_size),
        };
    }

    pub fn write_cycle(&self, firmware: &mut Vec<u8>) -> Result<(), VerificationError> {
        let length = firmware.len();

        self.erase();
        self.write(&firmware);
        let written = self.read(0, self.part.flash_size);

        // ARCANE: the ISP will copy the LJMP instruction (if existing) from the end to the very start of memory.
        // We need to make modifications to the expected payload to account for this.
        if firmware[length - 5] == LJMP_OPCODE {
            firmware[0] = LJMP_OPCODE;
            firmware.copy_within((length - 4)..(length - 2), 1); // Copy LJMP address
            firmware[(length - 5)..(length - 2)].fill(0); // Cleanup
        }

        info!("Verifying...");
        util::verify(&firmware, &written)?;
        self.finalize();
        return Ok(());
    }

    pub fn erase_cycle(&self) {
        info!("Erasing...");
        self.erase();
        self.finalize();
    }

    /// Allows firmware to be read prior to erasing it
    fn magic_sauce(&self) {
        let cmd: [u8; COMMAND_LENGTH] = [
            REPORT_ID_CMD,
            CMD_MAGIC_SAUCE,
            0x00,
            0x00,
            (self.part.flash_size & 0xff) as u8,
            (self.part.flash_size >> 8) as u8,
        ];

        self.request_device.send_feature_report(&cmd).unwrap();
    }

    fn read(&self, start_addr: usize, length: usize) -> Vec<u8> {
        let cmd: [u8; COMMAND_LENGTH] = [
            REPORT_ID_CMD,
            CMD_INIT_READ,
            (start_addr & 0xff) as u8,
            (start_addr >> 8) as u8,
            (length & 0xff) as u8,
            (length >> 8) as u8,
        ];
        self.request_device.send_feature_report(&cmd).unwrap();

        let page_size = self.part.page_size;
        let num_page = length / page_size;
        let mut result: Vec<u8> = vec![];
        for i in 0..num_page {
            debug!(
                "Reading page {} @ offset {:#06x}",
                i,
                start_addr + i * page_size
            );
            self.read_page(&mut result);
        }
        return result;
    }

    fn read_page(&self, buf: &mut Vec<u8>) {
        let page_size = self.part.page_size;
        let mut xfer_buf: Vec<u8> = vec![0; page_size + 2];
        xfer_buf[0] = REPORT_ID_XFER;
        xfer_buf[1] = XFER_READ_PAGE;
        self.read_device.get_feature_report(&mut xfer_buf).unwrap();
        buf.extend_from_slice(&xfer_buf[2..(page_size + 2)]);
    }

    fn write(&self, buffer: &Vec<u8>) {
        info!("Writing...");
        let cmd: [u8; COMMAND_LENGTH] = [
            REPORT_ID_CMD,
            CMD_INIT_WRITE,
            0,
            0,
            (self.part.flash_size & 0xff) as u8,
            (self.part.flash_size >> 8) as u8,
        ];

        self.request_device.send_feature_report(&cmd).unwrap();

        let page_size = self.part.page_size;
        for i in 0..self.part.num_pages() {
            debug!("Writting page {} @ offset {:#06x}", i, i * page_size);
            self.write_page(&buffer[(i * page_size)..((i + 1) * page_size)]);
        }
    }

    fn write_page(&self, buf: &[u8]) {
        let page_size = self.part.page_size;
        let mut xfer_buf: Vec<u8> = vec![0; page_size + 2];
        xfer_buf[0] = REPORT_ID_XFER;
        xfer_buf[1] = XFER_WRITE_PAGE;
        xfer_buf[2..page_size + 2].clone_from_slice(&buf);
        self.write_device.send_feature_report(&xfer_buf).unwrap();
    }

    fn erase(&self) {
        info!("Erasing...");
        let cmd: [u8; COMMAND_LENGTH] = [
            REPORT_ID_CMD,
            CMD_ERASE,
            CMD_ERASE,
            CMD_ERASE,
            CMD_ERASE,
            CMD_ERASE,
        ];
        self.request_device.send_feature_report(&cmd).unwrap();
        thread::sleep(time::Duration::from_millis(2000));
    }

    fn finalize(&self) {
        info!("Finalizing...");
        let cmd: [u8; COMMAND_LENGTH] = [
            REPORT_ID_CMD,
            CMD_MAGIC_SAUCE,
            CMD_MAGIC_SAUCE,
            CMD_MAGIC_SAUCE,
            CMD_MAGIC_SAUCE,
            CMD_MAGIC_SAUCE,
        ];
        self.request_device.send_feature_report(&cmd).unwrap();
    }
}
