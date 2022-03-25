#![no_main]
#![no_std]

use core::fmt::Write;
use core::str;

use cortex_m_rt::entry;
use panic_rtt_target as _;
use rtt_target::rtt_init_print;

#[cfg(feature = "v1")]
use microbit::{
    hal::prelude::*,
    hal::twi,
    hal::uart,
    hal::uart::{Baudrate, Parity},
    pac::twi0::frequency::FREQUENCY_A,
};

#[cfg(feature = "v2")]
use microbit::{
    hal::prelude::*,
    hal::twim,
    hal::uarte,
    hal::uarte::{Baudrate, Parity},
    pac::twim0::frequency::FREQUENCY_A,
};

use heapless::Vec;
use lsm303agr::{AccelOutputDataRate, Lsm303agr, MagOutputDataRate};

#[cfg(feature = "v2")]
mod serial_setup;
#[cfg(feature = "v2")]
use serial_setup::UartePort;

const ACCELEROMETER_ADDR: u8 = 0b0011001;
const MAGNETOMETER_ADDR: u8 = 0b0011110;

const ACCELEROMETER_ID_REG: u8 = 0x0f;
const MAGNETOMETER_ID_REG: u8 = 0x4f;

#[entry]
fn main() -> ! {
    rtt_init_print!();
    let board = microbit::Board::take().unwrap();

    #[cfg(feature = "v1")]
    let mut serial = {
        uart::Uart::new(
            board.UART0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        )
    };

    #[cfg(feature = "v2")]
    let mut serial = {
        let serial = uarte::Uarte::new(
            board.UARTE0,
            board.uart.into(),
            Parity::EXCLUDED,
            Baudrate::BAUD115200,
        );
        UartePort::new(serial)
    };

    #[cfg(feature = "v1")]
    let mut i2c = { twi::Twi::new(board.TWI0, board.i2c.into(), FREQUENCY_A::K100) };

    #[cfg(feature = "v2")]
    let mut i2c = { twim::Twim::new(board.TWIM0, board.i2c_internal.into(), FREQUENCY_A::K100) };

    let mut acc = [0];
    let mut mag = [0];

    // First write the address + register onto the bus, then read the chip's responses
    i2c.write_read(ACCELEROMETER_ADDR, &[ACCELEROMETER_ID_REG], &mut acc)
        .unwrap();
    i2c.write_read(MAGNETOMETER_ADDR, &[MAGNETOMETER_ID_REG], &mut mag)
        .unwrap();

    write!(serial, "Accelerometer chip id: {:#b}\r\n", acc[0]).unwrap();
    write!(serial, "Magnetometer chip id: {:#b}\r\n", mag[0]).unwrap();

    let mut sensor = Lsm303agr::new_with_i2c(i2c);
    sensor.init().unwrap();
    sensor.set_accel_odr(AccelOutputDataRate::Hz10).unwrap();
    sensor.set_mag_odr(MagOutputDataRate::Hz10).unwrap();
    let mut sensor = sensor.into_mag_continuous().ok().unwrap();

    let mut buffer: Vec<u8, 32> = Vec::new();

    loop {
        buffer.clear();
        write!(serial, "Command: accelerometer/magnetometer?\r\n").unwrap();

        loop {
            let byte = nb::block!(serial.read()).unwrap();

            if byte == b'\r' {
                break;
            }

            if buffer.push(byte).is_err() {
                write!(serial, "error: buffer full\r\n").unwrap();
                break;
            }
        }

        let command = str::from_utf8(&buffer).unwrap_or_default().trim();

        if command == "accelerometer" {
            while !sensor.accel_status().unwrap().xyz_new_data {}
            let data = sensor.accel_data().unwrap();
            write!(
                serial,
                "accelerometer: x {} y {} z {}\r\n",
                data.x, data.y, data.z
            )
            .unwrap();
        } else if command == "magnetometer" {
            while !sensor.mag_status().unwrap().xyz_new_data {}
            let data = sensor.mag_data().unwrap();
            write!(
                serial,
                "magnetometer: x {} y {} z {}\r\n",
                data.x, data.y, data.z
            )
            .unwrap();
        } else {
            write!(serial, "error: unsupported command '{}'\r\n", command).unwrap();
        }
    }
}
