#![no_std]
#![no_main]

// Blue-High 调试基础设施 - 使用 RTT 输出
// use defmt_rtt as _;
use panic_probe as _;

// 使用 defmt 进行日志输出
use defmt::{error, info};

mod diagnostics;
use diagnostics::BlueHighDiagnostics as Diag;

mod lora;

use sx1268_rs::{
  Sx1268, Sx1268Config,
  config::{
    CalibrationParams, FallbackMode, LoRaBandwidth, LoRaCodingRate, LoRaHeaderType,
    LoRaModulationParams, LoRaPacketParams, LoRaSpreadingFactor, PaConfig, PacketType, RampTime,
    RegulatorMode, SleepConfig, StandbyConfig, TcxoConfig, TcxoVoltage,
  },
  control::Control,
};

use cortex_m_rt::entry;
use stm32f1xx_hal::{
  i2c::{BlockingI2c, DutyCycle, Mode},
  pac,
  prelude::*,
  spi::{Mode as SpiMode, Phase, Polarity, Spi},
  usb::{Peripheral, UsbBus},
};

use embedded_graphics::{
  mono_font::{MonoTextStyleBuilder, ascii::FONT_6X10},
  pixelcolor::BinaryColor,
  prelude::*,
  text::{Baseline, Text},
};
use ssd1306::{I2CDisplayInterface, Ssd1306, prelude::*};

use usb_device::prelude::*;
use usbd_serial::{SerialPort, USB_CLASS_CDC};

use embedded_hal::spi::SpiDevice;

use crate::lora::LoraControl;

#[entry]
fn main() -> ! {
  rtt_target::rtt_init_defmt!();

  // 立即输出第一条日志 - 这应该总是工作
  info!("=== Blue-High 启动 ===");
  info!("版本: 0.1.0");
  info!("MCU: STM32F103C8T6");

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
    Config::hse(8.MHz()).sysclk(72.MHz()).pclk1(36.MHz()),
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
  let busy = gpiob.pb1.into_floating_input(&mut gpiob.crl);
  let nrst = gpiob.pb0.into_push_pull_output(&mut gpiob.crl);
  let _dio1 = gpioa.pa2.into_floating_input(&mut gpioa.crl);

  // RF Switch control pins (TXEN/RXEN)
  // Note: E22 module may have internal RF switch control
  // If not exposed, these pins can be configured as dummy outputs
  let txen = gpiob.pb12.into_push_pull_output(&mut gpiob.crh);
  let rxen = gpiob.pb13.into_push_pull_output(&mut gpiob.crh);

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

  // lora
  let mut lora = {
    // Controls
    let control = LoraControl {
      spi,
      nrst_pin: nrst,
      busy_pin: busy,
      cs_pin: nss,
      tx_pin: txen,
      rx_pin: rxen,
    };
    Sx1268::new(control)
  };
  // config
  let config = Sx1268Config::default()
    .with_package_lora()
    .with_frequency_hz(433_000_000)
    .expect("Invalid frequency")
    .with_pa_config(PaConfig::best_22dbm())
    .with_tx_power(20)
    .with_ramp_time(RampTime::Ramp40Us)
    .with_lora_modulation(
      LoRaModulationParams::default()
        .with_bandwidth(LoRaBandwidth::Bw500)
        .with_spreading_factor(LoRaSpreadingFactor::Sf11)
        .with_coding_rate(LoRaCodingRate::Cr4_5)
        .with_low_data_rate_optimize(true),
    )
    .with_lora_packet(
      LoRaPacketParams::default()
        .with_preamble_length(8)
        .with_header_type(LoRaHeaderType::Explicit)
        .with_payload_length(255)
        .with_crc_on(true)
        .with_invert_iq(false),
    )
    .with_regulator_mode(RegulatorMode::DcDcLdo)
    .with_lora_sync_word(0x1424)
    .with_tx_base_address(0x00)
    .with_rx_base_address(0x00)
    // .with_dio2_as_rf_switch(true)
    .with_fallback_mode(FallbackMode::StbyRc)
    .with_tcxo_config(TcxoVoltage::Ctrl3v3, 320)
    .with_calibration(CalibrationParams::ALL);

  lora
    .init(config.clone())
    .expect("SX1268 initialization failed");
  Diag::boot_sequence("E22-400M30S SX1268 驱动创建完成");

  // 打印配置信息到调试日志
  info!("╔══════════════════════════════════╗");
  info!("║      E22-400M30S LoRa 配置       ║");
  info!("╠══════════════════════════════════╣");
  info!("║ 频率: {}Hz", config.get_frequency_hz());
  info!("║ 功率: {} dBm", config.get_power_dbm());
  info!("║ 带宽: {} kHz", config.get_bandwidth_khz());
  info!("║ 扩频因子: SF{}", config.get_sf());
  info!(
    "║ 编码率: CR{}/{}",
    config.get_cr_ratio().0,
    config.get_cr_ratio().1
  );
  info!("║ 前导码: {} 符号", config.get_preamble_length());
  info!(
    "║ CRC: {}",
    if config.get_crc_enabled() {
      "启用"
    } else {
      "禁用"
    }
  );
  info!(
    "║ 头部: {}",
    if config.get_header_type() == LoRaHeaderType::Explicit {
      "显式"
    } else {
      "隐式"
    }
  );
  info!(
    "║ 同步字: 0x{:02X} ({})",
    config.get_sync_word(),
    if config.get_sync_word() == 0x14 {
      "公网"
    } else {
      "私网"
    }
  );
  info!(
    "║ PA 配置: duty={:02X} hp={:02X}",
    config.get_pa_duty_cycle(),
    config.get_pa_hp_max()
  );
  info!("╚══════════════════════════════════╝");

  lora
    .send_lora(&[1, 2, 3, 4, 5], 0)
    .expect("LoRa 发送测试失败");

  // 初始化 SX1268 芯片
  let mut delay_fn = |ms: u32| delay.delay_ms(ms);

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

  write!(&mut freq_str, "{}MHz", config.get_frequency_mhz()).ok();
  write!(&mut power_str, "{}dBm", config.get_power_dbm()).ok();
  write!(&mut bw_str, "BW{}k", config.get_bandwidth_khz()).ok();
  write!(
    &mut sf_cr_str,
    "SF{} CR{}/{}",
    config.get_sf(),
    config.get_cr_ratio().0,
    config.get_cr_ratio().1
  )
  .ok();

  Text::with_baseline(
    freq_str.as_str(),
    Point::new(0, 12),
    text_style,
    Baseline::Top,
  )
  .draw(&mut display)
  .unwrap();
  Text::with_baseline(
    power_str.as_str(),
    Point::new(60, 12),
    text_style,
    Baseline::Top,
  )
  .draw(&mut display)
  .unwrap();
  Text::with_baseline(
    bw_str.as_str(),
    Point::new(0, 24),
    text_style,
    Baseline::Top,
  )
  .draw(&mut display)
  .unwrap();
  Text::with_baseline(
    sf_cr_str.as_str(),
    Point::new(0, 36),
    text_style,
    Baseline::Top,
  )
  .draw(&mut display)
  .unwrap();

  // 显示网络类型
  let net_type = if config.get_sync_word() == 0x14 {
    "Public"
  } else {
    "Private"
  };
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
        info!("[主循环] 准备通过 LoRa 发送 {} 字节", count);

        match lora.send_lora(&usb_buf[0..count], 0) {
          Ok(_) => {
            info!("[主循环] ✅ LoRa 发送成功");

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
            Text::with_baseline(
              bytes_str.as_str(),
              Point::new(0, 24),
              text_style,
              Baseline::Top,
            )
            .draw(&mut display)
            .unwrap();

            display.flush().unwrap();
          }
          Err(_) => {
            error!("[主循环] ❌ LoRa 发送失败");
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
