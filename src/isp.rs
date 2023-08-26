use hidapi::DeviceInfo;
use log::*;
use std::{thread, time};
use thiserror::Error;

use crate::VerificationError;

use super::part::*;
use super::util;

extern crate hidapi;

use hidapi::{HidApi, HidDevice};

pub struct ISPDevice<'a> {
    request_device: HidDevice,
    data_device: HidDevice,
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

#[derive(Debug, Error)]
pub enum ISPError {
    #[error("Duplicate devices found")]
    DuplicateDevices(String, String),
    #[error("Device not found")]
    NotFound,
}

impl ISPDevice<'static> {
    fn hidapi() -> HidApi {
        let api = HidApi::new().unwrap();

        #[cfg(target_os = "macos")]
        api.set_open_exclusive(false); // macOS will throw a privilege violation error otherwise

        return api;
    }

    pub fn new(part: &'static Part) -> Result<Self, ISPError> {
        let (request_device, data_device) = Self::find_isp_device(part)?;
        return Ok(Self {
            request_device: request_device,
            data_device: data_device,
            part: &part,
        });
    }

    fn fetch_kb_devices() -> Result<(HidDevice, HidDevice), ISPError> {
        let api = Self::hidapi();

        let mut request_device: Option<&DeviceInfo> = None;
        let mut data_device: Option<&DeviceInfo> = None;

        for device_info in api.device_list() {
            if !(device_info.vendor_id() == GAMING_KB_VENDOR_ID
                && device_info.product_id() == GAMING_KB_PRODUCT_ID)
            {
                continue;
            }

            if !(device_info.usage() == 1 && device_info.usage_page() == 0xff00) {
                continue;
            }

            let path = device_info.path();
            let path_str = path.to_str().unwrap();

            debug!("Found: {}", path_str);

            #[cfg(target_os = "windows")]
            {
                if path_str.contains("Col02") {
                    if let Some(request_device) = request_device {
                        return Err(ISPError::DuplicateDevices(
                            request_device.path().to_str().unwrap().to_owned(),
                            path_str.to_owned(),
                        ));
                    }
                    request_device = Some(device_info);
                    continue;
                }

                if path_str.contains("Col03") {
                    if let Some(data_device) = data_device {
                        return Err(ISPError::DuplicateDevices(
                            data_device.path().to_str().unwrap().to_owned(),
                            path_str.to_owned(),
                        ));
                    }
                    data_device = Some(device_info);
                    continue;
                }
            };

            #[cfg(not(target_os = "windows"))]
            {
                if let Some(request_device) = request_device {
                    if request_device.path() != path {
                        warn!("Duplicate device found. Only the first one will be used");
                    }
                    continue;
                } else {
                    request_device = Some(device_info);
                    data_device = Some(device_info);
                    continue;
                }
            };
        }

        if let (Some(request_device), Some(data_device)) = (request_device, data_device) {
            debug!(
                "Opening: req - {:?} / data - {:?}",
                request_device.path(),
                data_device.path()
            );

            return Ok((
                api.open_path(request_device.path()).unwrap(),
                api.open_path(data_device.path()).unwrap(),
            ));
        } else {
            return Err(ISPError::NotFound);
        }
    }

    fn open_isp_device() -> Result<(HidDevice, HidDevice), ISPError> {
        return Self::fetch_kb_devices();
    }

    fn switch_kb_device(part: &Part) -> Result<(HidDevice, HidDevice), ISPError> {
        let api = Self::hidapi();

        let request_device_info = api
            .device_list()
            .filter(|d| {
                d.vendor_id() == part.vendor_id
                    && d.product_id() == part.product_id
                    && d.interface_number() == 1
            })
            .filter(|d| {
                #[cfg(target_os = "windows")]
                {
                    return String::from_utf8_lossy(d.path().to_bytes())
                        .to_string()
                        .contains("Col05");
                }
                return true;
            })
            .next();

        let Some(request_device_info) = request_device_info else {
            info!("Device didn't come up...");
            return Err(ISPError::NotFound);
        };

        debug!("Opening: {:?}", request_device_info.path());

        let device = api.open_path(request_device_info.path()).unwrap();

        info!("Found Device. Entering ISP mode...");
        Self::enter_isp_mode(&device);

        info!("Waiting for bootloader device...");
        thread::sleep(time::Duration::from_millis(1000));

        let Ok(isp_device) = Self::open_isp_device() else {
            info!("ISP Device didn't come up...");
            return Err(ISPError::NotFound);
        };

        return Ok(isp_device);
    }

    fn find_isp_device(part: &Part) -> Result<(HidDevice, HidDevice), ISPError> {
        for attempt in 1..MAX_RETRIES + 1 {
            if attempt > 1 {
                info!("Retrying... Attempt {}/{}", attempt, MAX_RETRIES);
            }

            if let Ok(devices) = Self::switch_kb_device(part) {
                info!("Connected!");
                return Ok(devices);
            }
            info!("No KB found. Trying bootloader directly...");
            if let Ok(devices) = Self::open_isp_device() {
                info!("Connected!");
                return Ok(devices);
            }
        }
        return Err(ISPError::NotFound);
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
        self.data_device.get_feature_report(&mut xfer_buf).unwrap();
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
        self.data_device.send_feature_report(&xfer_buf).unwrap();
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
