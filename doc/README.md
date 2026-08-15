# Vendor documentation

The documents for this board are not kept in this repository. They belong to Yepkit and are
under no free license, unlike the code here — redistributing them would be a separate
question to ask them. What follows are the places to find them.

The hardware and protocol figures in the [README](../README.md) rest on these sources,
retrieved on 2026-08-14.

## YKUSH3

| Document | Where |
|---|---|
| Datasheet v1.2.1, January 2019 | https://www.yepkit.com/uploads/documents/9f39a_ykush3-datasheet.pdf |
| USB control interface, kept current | https://ykushboards.yepkit.com/docs/ykush3/reference/usb/ |
| I2C interface | https://ykushboards.yepkit.com/docs/ykush3/reference/i2c/ |
| Device overview | https://ykushboards.yepkit.com/docs/ykush3/reference/intro/ |
| Firmware update | https://ykushboards.yepkit.com/docs/ykush3/reference/firmware-update/ |
| Board dimensions v1.1 | https://www.yepkit.com/uploads/documents/13290_YKUSH3_Dimensions_v1.1.pdf |
| Hardware revision 1.3.0, August 2022 | https://www.yepkit.com/uploads/documents/6c2cf_YKUSH3%20Rev.1.3.0%20Release%20Notes.pdf |
| Product page with every download | https://www.yepkit.com/product/300110/YKUSH3 |

The 2019 datasheet and the online reference do not fully agree: persistent mode and the
external 5V port for `-c` appear in neither, yet both are present in firmware 1.5.0. See the
section on checking against the hardware in the README.

## YKUR

Relevant if the relay board is to be driven over the I2C bus of a YKUSH3.

| Document | Where |
|---|---|
| Datasheet rev. 1.2.1, March 2018 | https://www.yepkit.com/uploads/documents/bb69a_YKUR_datasheet_Rev.1.2.1.pdf |
| Product page | https://www.yepkit.com/product/300106/YKUR |
| The vendor's own control program | https://github.com/Yepkit/ykurcmd |

## The C++ original

| What | Where |
|---|---|
| The source this was ported from | https://github.com/Yepkit/ykush |
