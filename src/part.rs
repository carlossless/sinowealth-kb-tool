use phf::*;

pub struct Part {
    pub flash_size: usize,
    pub bootloader_size: usize,
    pub page_size: usize,
    pub vendor_id: u16,
    pub product_id: u16,
}

pub const PART_NUPHY_AIR60: Part = Part {
    flash_size: 61440, // 61440 until bootloader
    bootloader_size: 4096,
    page_size: 2048,
    vendor_id: 0x5ac,
    product_id: 0x024f,
};

pub static PARTS: phf::Map<&'static str, Part> = phf_map! {
    "nuphy-air60" => PART_NUPHY_AIR60
};

impl Part {
    pub fn num_pages(&self) -> usize {
        return self.flash_size / self.page_size;
    }
}
