// Blue-High 项目诊断模块
// 专为 E22-400M30S LoRa 桥接器设计的调试输出

// defmt 时间戳实现（使用运行时计数）
defmt::timestamp!("{=u32:us}", {
  // 简单递增时间戳 - 在实际应用中可以使用系统定时器
  static mut TIMESTAMP: u32 = 0;
  unsafe {
    TIMESTAMP = TIMESTAMP.wrapping_add(1);
    TIMESTAMP
  }
});

pub struct BlueHighDiagnostics;

impl BlueHighDiagnostics {
  // 系统启动诊断
  pub fn boot_sequence(stage: &str) {
    defmt::println!("🚀 [启动] {}", stage);
  }

  // 时钟配置诊断
  pub fn clocks_configured(sys_mhz: u32, apb1_mhz: u32) {
    defmt::println!("⏰ [时钟] 系统: {}MHz, APB1: {}MHz", sys_mhz, apb1_mhz);
  }

  // OLED 显示器状态
  pub fn oled_status(message: &str) {
    defmt::println!("📺 [OLED] {}", message);
  }

  // USB CDC 桥接活动
  pub fn usb_bridge_rx(byte_count: usize) {
    defmt::println!("📥 [USB→LoRa] 接收 {} 字节", byte_count);
  }

  // USB 接收数据详细监控（显示十六进制和可打印 ASCII）
  pub fn usb_data_received(data: &[u8]) {
    const MAX_CHUNK_SIZE: usize = 16;
    let len = data.len();
    defmt::println!("┌─ USB 数据详细内容 ({} 字节) ─", len);

    // 每行显示最多 16 字节
    let mut offset = 0;
    while offset < len {
      let end = core::cmp::min(offset + MAX_CHUNK_SIZE, len);
      let chunk = &data[offset..end];

      // 方法1：显示完整的十六进制行
      if chunk.len() <= 8 {
        defmt::println!("│ {:04x}: {:02x}", offset, chunk);
      } else {
        // 分成两部分显示
        let (first, second) = chunk.split_at(8);
        defmt::println!("│ {:04x}: {:02x} {:02x}", offset, first, second);
      }

      // 方法2：显示可打印的 ASCII 内容
      let mut ascii_repr = heapless::String::<MAX_CHUNK_SIZE>::new();
      for &byte in chunk {
        if byte >= 0x20 && byte <= 0x7E {
          // 可打印 ASCII 字符
          let _ = ascii_repr.push(byte as char);
        } else {
          let _ = ascii_repr.push('.');
        }
      }
      if !ascii_repr.is_empty() {
        defmt::println!("│       ASCII: {}", ascii_repr.as_str());
      }

      offset += MAX_CHUNK_SIZE;
    }
    defmt::println!("└──────────────────────────────────");
  }

  pub fn usb_bridge_tx(byte_count: usize) {
    defmt::println!("📤 [LoRa→USB] 发送 {} 字节", byte_count);
  }

  // E22 LoRa 模块状态
  pub fn e22_reset() {
    defmt::println!("🔄 [E22] SX1268 复位序列");
  }

  pub fn e22_spi_transfer(bytes: usize) {
    defmt::println!("📡 [E22-SPI] 传输 {} 字节", bytes);
  }

  // SPI 总线活动
  pub fn spi_chip_select(active: bool) {
    let state = if active { "选中" } else { "释放" };
    defmt::println!("🔌 [SPI-NSS] {}", state);
  }

  // 错误诊断
  pub fn error_occurred(context: &str) {
    defmt::println!("❌ [错误] {}", context);
  }

  // 主循环心跳
  pub fn heartbeat(loop_count: u32) {
    if loop_count % 1000 == 0 {
      defmt::println!("💓 [心跳] 运行计数: {}", loop_count);
    }
  }
}
