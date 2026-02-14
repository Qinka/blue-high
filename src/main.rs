#![no_std]
#![no_main]

use panic_halt as _;

use cortex_m_rt::entry;
use stm32f1xx_hal::{
    pac,
    prelude::*,
    i2c::{BlockingI2c, DutyCycle, Mode},
    spi::{Spi, Mode as SpiMode, Phase, Polarity},
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
    // E22-400M30S LoRa Module Setup (SPI)
    // ========================================
    // The E22-400M30S uses SPI communication with SX1268 chip
    // SPI pins: SCK = PA5, MISO = PA6, MOSI = PA7
    // NSS/CS = PA4
    // BUSY = PA3
    // DIO1 = PA2 (for interrupt)
    // NRST = PA1 (reset pin)
    
    // SPI pins configuration
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6;
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);

    // Control pins
    let mut nss = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
    let _busy = gpioa.pa3.into_floating_input(&mut gpioa.crl);
    let _dio1 = gpioa.pa2.into_floating_input(&mut gpioa.crl);
    let mut nrst = gpioa.pa1.into_push_pull_output(&mut gpioa.crl);

    // Configure SPI1
    let _spi = Spi::spi1(
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
    
    // Reset sequence
    nrst.set_low();
    delay.delay_ms(10_u32);
    nrst.set_high();
    delay.delay_ms(10_u32);

    // Note: Full SX1268 driver integration would be added here
    // For now, we demonstrate the SPI interface is properly configured

    // Display LoRa status
    display.clear(BinaryColor::Off).unwrap();
    Text::with_baseline("E22-400M30S", Point::new(0, 0), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("SX1268 SPI", Point::new(0, 12), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline("Ready!", Point::new(0, 24), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    display.flush().unwrap();
    
    delay.delay_ms(100_u32);

    // Main loop - update display periodically
    let mut counter: u32 = 0;
    const COUNTER_LABELS: [&str; 10] = [
        "Count: 0", "Count: 1", "Count: 2", "Count: 3", "Count: 4",
        "Count: 5", "Count: 6", "Count: 7", "Count: 8", "Count: 9"
    ];
    
    loop {
        // In a full implementation, LoRa transmission would occur here
        // using the SX1268 driver over SPI
        
        // Update display with counter
        display.clear(BinaryColor::Off).unwrap();
        Text::with_baseline("E22-400M30S", Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        Text::with_baseline("SPI Ready", Point::new(0, 12), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
        
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
