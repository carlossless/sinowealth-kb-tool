use core::time;
use std::{ffi::CStr, thread};

use hidapi::{BusType, DeviceInfo, HidDevice, HidError, MAX_REPORT_DESCRIPTOR_SIZE};
use hidparser::parse_report_descriptor;
use itertools::Itertools;
use log::{debug, info};

const REPORT_ID_ISP: u8 = 0x05;
const CMD_ISP_MODE: u8 = 0x75;

const GAMING_KB_VENDOR_ID: u16 = 0x0603;
const GAMING_KB_PRODUCT_ID: u16 = 0x1020;
const GAMING_KB_V2_PRODUCT_ID: u16 = 0x1021;

const COMMAND_LENGTH: usize = 6;

const MAX_RETRIES: usize = 5; // TODO: move to an arg

#[cfg(not(target_os = "linux"))]
const HID_ISP_USAGE_PAGE: u16 = 0xff00;
#[cfg(not(target_os = "linux"))]
const HID_ISP_USAGE: u16 = 0x0001;

use crate::{to_hex_string, ISPDevice, ISPError, Part}; // TODO: Create own error here

pub struct DeviceSelector {
    api: hidapi::HidApi,
}

impl DeviceSelector {
    pub fn new() -> Result<Self, String> {
        let api = hidapi::HidApi::new().map_err(|e| e.to_string())?;

        #[cfg(target_os = "macos")]
        api.set_open_exclusive(false); // macOS will throw a privilege violation error otherwise

        Ok(Self { api })
    }

    fn sorted_usb_device_list(&self) -> Vec<&DeviceInfo> {
        let mut devices: Vec<_> = self
            .api
            .device_list()
            .filter(|d| d.bus_type() as u32 == BusType::Usb as u32)
            .collect();
        devices.sort_by_key(|d| {
            #[cfg(not(target_os = "linux"))]
            return (
                d.vendor_id(),
                d.product_id(),
                d.path(),
                d.interface_number(),
                d.usage_page(),
                d.usage(),
            );
            #[cfg(target_os = "linux")]
            return (
                d.vendor_id(),
                d.product_id(),
                d.path(),
                d.interface_number(),
            );
        });
        devices
    }

    fn get_feature_report_ids_from_path(&self, path: &CStr) -> Result<Vec<u32>, ISPError> {
        let dev = self.api.open_path(path).map_err(ISPError::from)?;
        self.get_feature_report_ids_from_device(dev)
    }

    fn get_feature_report_ids_from_device(&self, dev: HidDevice) -> Result<Vec<u32>, ISPError> {
        let mut buf: [u8; MAX_REPORT_DESCRIPTOR_SIZE] = [0; MAX_REPORT_DESCRIPTOR_SIZE];
        let size: usize = dev
            .get_report_descriptor(&mut buf)
            .map_err(ISPError::from)?;
        let report_descriptor =
            parse_report_descriptor(&buf[..size]).map_err(ISPError::ReportDescriptorError)?;
        let res = report_descriptor
            .features
            .iter()
            .filter_map(|item| item.report_id)
            .map(|report_id| report_id.into())
            .collect();
        Ok(res)
    }

    fn get_device_for_report_id<'a, I: IntoIterator<Item = &'a DeviceInfo>>(
        &self,
        devices: I,
        report_id: u32,
    ) -> Result<&'a DeviceInfo, ISPError> {
        for d in devices {
            let ids = self.get_feature_report_ids_from_path(d.path())?;
            for id in ids {
                if id == report_id {
                    return Ok(d);
                }
            }
        }
        Err(ISPError::NotFound)
    }

    fn open_isp_devices(&self, part: Part) -> Result<ISPDevice, ISPError> {
        let sorted_devices = self.sorted_usb_device_list();
        let isp_devices: Vec<_> = sorted_devices
            .clone()
            .into_iter()
            .filter(|d| {
                #[cfg(not(target_os = "linux"))]
                return d.vendor_id() == GAMING_KB_VENDOR_ID
                    && matches!(
                        d.product_id(),
                        GAMING_KB_PRODUCT_ID | GAMING_KB_V2_PRODUCT_ID
                    )
                    && d.interface_number() == 0
                    && d.usage_page() == HID_ISP_USAGE_PAGE
                    && d.usage() == HID_ISP_USAGE;
                #[cfg(target_os = "linux")]
                return d.vendor_id() == GAMING_KB_VENDOR_ID
                    && matches!(
                        d.product_id(),
                        GAMING_KB_PRODUCT_ID | GAMING_KB_V2_PRODUCT_ID
                    )
                    && d.interface_number() == 0;
            })
            .collect();

        for d in &isp_devices {
            #[cfg(not(target_os = "linux"))]
            debug!(
                "Found ISP Device: {:#06x} {:#06x} {:?} {} {:#06x} {:#06x}",
                d.vendor_id(),
                d.product_id(),
                d.path(),
                d.interface_number(),
                d.usage_page(),
                d.usage()
            );
            #[cfg(target_os = "linux")]
            debug!(
                "Found ISP Device: {:#06x} {:#06x} {:?}",
                d.vendor_id(),
                d.product_id(),
                d.path()
            );
        }

        let device_count = isp_devices.len();
        if device_count == 0 {
            return Err(ISPError::NotFound);
        }

        let s = isp_devices.clone();
        let cmd_device = self.get_device_for_report_id(s, REPORT_ID_ISP as u32)?;
        debug!("CMD device: {:?}", cmd_device.path());
        #[cfg(not(target_os = "windows"))]
        return Ok(ISPDevice::new(
            part,
            self.api.open_path(cmd_device.path()).unwrap(),
        ));

        #[cfg(target_os = "windows")]
        {
            let xfer_device =
                api.get_device_for_report_id(isp_devices.clone(), REPORT_ID_XFER as u32)?;
            debug!("XFER device: {:?}", xfer_device.path());
            return Ok(ISPDevice {
                request: api.open_path(cmd_device.path()).unwrap(),
                data: api.open_path(xfer_device.path()).unwrap(),
            });
        }
    }

    fn switch_kb_device(&mut self, part: Part) -> Result<ISPDevice, ISPError> {
        info!(
            "Looking for vId:{:#06x} pId:{:#06x}",
            part.vendor_id, part.product_id
        );

        let filtered_devices = self.sorted_usb_device_list().into_iter().filter(|d| {
            #[cfg(not(target_os = "linux"))]
            return d.vendor_id() == part.vendor_id
                && d.product_id() == part.product_id
                && d.interface_number() == part.isp_iface_num as i32
                && d.usage_page() == part.isp_usage_page
                && d.usage() == part.isp_usage;
            #[cfg(target_os = "linux")]
            return d.vendor_id() == part.vendor_id
                && d.product_id() == part.product_id
                && d.interface_number() == part.isp_iface_num as i32;
        });

        let mut cmd_device_info: Option<&DeviceInfo> = None;
        for d in filtered_devices {
            #[cfg(not(target_os = "linux"))]
            debug!(
                "Found Device: {:?} {} {:#06x} {:#06x}",
                d.path(),
                d.interface_number(),
                d.usage_page(),
                d.usage()
            );
            #[cfg(target_os = "linux")]
            debug!("Found Device: {:?} {}", d.path(), d.interface_number());

            let ids = self
                .get_feature_report_ids_from_path(d.path())
                .map_err(|_| ISPError::NotFound)?;
            for id in ids {
                if id == part.isp_report_id {
                    cmd_device_info = Some(d);
                }
            }
        }

        let Some(cmd_device_info) = cmd_device_info else {
            info!("Regular device didn't come up...");
            return Err(ISPError::NotFound);
        };

        debug!("Opening: {:?}", cmd_device_info.path());
        let device = self
            .api
            .open_path(cmd_device_info.path())
            .map_err(ISPError::from)?;

        info!("Found regular device. Entering ISP mode...");
        if let Err(err) = self.enter_isp_mode(&device) {
            match err {
                ISPError::HidError(HidError::HidApiError { ref message }) => {
                    match message.as_str() {
                        #[cfg(target_os = "macos")]
                        "IOHIDDeviceSetReport failed: (0xE0005000) unknown error code" => true,
                        #[cfg(target_os = "linux")]
                        "hid_error is not implemented yet" => true,
                        _ => {
                            // this often fails so we ignore the error
                            debug!("Error: {}", err);
                            info!("Waiting...");
                            thread::sleep(time::Duration::from_secs(2));
                            return Err(err);
                        }
                    }
                }
                err => {
                    // this often fails so we ignore the error
                    debug!("Error: {:}", err);
                    info!("Waiting...");
                    thread::sleep(time::Duration::from_secs(2));
                    return Err(err);
                }
            };
        }

        info!("Waiting for ISP device...");
        thread::sleep(time::Duration::from_secs(2));

        self.api.refresh_devices()?;

        let Ok(isp_device) = self.open_isp_devices(part) else {
            info!("ISP device didn't come up...");
            return Err(ISPError::NotFound);
        };
        Ok(isp_device)
    }

    pub fn find_isp_device(&mut self, part: Part) -> Result<ISPDevice, ISPError> {
        self.find_isp_device_retry(part, MAX_RETRIES)
    }

    fn find_isp_device_retry(&mut self, part: Part, retries: usize) -> Result<ISPDevice, ISPError> {
        for attempt in 1..retries + 1 {
            self.api.refresh_devices()?;
            if attempt > 1 {
                thread::sleep(time::Duration::from_millis(500));
                info!("Retrying... Attempt {}/{}", attempt, retries);
            }

            if let Ok(devices) = self.switch_kb_device(part) {
                info!("Connected!");
                return Ok(devices);
            }
            info!("Regular device not found. Trying ISP device...");
            if let Ok(devices) = self.open_isp_devices(part) {
                info!("Connected!");
                return Ok(devices);
            }
        }
        Err(ISPError::NotFound)
    }

    fn enter_isp_mode(&self, handle: &HidDevice) -> Result<(), ISPError> {
        let cmd: [u8; COMMAND_LENGTH] = [REPORT_ID_ISP, CMD_ISP_MODE, 0x00, 0x00, 0x00, 0x00];
        handle.send_feature_report(&cmd)?;
        Ok(())
    }

    /// Prints out all connected HID devices and their paths.
    pub fn print_connected_devices(&self, with_report_descriptor: bool) -> Result<(), ISPError> {
        info!("Listing all connected HID devices...");
        let devices: Vec<_> = self.sorted_usb_device_list();

        let id_chunks = devices.iter().chunk_by(|d| {
            return (
                d.vendor_id(),
                d.product_id(),
                d.manufacturer_string().unwrap_or("None"),
                d.product_string().unwrap_or("None"),
            );
        });

        for ((vid, pid, manufacturer, product), devices) in &id_chunks {
            info!(
                "ID {:04x}:{:04x}: manufacturer=\"{:}\" product=\"{:}\"",
                vid, pid, manufacturer, product
            );

            let path_chunks = devices.chunk_by(|d| (d.path(), d.interface_number()));

            for ((path, interface_number), devices) in &path_chunks {
                info!(
                    "  path=\"{}\" interface_number={}",
                    path.to_str().unwrap(),
                    interface_number
                );

                for d in devices {
                    #[cfg(not(target_os = "linux"))]
                    info!(
                        "    usage_page={:#06x} usage={:#06x}",
                        d.usage_page(),
                        d.usage()
                    );
                    #[cfg(target_os = "linux")]
                    info!("    interface_number={:#06x}", d.interface_number());
                }

                if let Ok(dev) = self.api.open_path(path) {
                    let mut buf: [u8; MAX_REPORT_DESCRIPTOR_SIZE] = [0; MAX_REPORT_DESCRIPTOR_SIZE];
                    if let Ok(size) = dev.get_report_descriptor(&mut buf) {
                        if with_report_descriptor {
                            info!("    report_descriptor={}", to_hex_string(&buf[..size]));
                        }
                        let rids: Vec<u32> = self.get_feature_report_ids_from_device(dev)?;
                        let r_string: Vec<String> =
                            rids.iter().map(|rid| format!("{:#04x}", rid)).collect();
                        if !r_string.is_empty() {
                            info!("    feature_report_ids={}", r_string.join(", "));
                        }
                    } else {
                        info!("    feature_report_ids=error");
                    }
                } else {
                    info!("    feature_report_ids=could not open {:?}", path);
                }
            }
        }

        info!("Found {} devices", devices.len());

        Ok(())
    }
}
