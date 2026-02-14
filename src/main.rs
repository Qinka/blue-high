#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;
use stm32f1xx_hal::{
    pac,
    prelude::*,
    i2c::{BlockingI2c, DutyCycle, Mode},
    serial::{Config, Serial},
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
    // `clocks`
    let clocks = rcc.cfgr.freeze(&mut flash.acr);

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
    // E22-400M30S LoRa Module Setup (UART)
    // ========================================
    // The E22-400M30S uses UART communication
    // UART1: TX = PA9, RX = PA10
    // M0 and M1 pins for mode control (can use PA2, PA3)
    // AUX pin for status monitoring (optional, can use PA4)
    
    // Configure mode control pins
    // M0 = PA2, M1 = PA3
    // Mode 0 (M0=0, M1=0): Normal/Transmission mode
    // Mode 1 (M0=1, M1=0): Wake-up mode  
    // Mode 2 (M0=0, M1=1): Power-saving mode
    // Mode 3 (M0=1, M1=1): Sleep/Configuration mode
    let mut m0 = gpioa.pa2.into_push_pull_output(&mut gpioa.crl);
    let mut m1 = gpioa.pa3.into_push_pull_output(&mut gpioa.crl);
    
    // Set to Normal mode (M0=0, M1=0) for transmission
    m0.set_low();
    m1.set_low();
    
    // Configure UART1 (PA9=TX, PA10=RX)
    let tx = gpioa.pa9.into_alternate_push_pull(&mut gpioa.crh);
    let rx = gpioa.pa10;
    
    // Initialize UART with 9600 baud (default for E22 modules)
    let serial = Serial::new(
        dp.USART1,
        (tx, rx),
        &mut afio.mapr,
        Config::default().baudrate(9600.bps()),
        &clocks,
    );
    
    let (mut tx_uart, mut rx_uart) = serial.split();

    // Display LoRa status
    display.clear(BinaryColor::Off).unwrap();
    Text::with_baseline("E22-400M30S", Point::new(0, 0), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("LoRa Ready!", Point::new(0, 12), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("UART @ 9600", Point::new(0, 24), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();
    
    delay.delay_ms(100_u32);

    // Main loop - transmit LoRa and update display periodically
    let mut counter: u32 = 0;
    const COUNTER_LABELS: [&str; 10] = [
        "Count: 0", "Count: 1", "Count: 2", "Count: 3", "Count: 4",
        "Count: 5", "Count: 6", "Count: 7", "Count: 8", "Count: 9"
    ];
    
    loop {
        // Transmit a message via LoRa (E22 module in transparent transmission mode)
        let message = b"Hello E22 LoRa!";
        
        // Send message byte by byte via UART
        for &byte in message.iter() {
            block!(tx_uart.write(byte)).ok();
        }
        
        // Try to read any incoming data (non-blocking)
        let mut rx_count = 0;
        for _ in 0..64 {
            match rx_uart.read() {
                Ok(_byte) => {
                    rx_count += 1;
                }
                Err(_) => break,
            }
        }

        // Update display with counter
        display.clear(BinaryColor::Off).unwrap();
        Text::with_baseline("E22-400M30S", Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        
        if rx_count > 0 {
            Text::with_baseline("RX Data OK", Point::new(0, 12), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
        } else {
            Text::with_baseline("TX OK", Point::new(0, 12), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
        }
        
        // Display counter using lookup table
        let counter_text = COUNTER_LABELS[(counter % 10) as usize];
        Text::with_baseline(counter_text, Point::new(0, 24), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        display.flush().unwrap();

        counter = counter.wrapping_add(1);
        
        // Delay 1 second
        delay.delay_ms(1000_u32);
    }
}
