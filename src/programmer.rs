use rusb::*;
use log::*;
use std::{thread, time};

use super::part::*;
use super::util;
use super::hid;

pub struct Programmer<'a> {
    device: DeviceHandle<GlobalContext>,
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

#[derive(Debug, Clone)]
pub enum ReadType {
    Normal,
    Bootloader,
    Full
}

impl Programmer<'static> {
    pub fn new(part: &'static Part) -> Self {
        let device = Self::find_isp_device(part);
        return Self {
            device: device,
            part: &part,
        };
    }

    fn find_isp_device(part: &Part) -> DeviceHandle<GlobalContext> {
        for attempt in 1..MAX_RETRIES+1 {
            if attempt > 1 {
                info!("Retrying... Attempt {}/{}", attempt, MAX_RETRIES);
            }

            let kb_device_info = open_device_with_vid_pid(part.vendor_id, part.product_id);

            let Some(device_info) = kb_device_info else {
                info!("No KB found. Trying bootloader directly...");
                let device = open_device_with_vid_pid(GAMING_KB_VENDOR_ID, GAMING_KB_PRODUCT_ID).unwrap();
                info!("Connected!");
                return device;
            };

            info!("Found Device. Entering ISP mode...");
            Self::enter_isp_mode(&device_info);

            info!("Waiting for bootloader device...");
            thread::sleep(time::Duration::from_millis(1000));

            let kb_device_info = open_device_with_vid_pid(GAMING_KB_VENDOR_ID, GAMING_KB_PRODUCT_ID);

            let Some(_) = kb_device_info else {
                info!("Device didn't come up...");
                continue;
            };

            let device = open_device_with_vid_pid(GAMING_KB_VENDOR_ID, GAMING_KB_PRODUCT_ID).unwrap();
            info!("Connected!");

            return device;
        }
        panic!("Couldn't find ISP device");
    }

    fn enter_isp_mode(handle: &DeviceHandle<GlobalContext>) {
        let cmd: [u8; COMMAND_LENGTH] = [REPORT_ID_CMD, CMD_ISP_MODE, 0x00, 0x00, 0x00, 0x00];
        let _ = Self::send_feature_report(handle, &cmd); // ignore errors, many might be encountered here
    }

    fn get_feature_report(handle: &DeviceHandle<GlobalContext>, buf: &mut [u8]) -> Result<usize> {
        let report_number = buf[0] as u16;

        return handle.read_control(
            constants::LIBUSB_REQUEST_TYPE_CLASS | constants::LIBUSB_RECIPIENT_INTERFACE | constants::LIBUSB_ENDPOINT_IN,
            0x01,
            (3/*HID feature*/ << 8) | report_number,
            1, // fix
            buf,
            time::Duration::from_millis(1000)
        );
    }

    fn send_feature_report(handle: &DeviceHandle<GlobalContext>, buf: &[u8]) -> Result<usize> {
        let report_number = buf[0] as u16;

        return handle.write_control(
            constants::LIBUSB_REQUEST_TYPE_CLASS | constants::LIBUSB_RECIPIENT_INTERFACE | constants::LIBUSB_ENDPOINT_OUT,
            0x09,
            (3/*HID feature*/ << 8) | report_number,
            1, // fix
            buf,
            time::Duration::from_millis(1000)
        );
    }

    pub fn read_cycle(&self, read_type: ReadType) -> Vec<u8> {
        self.magic_sauce();

        return match read_type {
            ReadType::Normal => self.read(0, self.part.flash_size),
            ReadType::Bootloader => self.read(self.part.flash_size, self.part.bootloader_size),
            ReadType::Full => self.read(0, self.part.flash_size + self.part.bootloader_size)
        };
    }

    pub fn write_cycle(&self, firmware: &mut Vec<u8>) {
        let length = firmware.len();

        assert_eq!(
            self.part.flash_size, length,
            "Wrong firmware size. Expected {}, but got {}",
            self.part.flash_size, length
        );

        // this is a bit of an arcane part and I'm not certain why this happens
        firmware.copy_within(0..3, length - 5);
        firmware[(length - 5)..(length - 2)].fill(0);

        self.erase();
        self.write(&firmware);
        let written = self.read(0, self.part.flash_size);

        info!("Verifying...");
        match util::verify(&firmware, &written) {
            Err(e) => {
                error!("{}", e.to_message());
                return;
            }
            Ok(_) => {}
        };
        self.finalize();
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

        Self::send_feature_report(&self.device, &cmd).unwrap();
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
        Self::send_feature_report(&self.device, &cmd).unwrap();

        let page_size = self.part.page_size;
        let num_page = length / page_size;
        let mut result: Vec<u8> = vec![];
        for i in 0..num_page {
            debug!("Reading page {} @ offset {:#06x}", i, start_addr + i * page_size);
            self.read_page(&mut result);
        }
        return result;
    }

    fn read_page(&self, buf: &mut Vec<u8>) {
        let page_size = self.part.page_size;
        let mut xfer_buf: Vec<u8> = vec![0; page_size + 2];
        xfer_buf[0] = REPORT_ID_XFER;
        xfer_buf[1] = XFER_READ_PAGE;
        Self::get_feature_report(&self.device, &mut xfer_buf).unwrap();
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

        Self::send_feature_report(&self.device, &cmd).unwrap();

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
        Self::send_feature_report(&self.device, &xfer_buf).unwrap();
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
        Self::send_feature_report(&self.device, &cmd).unwrap();
        thread::sleep(time::Duration::from_millis(2000));
    }

    // TODO: verify what this command does and if it's actually needed
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
        Self::send_feature_report(&self.device, &cmd).unwrap();
    }
}
