// 该文件是 BlueHigh 项目的一部分。
// src/main.rs - 主程序入口
//
// 本文件根据 Apache 许可证第 2.0 版（以下简称“许可证”）授权使用；
// 除非遵守该许可证条款，否则您不得使用本文件。
// 您可通过以下网址获取许可证副本：
// http://www.apache.org/licenses/LICENSE-2.0
// 除非适用法律要求或书面同意，根据本许可协议分发的软件均按“原样”提供，
// 不附带任何形式的明示或暗示的保证或条件。
// 有关许可权限与限制的具体条款，请参阅本许可协议。
//
// Copyright (C) 2026 Johann Li <me@qinka.pro>, Wareless Group

#![no_std]
#![no_main]

use defmt::{error, info};
use panic_probe as _;

mod diagnostics;
use diagnostics::BlueHighDiagnostics as Diag;

mod lora;

use sx1268_rs::{
  Sx1268, Sx1268Config,
  config::{
    CalibrationParams, FallbackMode, LoRaBandwidth, LoRaCodingRate, LoRaHeaderType,
    LoRaModulationParams, LoRaPacketParams, LoRaSpreadingFactor, PaConfig, RampTime, RegulatorMode,
    TcxoVoltage,
  },
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

use crate::lora::LoraControl;

#[entry]
fn main() -> ! {
  rtt_target::rtt_init_defmt!();

  info!("=== Blue-High Boot ===");
  info!("Version: 0.1.0");
  info!("MCU: STM32F103C8T6");

  Diag::boot_sequence("STM32F103C8T6 init start");

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
  Diag::oled_status("I2C2 OLED init (PB10/PB11)");
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

  Diag::oled_status("SSD1306 128x64 ready");

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

  Diag::boot_sequence("USB CDC serial ready");

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
  // DIO1 signals RxDone / Timeout / error IRQs from the SX1268 (active high).
  let dio1 = gpioa.pa3.into_pull_up_input(&mut gpioa.crl);

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
  Diag::boot_sequence("E22-400M30S SX1268 driver ready");

  // 打印配置信息到调试日志
  info!("╔══════════════════════════════════╗");
  info!("║     E22-400M30S LoRa Config      ║");
  info!("╠══════════════════════════════════╣");
  info!("║ Freq : {}Hz", config.get_frequency_hz());
  info!("║ Power: {} dBm", config.get_power_dbm());
  info!("║ BW   : {} kHz", config.get_bandwidth_khz());
  info!("║ SF   : SF{}", config.get_sf());
  info!(
    "║ CR   : CR{}/{}",
    config.get_cr_ratio().0,
    config.get_cr_ratio().1
  );
  info!("║ Preamble: {} symbols", config.get_preamble_length());
  info!(
    "║ CRC  : {}",
    if config.get_crc_enabled() {
      "on"
    } else {
      "off"
    }
  );
  info!(
    "║ Header: {}",
    if config.get_header_type() == LoRaHeaderType::Explicit {
      "explicit"
    } else {
      "implicit"
    }
  );
  info!(
    "║ Sync : 0x{:04X} ({})",
    config.get_sync_word(),
    if config.get_sync_word() == 0x3444 {
      "public"
    } else {
      "private"
    }
  );
  info!(
    "║ PA   : duty={:02X} hp={:02X}",
    config.get_pa_duty_cycle(),
    config.get_pa_hp_max()
  );
  info!("╚══════════════════════════════════╝");

  // Send a startup test packet to verify the TX path.
  lora
    .send_lora(&[1, 2, 3, 4, 5], 0)
    .expect("LoRa startup TX failed");

  // Wait for TxDone — DIO1 goes high when transmission completes.
  {
    let mut tx_wait = 0u32;
    while !dio1.is_high() {
      tx_wait = tx_wait.wrapping_add(1);
      if tx_wait > 20_000_000 {
        break;
      }
    }
  }

  // Enter continuous RX mode (timeout = 0xFFFFFF → never times out).
  lora.start_lora_rx(0xFFFFFF).expect("LoRa start_rx failed");
  Diag::boot_sequence("LoRa entered continuous RX mode");

  // Display radio config on the OLED.
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
  let net_type = if config.get_sync_word() == 0x3444 {
    "Public"
  } else {
    "Private"
  };
  Text::with_baseline(net_type, Point::new(0, 48), text_style, Baseline::Top)
    .draw(&mut display)
    .unwrap();

  display.flush().unwrap();

  delay.delay_ms(100_u32);

  Diag::boot_sequence("System init complete, entering main loop");

  // Main loop — USB ↔ LoRa bridge backed by the SX1268 driver.
  const BUFFER_SIZE: usize = 64;
  let mut usb_buf = [0u8; BUFFER_SIZE];
  let mut rx_buf = [0u8; BUFFER_SIZE];
  let mut loop_counter: u32 = 0;

  loop {
    loop_counter = loop_counter.wrapping_add(1);

    // USB → LoRa: forward data received on the USB serial port to the radio.
    if usb_dev.poll(&mut [&mut serial]) {
      match serial.read(&mut usb_buf) {
        Ok(count) if count > 0 => {
          Diag::usb_bridge_rx(count);
          Diag::usb_data_received(&usb_buf[0..count]);
          info!("[main] Sending {} bytes via LoRa", count);

          match lora.send_lora(&usb_buf[0..count], 0) {
            Ok(_) => {
              info!("[main] LoRa TX ok");
              // Wait for TxDone — DIO1 goes high when transmission completes.
              let mut tx_wait = 0u32;
              while !dio1.is_high() {
                tx_wait = tx_wait.wrapping_add(1);
                if tx_wait > 20_000_000 {
                  break;
                }
              }

              // Update OLED display.
              display.clear(BinaryColor::Off).unwrap();
              Text::with_baseline("USB->LoRa", Point::new(0, 0), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
              Text::with_baseline("TX Success", Point::new(0, 12), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
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

              // Re-enter continuous RX after TX completes.
              lora.start_lora_rx(0xFFFFFF).ok();
            }
            Err(_) => {
              error!("[main] LoRa TX failed");
              Diag::error_occurred("LoRa TX failed");

              display.clear(BinaryColor::Off).unwrap();
              Text::with_baseline("LoRa TX", Point::new(0, 0), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
              Text::with_baseline("Failed!", Point::new(0, 12), text_style, Baseline::Top)
                .draw(&mut display)
                .unwrap();
              display.flush().unwrap();

              // Re-enter RX even after a TX error.
              lora.start_lora_rx(0xFFFFFF).ok();
            }
          }
        }
        _ => {}
      }
    }

    // LoRa → USB: forward received packets to the USB serial port.
    // DIO1 is high when the chip has raised an RxDone (or error) IRQ.
    if dio1.is_high() {
      let recv = lora.recv_lora(&mut rx_buf);
      match recv {
        Ok(Some(len)) => {
          info!("[main] LoRa RX {} bytes, forwarding to USB", len);
          info!("[main] RX hex: {:02X}", &rx_buf[..len]);
          if let Ok(s) = core::str::from_utf8(&rx_buf[..len]) {
            info!("[main] RX str: {}", s);
          } else {
            info!("[main] RX str: <non-UTF8>");
          }

          // Write received bytes to the USB CDC serial port.
          let mut written = 0;
          while written < len {
            match serial.write(&rx_buf[written..len]) {
              Ok(n) => written += n,
              Err(_) => break,
            }
          }
          serial.flush().ok();

          // Update OLED display.
          display.clear(BinaryColor::Off).unwrap();
          Text::with_baseline("LoRa->USB", Point::new(0, 0), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
          Text::with_baseline("RX OK", Point::new(0, 12), text_style, Baseline::Top)
            .draw(&mut display)
            .unwrap();
          let mut rx_len_str = heapless::String::<20>::new();
          write!(&mut rx_len_str, "{} bytes", len).ok();
          Text::with_baseline(
            rx_len_str.as_str(),
            Point::new(0, 24),
            text_style,
            Baseline::Top,
          )
          .draw(&mut display)
          .unwrap();
          display.flush().unwrap();
        }
        Ok(None) => {
          // DIO1 glitch — IRQ cleared with no data; ignore.
        }
        Err(_) => {
          error!("[main] LoRa RX error");
          Diag::error_occurred("LoRa RX error");
        }
      }
      // In continuous RX mode (0xFFFFFF) the chip auto-relistens after each
      // packet — do NOT call start_lora_rx here; it would reset the buffer
      // and corrupt subsequent packets.
    }

    // Diag::heartbeat(loop_counter);
  }
}
