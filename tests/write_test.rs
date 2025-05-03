use std::fs;

use assert_cmd::Command;
use serial_test::serial;

#[macro_use]
mod common;

#[test]
#[serial]
fn test_write() {
    let file = "tests/fixtures/nuphy-air60_smk.hex";
    let mut cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = cmd
        .arg("write")
        .arg("--part").arg("nuphy-air60")
        .arg(file)
        .assert();
    assert.success();
}

#[test]
#[serial]
fn test_write_and_readback() {
    let fixture_file = "tests/fixtures/nuphy-air60_smk.hex";
    let mut write_cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = write_cmd
        .arg("write")
        .arg("--part").arg("nuphy-air60")
        .arg(&fixture_file)
        .assert();
    assert.success();

    let output_file = test_filename!("hex");
    let mut read_cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = read_cmd
        .arg("read")
        .arg("--part").arg("nuphy-air60")
        .arg(&output_file)
        .assert();
    assert.success()
        .stderr(predicates::str::contains("MD5: 662c8707c4be0e0712e30336b0e7cfd1"));

    let computed_md5 = md5::compute(fs::read(&output_file).unwrap());
    assert_eq!(format!("{:x}", computed_md5), "6594e5a1ab671deb40f36483a84ad61f");
}

#[test]
#[serial]
fn test_write_custom_and_readback() {
    let fixture_file = "tests/fixtures/nuphy-air60_smk.hex";
    let mut write_cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = write_cmd
        .arg("write")
        .arg("--vendor_id").arg("0x05ac")
        .arg("--product_id").arg("0x024f")
        .arg("--isp_iface_num").arg("1")
        .arg("--isp_report_id").arg("5")
        .arg("--firmware_size").arg("61440")
        .arg(&fixture_file)
        .assert();
    assert.success();

    let output_file = test_filename!("hex");
    let mut read_cmd = Command::cargo_bin("sinowealth-kb-tool").unwrap();
    let assert = read_cmd
        .arg("read")
        .arg("--vendor_id").arg("0x05ac")
        .arg("--product_id").arg("0x024f")
        .arg("--isp_iface_num").arg("1")
        .arg("--isp_report_id").arg("5")
        .arg("--firmware_size").arg("61440")
        .arg(&output_file)
        .assert();
    assert.success()
        .stderr(predicates::str::contains("MD5: 662c8707c4be0e0712e30336b0e7cfd1"));

    let computed_md5 = md5::compute(fs::read(&output_file).unwrap());
    assert_eq!(format!("{:x}", computed_md5), "6594e5a1ab671deb40f36483a84ad61f");
}
