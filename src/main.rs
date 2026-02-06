#![no_std]
#![no_main]

// Blue-High 调试基础设施 - 使用 RTT 输出
// use defmt_rtt as _;
use panic_probe as _;

// 使用 defmt 进行日志输出
use defmt::{self, info};

mod diagnostics;
use diagnostics::BlueHighDiagnostics as Diag;

mod lora_config;
use lora_config::{LoRaConfig, CURRENT_CONFIG};

mod sx1268_hal;
mod sx1268_driver;
use sx1268_hal::Sx1268Context;
use sx1268_driver::Sx1268Driver;

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

    rtt_target::rtt_init_defmt!();

    // 立即输出第一条日志 - 这应该总是工作
    defmt::info!("=== Blue-High 启动 ===");
    defmt::info!("版本: 0.1.0");
    defmt::info!("MCU: STM32F103C8T6");

    Diag::boot_sequence("STM32F103C8T6 初始化开始");

    // Get access to the device specific peripherals from the peripheral access crate
    let dp = pac::Peripherals::take().unwrap();

    // Take ownership over the raw flash and rcc devices and convert them into the corresponding
    // HAL structs
    let mut flash = dp.FLASH.constrain();
    let rcc = dp.RCC.constrain();

    // Freeze the configuration of all the clocks in the system and store the frozen frequencies in
    // `clocks`. Configure 72MHz system clock with USB support
    use stm32f1xx_hal::rcc::Config;
    let mut rcc = rcc.freeze(
        Config::hse(8.MHz())
            .sysclk(72.MHz())
            .pclk1(36.MHz()),
        &mut flash.acr,
    );

    Diag::clocks_configured(72, 36);

    // Acquire the GPIO and AFIO peripherals
    let mut gpiob = dp.GPIOB.split(&mut rcc);
    let mut gpioa = dp.GPIOA.split(&mut rcc);
    // AFIO is still initialized to enable alternate function remapping for peripherals
    let _afio = dp.AFIO.constrain(&mut rcc);

    // Create delay abstraction using TIM2
    let mut delay = dp.TIM2.delay_us(&mut rcc);

    // ========================================
    // OLED Display Setup (I2C2 on PB10/PB11)
    // ========================================
    Diag::oled_status("初始化 OLED 显示屏 (I2C2 @ PB10/PB11)");
    let i2c_scl = gpiob.pb10.into_alternate_open_drain(&mut gpiob.crh);
    let i2c_sda = gpiob.pb11.into_alternate_open_drain(&mut gpiob.crh);

    let i2c = BlockingI2c::new(
        dp.I2C2,
        (i2c_scl, i2c_sda),
        Mode::Fast {
            frequency: 400_000.Hz(),
            duty_cycle: DutyCycle::Ratio2to1,
        },
        &mut rcc,
        1000,
        10,
        1000,
        1000,
    );

    let interface = I2CDisplayInterface::new(i2c);
    let mut display = Ssd1306::new(interface, DisplaySize128x64, DisplayRotation::Rotate0)
        .into_buffered_graphics_mode();

    display.init().unwrap();

    Diag::oled_status("SSD1306 128x64 初始化完成");

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

    let mut usb_dev = UsbDeviceBuilder::new(&usb_bus, UsbVidPid(0x26c0, 0x27dd))
        .strings(&[usb_device::device::StringDescriptors::default()
            .manufacturer("Wareless Group")
            .product("Blue-High LoRa Cake")
            .serial_number("E22-400M30S-0001")])
        .unwrap()
        .device_class(USB_CLASS_CDC)
        .build();

    Diag::boot_sequence("USB CDC 虚拟串口已配置");

    // ========================================
    // E22-400M30S LoRa SPI Setup with SX1268 Driver
    // ========================================
    // The E22-400M30S uses SPI communication with SX1268 chip
    // SPI pins: SCK = PA5, MISO = PA6, MOSI = PA7
    // Control pins: NSS = PA4, BUSY = PA3, DIO1 = PA2, NRST = PA1
    // RF Switch: TXEN = PB0, RXEN = PB1 (based on typical E22 design)

    // SPI pins configuration
    let sck = gpioa.pa5.into_alternate_push_pull(&mut gpioa.crl);
    let miso = gpioa.pa6;
    let mosi = gpioa.pa7.into_alternate_push_pull(&mut gpioa.crl);

    // SX1268 Control pins
    let nss = gpioa.pa4.into_push_pull_output(&mut gpioa.crl);
    let busy = gpioa.pa3.into_floating_input(&mut gpioa.crl);
    let _dio1 = gpioa.pa2.into_floating_input(&mut gpioa.crl);
    let nrst = gpioa.pa1.into_push_pull_output(&mut gpioa.crl);
    
    // RF Switch control pins (TXEN/RXEN)
    // Note: E22 module may have internal RF switch control
    // If not exposed, these pins can be configured as dummy outputs
    let txen = gpiob.pb0.into_push_pull_output(&mut gpiob.crl);
    let rxen = gpiob.pb1.into_push_pull_output(&mut gpiob.crl);

    // Configure SPI1
    let spi = Spi::new(
        dp.SPI1,
        (Some(sck), Some(miso), Some(mosi)),
        SpiMode {
            polarity: Polarity::IdleLow,
            phase: Phase::CaptureOnFirstTransition,
        },
        1.MHz(),
        &mut rcc,
    );

    // Create SX1268 HAL context
    let sx1268_ctx = Sx1268Context::new(spi, nss, nrst, busy, txen, rxen);
    
    // Create SX1268 driver
    let mut sx1268 = Sx1268Driver::new(sx1268_ctx);

    Diag::boot_sequence("E22-400M30S SX1268 驱动创建完成");

    // ========================================
    // LoRa 配置加载和初始化
    // ========================================
    // 用户可以在 src/lora_config.rs 中修改 CURRENT_CONFIG 来改变 LoRa 参数
    let lora_cfg = CURRENT_CONFIG;

    // 打印配置信息到调试日志
    defmt::info!("╔══════════════════════════════════╗");
    defmt::info!("║      E22-400M30S LoRa 配置       ║");
    defmt::info!("╠══════════════════════════════════╣");
    defmt::info!("║ 频率: {} MHz ({}Hz)", lora_cfg.get_frequency_mhz(), lora_cfg.get_frequency_hz());
    defmt::info!("║ 功率: {} dBm (2级输出)", lora_cfg.get_power_dbm());
    defmt::info!("║       {} dBm (1级芯片)", lora_cfg.get_chip_power_dbm());
    defmt::info!("║ 带宽: {} kHz", lora_cfg.get_bandwidth_khz());
    defmt::info!("║ 扩频因子: SF{}", lora_cfg.get_sf());
    defmt::info!("║ 编码率: CR4/{}", lora_cfg.get_cr_ratio());
    defmt::info!("║ 前导码: {} 符号", lora_cfg.preamble_length);
    defmt::info!("║ CRC: {}", if lora_cfg.crc_enabled { "启用" } else { "禁用" });
    defmt::info!("║ 头部: {}", if lora_cfg.explicit_header { "显式" } else { "隐式" });
    defmt::info!("║ 同步字: 0x{:02X} ({})", lora_cfg.sync_word,
        if lora_cfg.sync_word == 0x12 { "公网" } else { "私网" });
    defmt::info!("║ PA 配置: duty={:02X} hp={:02X}",
        lora_cfg.pa_config.pa_duty_cycle, lora_cfg.pa_config.hp_max);
    defmt::info!("╚══════════════════════════════════╝");

    // 初始化 SX1268 芯片
    let mut delay_fn = |ms: u32| delay.delay_ms(ms);
    match sx1268.init(&lora_cfg, &mut delay_fn) {
        Ok(_) => {
            defmt::info!("[SX1268] ✅ 初始化成功");
            Diag::boot_sequence("SX1268 LoRa 芯片初始化完成");
        }
        Err(_) => {
            defmt::error!("[SX1268] ❌ 初始化失败");
            Diag::error_occurred("SX1268 初始化失败");
        }
    }

    // Display status with configuration
    display.clear(BinaryColor::Off).unwrap();
    Text::with_baseline("E22 LoRa", Point::new(0, 0), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    // 创建配置信息字符串
    use core::fmt::Write;
    let mut freq_str = heapless::String::<16>::new();
    let mut power_str = heapless::String::<16>::new();
    let mut bw_str = heapless::String::<16>::new();
    let mut sf_cr_str = heapless::String::<20>::new();

    write!(&mut freq_str, "{}MHz", lora_cfg.get_frequency_mhz()).ok();
    write!(&mut power_str, "{}dBm", lora_cfg.get_power_dbm()).ok();
    write!(&mut bw_str, "BW{}k", lora_cfg.get_bandwidth_khz()).ok();
    write!(&mut sf_cr_str, "SF{} CR4/{}", lora_cfg.get_sf(), lora_cfg.get_cr_ratio()).ok();

    Text::with_baseline(freq_str.as_str(), Point::new(0, 12), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline(power_str.as_str(), Point::new(60, 12), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline(bw_str.as_str(), Point::new(0, 24), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();
    Text::with_baseline(sf_cr_str.as_str(), Point::new(0, 36), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    // 显示网络类型
    let net_type = if lora_cfg.sync_word == 0x12 { "Public" } else { "Private" };
    Text::with_baseline(net_type, Point::new(0, 48), text_style, Baseline::Top)
        .draw(&mut display)
        .unwrap();

    display.flush().unwrap();

    delay.delay_ms(100_u32);

    Diag::boot_sequence("系统初始化完成，进入主循环");

    // Main loop - USB to LoRa bridge with SX1268 driver
    const BUFFER_SIZE: usize = 64;
    let mut usb_buf = [0u8; BUFFER_SIZE];
    let mut loop_counter: u32 = 0;
    
    // Create delay closure once outside the loop
    let mut delay_fn = |ms: u32| delay.delay_ms(ms);

    loop {
        loop_counter = loop_counter.wrapping_add(1);
        
        // Poll USB
        if !usb_dev.poll(&mut [&mut serial]) {
            continue;
        }

        // USB -> LoRa: Read from USB and send via SX1268
        match serial.read(&mut usb_buf) {
            Ok(count) if count > 0 => {
                Diag::usb_bridge_rx(count);

                // 显示接收到的 USB 数据详细内容（十六进制和 ASCII）
                Diag::usb_data_received(&usb_buf[0..count]);

                // 使用 SX1268 驱动发送 LoRa 数据
                defmt::info!("[主循环] 准备通过 LoRa 发送 {} 字节", count);
                
                match sx1268.transmit(&usb_buf[0..count], &mut delay_fn) {
                    Ok(_) => {
                        defmt::info!("[主循环] ✅ LoRa 发送成功");
                        
                        // Update display
                        display.clear(BinaryColor::Off).unwrap();
                        Text::with_baseline("USB->LoRa", Point::new(0, 0), text_style, Baseline::Top)
                            .draw(&mut display)
                            .unwrap();
                        Text::with_baseline("TX Success", Point::new(0, 12), text_style, Baseline::Top)
                            .draw(&mut display)
                            .unwrap();
                        
                        // 显示发送的字节数
                        use core::fmt::Write;
                        let mut bytes_str = heapless::String::<20>::new();
                        write!(&mut bytes_str, "{} bytes", count).ok();
                        Text::with_baseline(bytes_str.as_str(), Point::new(0, 24), text_style, Baseline::Top)
                            .draw(&mut display)
                            .unwrap();
                        
                        display.flush().unwrap();
                    }
                    Err(_) => {
                        defmt::error!("[主循环] ❌ LoRa 发送失败");
                        Diag::error_occurred("LoRa 发送失败");
                        
                        // Update display
                        display.clear(BinaryColor::Off).unwrap();
                        Text::with_baseline("LoRa TX", Point::new(0, 0), text_style, Baseline::Top)
                            .draw(&mut display)
                            .unwrap();
                        Text::with_baseline("Failed!", Point::new(0, 12), text_style, Baseline::Top)
                            .draw(&mut display)
                            .unwrap();
                        display.flush().unwrap();
                    }
                }
            }
            _ => {
                // 无数据，继续轮询
            }
        }

        // For SPI-based LoRa, data reception would require polling the module
        // or using DIO1 interrupt. This is a simplified transmit-only example.
    }
}
