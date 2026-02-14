#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;
use stm32f1xx_hal::{
    pac,
    prelude::*,
    i2c::{BlockingI2c, DutyCycle, Mode},
    spi::{Spi, Mode as SpiMode, Phase, Polarity},
    usb::{Peripheral, UsbBus},
};

use ssd1306::{
    prelude::*,
    I2CDisplayInterface, Ssd1306,
};
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyleBuilder},
    pixelcolor::BinaryColor,
    prelude::*,
    text::{Baseline, Text},
};

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

#[entry]
fn main() -> ! {
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`. Configure 72MHz system clock with USB support
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(72.MHz())
        .pclk1(36.MHz())
        .freeze(&mut flash.acr);

    // Acquire the GPIO and AFIO peripherals
    let mut gpiob = dp.GPIOB.split();
    let mut gpioa = dp.GPIOA.split();
    let mut afio = dp.AFIO.constrain();
    
    // Create delay abstraction using TIM2
    let mut delay = dp.TIM2.delay_us(&clocks);

    // ========================================
    // OLED Display Setup (I2C on PB6/PB7)
    // ========================================
    let scl = gpiob.pb6.into_alternate_open_drain(&mut gpiob.crl);
    let sda = gpiob.pb7.into_alternate_open_drain(&mut gpiob.crl);

    let i2c = BlockingI2c::i2c1(
        dp.I2C1,
        (scl, sda),
        &mut afio.mapr,
        Mode::Fast {
            frequency: 400_000.Hz(),
            duty_cycle: DutyCycle::Ratio2to1,
        },
        clocks,
        1000,
        10,
        1000,
        1000,
    );

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();
    
    display.init().unwrap();

    // Create a text style
    let text_style = MonoTextStyleBuilder::new()
        .font(&FONT_6X10)
        .text_color(BinaryColor::On)
        .build();

    // Display "Hello OLED" on the screen
    display.clear(BinaryColor::Off).unwrap();
    Text::with_baseline("STM32F103C8T6", Point::new(0, 0), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("OLED Ready!", Point::new(0, 12), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();

    // ========================================
    // USB CDC Setup (PA11/PA12)
    // ========================================
    // Configure USB peripheral
    let mut gpioa_crh = gpioa.crh;
    let usb_dm = gpioa.pa11.into_floating_input(&mut gpioa_crh);
    let usb_dp = gpioa.pa12.into_floating_input(&mut gpioa_crh);
    
    let usb = Peripheral {
        usb: dp.USB,
        pin_dm: usb_dm,
        pin_dp: usb_dp,
    };
    
    let usb_bus = UsbBus::new(usb);
    
    let mut serial = SerialPort::new(&usb_bus);
    
    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x16c0, 0x27dd))
        .manufacturer("Qinka")
        .product("Blue-High LoRa Bridge")
        .serial_number("E22-001")
        .device_class(USB_CLASS_CDC)
        .build();

    // ========================================
    // E22-400M30S LoRa SPI Setup
    // ========================================
    // The E22-400M30S uses SPI communication with SX1268 chip
    // SPI pins: SCK = PA5, MISO = PA6, MOSI = PA7
    // Control pins: NSS = PA4, BUSY = PA3, DIO1 = PA2, NRST = PA1
    
    // SPI pins configuration
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6;
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);

    // Control pins
    let mut nss = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
    // Note: BUSY and DIO1 pins are available for future SX1268 driver integration
    // BUSY should be checked before SPI transactions
    // DIO1 can be used for interrupt-driven event handling
    let _busy = gpioa.pa3.into_floating_input(&mut gpioa.crl);
    let _dio1 = gpioa.pa2.into_floating_input(&mut gpioa.crl);
    let mut nrst = gpioa.pa1.into_push_pull_output(&mut gpioa.crl);

    // Configure SPI1
    let mut spi = Spi::spi1(
        dp.SPI1,
        (sck, miso, mosi),
        &mut afio.mapr,
        SpiMode {
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        },
        1.MHz(),
        clocks,
    );

    // Initialize E22-400M30S control pins
    nss.set_high(); // Deselect initially
    nrst.set_high(); // Keep module active
    
    // Reset sequence for SX1268
    nrst.set_low();
    delay.delay_ms(10_u32);
    nrst.set_high();
    delay.delay_ms(10_u32);

    // Display status
    display.clear(BinaryColor::Off).unwrap();
    Text::with_baseline("USB-LoRa SPI", Point::new(0, 0), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("E22 Ready", Point::new(0, 12), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("72MHz", Point::new(0, 24), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();
    
    delay.delay_ms(100_u32);

    // Main loop - USB to SPI bridge for LoRa control
    const BUFFER_SIZE: usize = 64;
    let mut usb_buf = [0u8; BUFFER_SIZE];
    
    loop {
        // Poll USB
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }
        
        // USB -> LoRa SPI: Read from USB and send to LoRa via SPI
        match serial.read(&mut usb_buf) {
            Ok(count) if count > 0 => {
                // Send data to LoRa via SPI (more efficient batch transfer)
                nss.set_low(); // Select chip
                
                // Transfer all bytes in one SPI transaction for efficiency
                if let Ok(_) = spi.transfer(&mut usb_buf[0..count]) {
                    // Data transferred successfully
                }
                
                nss.set_high(); // Deselect chip
                
                // Update display
                display.clear(BinaryColor::Off).unwrap();
                Text::with_baseline("USB->LoRa", Point::new(0, 0), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("SPI TX", Point::new(0, 12), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                display.flush().unwrap();
            }
            _ => {}
        }
        
        // For SPI-based LoRa, data reception would require polling the module
        // or using DIO1 interrupt. This is a simplified example showing USB control
    }
}
