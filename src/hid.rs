use rusb::*;
use std::time;

pub struct HidDevice {
    handle: DeviceHandle<GlobalContext>
}

impl HidDevice {

    fn open(vendorId: u16, productId: u16) -> Option<HidDevice> {
        let Some(handle) = open_device_with_vid_pid(vendorId, productId) else {
            return None;
        };
        return Some(HidDevice { handle: handle });
    }

    fn get_feature_report(self: &Self, buf: &mut [u8]) -> Result<usize> {
        let report_number = buf[0] as u16;

        return self.handle.read_control(
            constants::LIBUSB_REQUEST_TYPE_CLASS | constants::LIBUSB_RECIPIENT_INTERFACE | constants::LIBUSB_ENDPOINT_IN,
            0x01,
            (3/*HID feature*/ << 8) | report_number,
            1, // fix
            buf,
            time::Duration::from_millis(1000)
        );
    }

    fn send_feature_report(self: &Self, buf: &[u8]) -> Result<usize> {
        let report_number = buf[0] as u16;

        return self.handle.write_control(
            constants::LIBUSB_REQUEST_TYPE_CLASS | constants::LIBUSB_RECIPIENT_INTERFACE | constants::LIBUSB_ENDPOINT_OUT,
            0x09,
            (3/*HID feature*/ << 8) | report_number,
            1, // fix
            buf,
            time::Duration::from_millis(1000)
        );
    }


}