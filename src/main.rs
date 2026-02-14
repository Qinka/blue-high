#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;
use stm32f1xx_hal::{
    pac,
    prelude::*,
    i2c::{BlockingI2c, DutyCycle, Mode},
    serial::{Config, Serial},
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

use nb::block;

#[entry]
fn main() -> ! {
    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`. Configure for USB (48 MHz required)
    let clocks = rcc
        .cfgr
        .use_hse(8.MHz())
        .sysclk(48.MHz())
        .pclk1(24.MHz())
        .freeze(&mut flash.acr);

    // Acquire the GPIO and AFIO peripherals
    let mut gpiob = dp.GPIOB.split();
    let gpioa = dp.GPIOA.split();
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
    let usb_dm = gpioa.pa11;
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
    // E22-400M30S LoRa UART Setup (PA9/PA10)
    // ========================================
    // UART1 for LoRa communication
    let tx = gpioa.pa9.into_alternate_push_pull(&mut gpioa_crh);
    let rx = gpioa.pa10;
    
    let serial_lora = Serial::new(
        dp.USART1,
        (tx, rx),
        &mut afio.mapr,
        Config::default().baudrate(9600.bps()),
        &clocks,
    );
    
    let (mut tx_lora, mut rx_lora) = serial_lora.split();

    // Display status
    display.clear(BinaryColor::Off).unwrap();
    Text::with_baseline("USB-LoRa", Point::new(0, 0), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("Bridge Ready", Point::new(0, 12), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("9600 baud", Point::new(0, 24), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();
    
    delay.delay_ms(100_u32);

    // Main loop - transparent data bridge
    let mut usb_buf = [0u8; 64];
    let mut lora_buf = [0u8; 64];
    let mut byte_count: u32 = 0;
    
    loop {
        // Poll USB
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }
        
        // USB -> LoRa: Read from USB and send to LoRa
        match serial.read(&mut usb_buf) {
            Ok(count) if count > 0 => {
                // Send data to LoRa UART
                for i in 0..count {
                    block!(tx_lora.write(usb_buf[i])).ok();
                }
                byte_count = byte_count.wrapping_add(count as u32);
                
                // Update display
                display.clear(BinaryColor::Off).unwrap();
                Text::with_baseline("USB->LoRa", Point::new(0, 0), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                Text::with_baseline("TX OK", Point::new(0, 12), text_style, Baseline::Top)
                    .draw(&mut display)
                    .unwrap();
                display.flush().unwrap();
            }
            _ => {}
        }
        
        // LoRa -> USB: Read from LoRa and send to USB
        let mut lora_count = 0;
        for i in 0..64 {
            match rx_lora.read() {
                Ok(byte) => {
                    lora_buf[i] = byte;
                    lora_count += 1;
                }
                Err(_) => break,
            }
        }
        
        if lora_count > 0 {
            // Send data to USB
            let mut write_offset = 0;
            while write_offset < lora_count {
                match serial.write(&lora_buf[write_offset..lora_count]) {
                    Ok(len) => {
                        write_offset += len;
                    }
                    Err(_) => break,
                }
            }
            byte_count = byte_count.wrapping_add(lora_count as u32);
            
            // Update display
            display.clear(BinaryColor::Off).unwrap();
            Text::with_baseline("LoRa->USB", Point::new(0, 0), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
            Text::with_baseline("RX OK", Point::new(0, 12), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
            display.flush().unwrap();
        }
    }
}
