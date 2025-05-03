use std::fs;

use assert_cmd::Command;
use serial_test::serial;

#[macro_use]
mod common;

#[test]
#[serial]
fn test_read() {
    let file = test_filename!("hex");
    let mut cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = cmd
        .arg("read")
        .arg("--part").arg("nuphy-air60")
        .arg(&file)
        .assert();
    assert.success()
        .stderr(predicates::str::contains("MD5: 662c8707c4be0e0712e30336b0e7cfd1"));

    let computed_md5 = md5::compute(fs::read(&file).unwrap());
    assert_eq!(format!("{:x}", computed_md5), "6594e5a1ab671deb40f36483a84ad61f");
}

#[test]
#[serial]
fn test_read_bin() {
    let file = test_filename!("bin");
    let mut cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = cmd
        .arg("read")
        .arg("--part").arg("nuphy-air60")
        .arg(&file)
        .assert();
    assert.success()
        .stderr(predicates::str::contains("MD5: 662c8707c4be0e0712e30336b0e7cfd1"));

    let computed_md5 = md5::compute(fs::read(&file).unwrap());
    assert_eq!(format!("{:x}", computed_md5), "662c8707c4be0e0712e30336b0e7cfd1");
}

#[test]
#[serial]
fn test_read_bootloader() {
    let file = test_filename!("hex");
    let mut cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = cmd
        .arg("read")
        .arg("--part").arg("nuphy-air60")
        .arg("--section").arg("bootloader")
        .arg(&file)
        .assert();
    assert.success()
        .stderr(predicates::str::contains("MD5: 3e0ebd0c440af5236d7ff8872343f85d"));

    let computed_md5 = md5::compute(fs::read(&file).unwrap());
    assert_eq!(format!("{:x}", computed_md5), "65956adbe2e77369d3581ebabb1592f7");
}

#[test]
#[serial]
fn test_read_full() {
    let file = test_filename!("hex");
    let mut cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = cmd
        .arg("read")
        .arg("--part").arg("nuphy-air60")
        .arg("--section").arg("full")
        .arg(&file)
        .assert();
    assert.success()
        .stderr(predicates::str::contains("MD5: 3f87adb2125bcc5beee424a3af5272e9"));

    let computed_md5 = md5::compute(fs::read(&file).unwrap());
    assert_eq!(format!("{:x}", computed_md5), "e6f497f108dbe82f8d340562eb88fe44");
}

#[test]
#[serial]
fn test_read_custom_part() {
    let file = test_filename!("hex");
    let mut cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = cmd
        .arg("read")
        .arg("--vendor_id").arg("0x05ac")
        .arg("--product_id").arg("0x024f")
        .arg("--isp_iface_num").arg("1")
        .arg("--isp_report_id").arg("5")
        .arg("--firmware_size").arg("61440")
        .arg(&file)
        .assert();
    assert.success()
        .stderr(predicates::str::contains("MD5: 662c8707c4be0e0712e30336b0e7cfd1"));

    let computed_md5 = md5::compute(fs::read(&file).unwrap());
    assert_eq!(format!("{:x}", computed_md5), "6594e5a1ab671deb40f36483a84ad61f");
}
