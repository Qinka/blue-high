// 该文件是 BlueHigh 项目的一部分。
// src/diagnostics.rs - 诊断模块
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

//! Diagnostic / tracing helpers for the Blue-High firmware.
//!
//! All output is emitted through `defmt` and is only visible when a
//! probe-rs / RTT session is active.  The functions are thin wrappers so
//! that call-sites stay readable.

// Simple incrementing defmt timestamp (replace with a hardware timer for
// accurate timing).
defmt::timestamp!("{=u32:us}", {
  static mut TIMESTAMP: u32 = 0;
  unsafe {
    TIMESTAMP = TIMESTAMP.wrapping_add(1);
    TIMESTAMP
  }
});

pub struct BlueHighDiagnostics;

#[allow(dead_code)]
impl BlueHighDiagnostics {
  /// Emit a boot-sequence step message.
  pub fn boot_sequence(stage: &str) {
    defmt::println!("[boot] {}", stage);
  }

  /// Emit a clock-configuration summary.
  pub fn clocks_configured(sys_mhz: u32, apb1_mhz: u32) {
    defmt::println!("[clk] sys={}MHz apb1={}MHz", sys_mhz, apb1_mhz);
  }

  /// Emit an OLED status message.
  pub fn oled_status(message: &str) {
    defmt::println!("[oled] {}", message);
  }

  /// Emit a USB-RX byte count (USB → LoRa direction).
  pub fn usb_bridge_rx(byte_count: usize) {
    defmt::println!("[usb-rx] {} bytes", byte_count);
  }

  /// Dump received USB data as hex + printable ASCII.
  pub fn usb_data_received(data: &[u8]) {
    const CHUNK: usize = 16;
    let len = data.len();
    defmt::println!("[usb-rx] {} bytes --", len);

    let mut offset = 0;
    while offset < len {
      let end = core::cmp::min(offset + CHUNK, len);
      let chunk = &data[offset..end];

      if chunk.len() <= 8 {
        defmt::println!("  {:04x}: {:02x}", offset, chunk);
      } else {
        let (first, second) = chunk.split_at(8);
        defmt::println!("  {:04x}: {:02x} {:02x}", offset, first, second);
      }

      let mut ascii_repr = heapless::String::<CHUNK>::new();
      for &byte in chunk {
        let _ = ascii_repr.push(if (0x20..=0x7E).contains(&byte) {
          byte as char
        } else {
          '.'
        });
      }
      if !ascii_repr.is_empty() {
        defmt::println!("         {}", ascii_repr.as_str());
      }

      offset += CHUNK;
    }
  }

  /// Emit a LoRa-TX byte count (LoRa → USB direction).
  pub fn usb_bridge_tx(byte_count: usize) {
    defmt::println!("[lora-tx] {} bytes", byte_count);
  }

  /// Log an SX1268 reset event.
  pub fn e22_reset() {
    defmt::println!("[e22] reset");
  }

  /// Log an SPI transfer byte count.
  pub fn e22_spi_transfer(bytes: usize) {
    defmt::println!("[spi] {} bytes", bytes);
  }

  /// Log an NSS (chip-select) state change.
  pub fn spi_chip_select(active: bool) {
    defmt::println!("[nss] {}", if active { "assert" } else { "deassert" });
  }

  /// Log an error with caller-supplied context string.
  pub fn error_occurred(context: &str) {
    defmt::println!("[error] {}", context);
  }

  /// Emit a periodic heartbeat log (every 1000 iterations).
  pub fn heartbeat(loop_count: u32) {
    if loop_count.is_multiple_of(1000) {
      defmt::println!("[heartbeat] count={}", loop_count);
    }
  }
}
