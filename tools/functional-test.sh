#!/usr/bin/env bash

set -euo pipefail

# TOOL=sinowealth-kb-tool
TOOL="cargo run --"
FILE_PREFIX="private/test-$(date +'%Y%m%dT%H%M%S')"

function reboot_device () {
    echo "Turning off port..."
    uhubctl -a off -p 1 -l 65-1
    sleep 1
    echo "Turning on port..."
    uhubctl -a on -p 1 -l 65-1
    echo "Waiting..."
    sleep 5
}

reboot_device

$TOOL read --part nuphy-air60 "$FILE_PREFIX-read.hex"

reboot_device

$TOOL read \
    --flash_size 61440 \
    --bootloader_size 4096 \
    --page_size 2048 \
    --vendor_id 0x05ac \
    --product_id 0x024f \
    "$FILE_PREFIX-read-custom.hex"

reboot_device

$TOOL read \
    --part nuphy-air60 \
    --vendor_id 0x05ac \
    --product_id 0x024f \
    "$FILE_PREFIX-read-override.hex"

reboot_device

READ_MD5=($(md5sum "$FILE_PREFIX-read.hex"))
READ_CUSTOM_MD5=($(md5sum "$FILE_PREFIX-read-custom.hex"))
READ_OVERRIDE_MD5=($(md5sum $FILE_PREFIX-read-override.hex))

if [[ "$READ_MD5" != "$READ_CUSTOM_MD5" ]]; then
    echo "MD5 mismatch $READ_MD5 != $READ_CUSTOM_MD5"
    exit 1
fi

if [[ "$READ_MD5" != "$READ_OVERRIDE_MD5" ]]; then
    echo "MD5 mismatch $READ_MD5 != $READ_OVERRIDE_MD5"
    exit 1
fi

$TOOL write --part nuphy-air60 "$FILE_PREFIX-read.hex"

reboot_device

$TOOL read --part nuphy-air60 "$FILE_PREFIX-post-write.hex"

READ_POST_WRITE_MD5=($(md5sum "$FILE_PREFIX-post-write.hex"))

if [[ "$READ_MD5" != "$READ_POST_WRITE_MD5" ]]; then
    echo "MD5 mismatch $READ_MD5 != $READ_POST_WRITE_MD5"
    exit 1
fi

echo "Passed all tests!"

reboot_device
