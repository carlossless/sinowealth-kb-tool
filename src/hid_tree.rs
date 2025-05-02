use crate::{to_hex_string, ISPError};

pub struct DeviceNode {
    pub product_id: u16,
    pub vendor_id: u16,
    pub product_string: String,
    pub manufacturer_string: String,
    pub children: Vec<InterfaceNode>,
}

pub struct InterfaceNode {
    pub interface_number: i32,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub path: String,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub descriptor: Result<Vec<u8>, ISPError>,
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    pub feature_report_ids: Result<Vec<u32>, ISPError>,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub children: Vec<ItemNode>,
}

pub struct ItemNode {
    #[cfg(target_os = "windows")]
    pub path: String,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub usage_page: u16,
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    pub usage: u16,
    #[cfg(target_os = "windows")]
    pub descriptor: Result<Vec<u8>, ISPError>,
    #[cfg(target_os = "windows")]
    pub feature_report_ids: Result<Vec<u32>, ISPError>,
}

const INDENT_SIZE: usize = 4;

pub trait TreeDisplay {
    fn to_tree_string(&self, level: usize) -> String;
}

impl TreeDisplay for Vec<DeviceNode> {
    fn to_tree_string(&self, _level: usize) -> String {
        let mut s = String::new();
        for device in self {
            s.push_str(&format!("{}\n", device.to_tree_string(0)));
        }
        s
    }
}

impl TreeDisplay for DeviceNode {
    fn to_tree_string(&self, level: usize) -> String {
        let indent = " ".repeat(INDENT_SIZE).repeat(level);
        let mut s = format!(
            "{indent}ID {:04x}:{:04x} manufacturer=\"{:}\" product=\"{:}\"\n",
            self.vendor_id, self.product_id, self.manufacturer_string, self.product_string
        );
        for child in &self.children {
            s.push_str(&child.to_tree_string(level + 1));
        }
        s
    }
}

impl TreeDisplay for InterfaceNode {
    fn to_tree_string(&self, level: usize) -> String {
        let indent = " ".repeat(INDENT_SIZE).repeat(level);
        let mut s = String::new();
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        s.push_str(&format!(
            "{indent}path=\"{}\" interface_number={}\n",
            self.path, self.interface_number
        ));
        #[cfg(target_os = "windows")]
        s.push_str(&format!(
            "{indent}interface_number={}\n",
            self.interface_number
        ));
        #[cfg(any(target_os = "macos", target_os = "linux"))]
        {
            let descriptor = self.descriptor.as_ref();
            match descriptor {
                Ok(descriptor) => {
                    s.push_str(&format!(
                        "{indent}report_descriptor=[{}]\n",
                        to_hex_string(&descriptor)
                    ));
                }
                Err(e) => {
                    s.push_str(&format!("{indent}report_descriptor=error: {}\n", e));
                }
            }
            let feature_report_ids = self.feature_report_ids.as_ref();
            match feature_report_ids {
                Ok(feature_report_ids) => {
                    s.push_str(&format!(
                        "{indent}feature_report_ids=[{}]\n",
                        feature_report_ids
                            .iter()
                            .map(|rid| format!("{:#04x}", rid))
                            .collect::<Vec<String>>()
                            .join(", ")
                    ));
                }
                Err(e) => {
                    s.push_str(&format!("{indent}feature_report_ids=error: {}\n", e));
                }
            }
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            for child in &self.children {
                s.push_str(&child.to_tree_string(level + 1));
            }
        }
        return s;
    }
}

impl TreeDisplay for ItemNode {
    fn to_tree_string(&self, level: usize) -> String {
        let indent = " ".repeat(INDENT_SIZE).repeat(level);
        let mut s = String::new();
        #[cfg(any(target_os = "macos"))]
        s.push_str(&format!(
            "{indent}usage_page={:#06x} usage={:#06x}\n",
            self.usage_page, self.usage
        ));
        #[cfg(any(target_os = "windows"))]
        {
            s.push_str(&format!(
                "{indent}path=\"{}\" usage_page={:#06x} usage={:#06x}\n",
                self.path, self.usage_page, self.usage
            ));
            let descriptor = self.descriptor.as_ref();
            match descriptor {
                Ok(descriptor) => {
                    s.push_str(&format!(
                        "{indent}report_descriptor=[{}]\n",
                        to_hex_string(&descriptor)
                    ));
                }
                Err(e) => {
                    s.push_str(&format!("{indent}report_descriptor=error: {}\n", e));
                }
            }
            let feature_report_ids = self.feature_report_ids.as_ref();
            match feature_report_ids {
                Ok(feature_report_ids) => {
                    s.push_str(&format!(
                        "{indent}feature_report_ids=[{}]\n",
                        feature_report_ids
                            .iter()
                            .map(|rid| format!("{:#04x}", rid))
                            .collect::<Vec<String>>()
                            .join(", ")
                    ));
                }
                Err(e) => {
                    s.push_str(&format!("{indent}feature_report_ids=error: {}\n", e));
                }
            }
        }
        s
    }
}
