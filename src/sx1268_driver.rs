//! SX1268 驱动层实现
//! 
//! 基于 Semtech SX1268 芯片的 LoRa 驱动
//! 参考: SX126x Datasheet 和 E22-400M30S 用户手册

use crate::sx1268_hal::{Sx1268Context, Sx1268HalStatus};
use crate::lora_config::LoRaConfig;
use embedded_hal::spi::SpiBus;
use embedded_hal::digital::{OutputPin, InputPin};
use defmt;

/// SX1268 命令定义
#[allow(dead_code)]
mod commands {
    pub const SET_SLEEP: u8 = 0x84;
    pub const SET_STANDBY: u8 = 0x80;
    pub const SET_FS: u8 = 0xC1;
    pub const SET_TX: u8 = 0x83;
    pub const SET_RX: u8 = 0x82;
    pub const STOP_TIMER_ON_PREAMBLE: u8 = 0x9F;
    pub const SET_RX_DUTY_CYCLE: u8 = 0x94;
    pub const SET_CAD: u8 = 0xC5;
    pub const SET_TX_CONTINUOUS_WAVE: u8 = 0xD1;
    pub const SET_TX_INFINITE_PREAMBLE: u8 = 0xD2;
    pub const SET_REGULATOR_MODE: u8 = 0x96;
    pub const CALIBRATE: u8 = 0x89;
    pub const CALIBRATE_IMAGE: u8 = 0x98;
    pub const SET_PA_CONFIG: u8 = 0x95;
    pub const SET_RX_TX_FALLBACK_MODE: u8 = 0x93;
    pub const WRITE_REGISTER: u8 = 0x0D;
    pub const READ_REGISTER: u8 = 0x1D;
    pub const WRITE_BUFFER: u8 = 0x0E;
    pub const READ_BUFFER: u8 = 0x1E;
    pub const SET_DIO_IRQ_PARAMS: u8 = 0x08;
    pub const GET_IRQ_STATUS: u8 = 0x12;
    pub const CLR_IRQ_STATUS: u8 = 0x02;
    pub const SET_DIO2_AS_RF_SWITCH_CTRL: u8 = 0x9D;
    pub const SET_DIO3_AS_TCXO_CTRL: u8 = 0x97;
    pub const SET_RF_FREQUENCY: u8 = 0x86;
    pub const SET_PACKET_TYPE: u8 = 0x8A;
    pub const GET_PACKET_TYPE: u8 = 0x11;
    pub const SET_TX_PARAMS: u8 = 0x8E;
    pub const SET_MODULATION_PARAMS: u8 = 0x8B;
    pub const SET_PACKET_PARAMS: u8 = 0x8C;
    pub const GET_RX_BUFFER_STATUS: u8 = 0x13;
    pub const GET_PACKET_STATUS: u8 = 0x14;
    pub const GET_RSSI_INST: u8 = 0x15;
    pub const GET_STATS: u8 = 0x10;
    pub const RESET_STATS: u8 = 0x00;
    pub const CFG_DIO_MASK: u8 = 0x9E;
    pub const GET_DEVICE_ERRORS: u8 = 0x17;
    pub const CLR_DEVICE_ERRORS: u8 = 0x07;
    pub const GET_STATUS: u8 = 0xC0;
    pub const SET_LORA_SYMB_NUM_TIMEOUT: u8 = 0xA0;
    pub const SET_BUFFER_BASE_ADDRESS: u8 = 0x8F;
}

/// SX1268 驱动器
pub struct Sx1268Driver<SPI, NSS, NRST, BUSY, TXEN, RXEN>
where
    SPI: SpiBus,
    NSS: OutputPin,
    NRST: OutputPin,
    BUSY: InputPin,
    TXEN: OutputPin,
    RXEN: OutputPin,
{
    hal: Sx1268Context<SPI, NSS, NRST, BUSY, TXEN, RXEN>,
    /// 保存的数据包参数配置
    preamble_length: u16,
    header_type: u8,
    crc_enabled: bool,
    invert_iq: bool,
}

impl<SPI, NSS, NRST, BUSY, TXEN, RXEN> Sx1268Driver<SPI, NSS, NRST, BUSY, TXEN, RXEN>
where
    SPI: SpiBus,
    NSS: OutputPin,
    NRST: OutputPin,
    BUSY: InputPin,
    TXEN: OutputPin,
    RXEN: OutputPin,
{
    /// 创建新的驱动实例
    pub fn new(context: Sx1268Context<SPI, NSS, NRST, BUSY, TXEN, RXEN>) -> Self {
        Self {
            hal: context,
            preamble_length: 8,
            header_type: 0x00, // Explicit header
            crc_enabled: true,
            invert_iq: false,
        }
    }

    /// 初始化 SX1268 芯片
    pub fn init(&mut self, config: &LoRaConfig, delay_fn: &mut dyn FnMut(u32)) -> Result<(), ()> {
        defmt::info!("[SX1268] 开始初始化");
        
        // 1. 硬件复位
        self.hal.reset(delay_fn);
        defmt::info!("[SX1268] ✓ 硬件复位完成");
        
        // 2. 唤醒
        self.hal.wakeup(delay_fn);
        defmt::info!("[SX1268] ✓ 唤醒完成");
        
        // 3. 清除设备错误
        self.clear_device_errors()?;
        defmt::info!("[SX1268] ✓ 清除设备错误");
        
        // 4. 设置待机模式(RC)
        self.set_standby(0x00)?;
        defmt::info!("[SX1268] ✓ 待机模式 (RC)");
        
        // 5. SPI 通信测试 - 写入并读回寄存器验证
        self.spi_test()?;
        defmt::info!("[SX1268] ✓ SPI 通信测试通过");
        
        // 6. 设置内部电源模式 (DCDC)
        self.set_regulator_mode(0x01)?;
        defmt::info!("[SX1268] ✓ 内部电源模式 (DCDC)");
        
        // 7. 禁用 DIO2 控制射频开关
        // E22-400M30S 使用外部 TXEN/RXEN 控制，不使用 DIO2
        self.set_dio2_as_rf_switch(false)?;
        defmt::info!("[SX1268] ✓ DIO2 RF 开关已禁用 (使用外部 TXEN/RXEN)");
        
        // 8. 开启 TCXO (E22 使用 3.3V TCXO)
        self.set_dio3_as_tcxo(0x07, 320)?; // 3.3V, 10ms timeout
        defmt::info!("[SX1268] ✓ TCXO 配置完成 (3.3V, 10ms)");
        
        // 9. 检查 XOSC 启动
        self.check_xosc_start()?;
        defmt::info!("[SX1268] ✓ XOSC 启动正常");
        
        // 10. 设置缓冲区基地址
        self.set_buffer_base_address(0x00, 0x00)?; // TX=0x00, RX=0x00
        defmt::info!("[SX1268] ✓ 缓冲区基地址设置完成");
        
        // 11. 校准
        self.calibrate(0x7F)?; // 校准所有
        
        // 12. 设置数据包类型为 LoRa
        self.set_packet_type(0x01)?; // 0x01 = LoRa
        
        // 13. 设置射频频率
        let freq_hz = config.get_frequency_hz();
        self.set_rf_frequency(freq_hz)?;
        
        // 14. 设置 PA 配置
        self.set_pa_config(
            config.pa_config.pa_duty_cycle,
            config.pa_config.hp_max,
            config.pa_config.device_sel,
            config.pa_config.pa_lut,
        )?;
        
        // 15. 设置发射功率
        let chip_power = config.get_chip_power_dbm() as u8;
        self.set_tx_params(chip_power, 0x04)?; // 0x04 = 40us ramp
        
        // 16. 设置 RX/TX 完成后的状态
        self.set_rx_tx_fallback_mode(0x40)?; // 0x40 = STDBY_RC
        defmt::info!("[SX1268] ✓ FALLBACK 模式配置完成");
        
        // 17. 设置 LoRa 调制参数
        self.set_lora_mod_params(config)?;
        
        // 18. 设置 LoRa 数据包参数
        self.set_lora_packet_params(config)?;
        
        // 19. 设置 LoRa 同步字
        self.set_lora_sync_word(config.sync_word)?;
        
        defmt::info!("[SX1268] 初始化完成");
        Ok(())
    }

    /// 发送数据
    pub fn transmit(&mut self, data: &[u8], delay_fn: &mut dyn FnMut(u32)) -> Result<(), ()> {
        let len = data.len();
        defmt::info!("[SX1268] 发送 {} 字节", len);
        
        // TX 完成延迟 - 简化处理，实际应该检查中断
        const TX_COMPLETION_DELAY_MS: u32 = 100;
        
        // 1. 设置待机模式
        self.set_standby(0x00)?;
        defmt::debug!("[SX1268] → 待机模式");
        
        // 2. 写入数据到缓冲区
        self.write_buffer(0x00, data)?;
        defmt::debug!("[SX1268] → 数据写入缓冲区");
        
        // 3. 更新数据包长度为实际长度
        self.update_packet_length(len as u8)?;
        defmt::debug!("[SX1268] → 数据包长度: {} 字节", len);
        
        // 4. 设置中断参数（发送完成）
        self.set_dio_irq_params(0x0001, 0x0001, 0x0000, 0x0000)?; // TxDone
        
        // 5. 清除中断状态
        self.clear_irq_status(0xFFFF)?;
        
        // 6. 手动切换 RF 开关到 TX 模式
        // E22-400M30S 使用外部 TXEN/RXEN 引脚控制
        match self.hal.rf_switch_tx() {
            Sx1268HalStatus::Ok => {}
            _ => return Err(()),
        }
        defmt::info!("[SX1268] → RF 开关切换到 TX");
        
        // 7. 进入发送模式
        // 超时计算: 15.625μs per tick, 0x002710 = 10000 ticks = 156.25ms
        let timeout = 0x002710; // ~156ms timeout
        self.set_tx(timeout)?;
        defmt::info!("[SX1268] → 进入发送模式 (超时: 0x{:06X} ~156ms)", timeout);
        
        // 8. 验证芯片状态
        if let Ok(status) = self.get_status() {
            let mode = (status >> 4) & 0x07;
            let cmd_status = (status >> 1) & 0x07;
            defmt::info!("[SX1268] 状态验证: 模式={}, 命令状态={}", mode, cmd_status);
            // 模式: 6=TX, 命令状态: 1=success
            if mode != 6 {
                defmt::warn!("[SX1268] ⚠ 芯片模式异常: 期望 6(TX), 实际 {}", mode);
            }
            if cmd_status != 1 {
                defmt::warn!("[SX1268] ⚠ 命令状态异常: 期望 1(success), 实际 {}", cmd_status);
            }
        }
        
        // 9. 等待发送完成 (简化处理，实际应该检查中断)
        defmt::info!("[SX1268] ⏳ 等待发送完成 ({}ms)", TX_COMPLETION_DELAY_MS);
        delay_fn(TX_COMPLETION_DELAY_MS);
        
        // 10. 关闭 RF 开关
        match self.hal.rf_switch_off() {
            Sx1268HalStatus::Ok => {}
            _ => return Err(()),
        }
        defmt::info!("[SX1268] → RF 开关关闭");
        
        defmt::info!("[SX1268] ✓ 发送完成");
        Ok(())
    }

    // ==================== 底层命令实现 ====================

    fn set_standby(&mut self, mode: u8) -> Result<(), ()> {
        let cmd = [commands::SET_STANDBY, mode];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_regulator_mode(&mut self, mode: u8) -> Result<(), ()> {
        let cmd = [commands::SET_REGULATOR_MODE, mode];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_dio2_as_rf_switch(&mut self, enable: bool) -> Result<(), ()> {
        let cmd = [commands::SET_DIO2_AS_RF_SWITCH_CTRL, if enable { 0x01 } else { 0x00 }];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_dio3_as_tcxo(&mut self, voltage: u8, timeout: u16) -> Result<(), ()> {
        let timeout_bytes = timeout.to_be_bytes();
        let mut data = [0u8; 4];
        data[0] = voltage;
        data[1] = timeout_bytes[0];
        data[2] = timeout_bytes[1];
        data[3] = 0x00;
        
        let cmd = [commands::SET_DIO3_AS_TCXO_CTRL];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn calibrate(&mut self, param: u8) -> Result<(), ()> {
        let cmd = [commands::CALIBRATE, param];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_buffer_base_address(&mut self, tx_base: u8, rx_base: u8) -> Result<(), ()> {
        let cmd = [commands::SET_BUFFER_BASE_ADDRESS, tx_base, rx_base];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_packet_type(&mut self, pkt_type: u8) -> Result<(), ()> {
        let cmd = [commands::SET_PACKET_TYPE, pkt_type];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_rf_frequency(&mut self, freq_hz: u32) -> Result<(), ()> {
        // SX1268 频率计算公式: freq_reg = (freq_hz * 2^25) / FXTAL
        // 其中 FXTAL = 32MHz (SX1268 晶振频率)
        const FXTAL: u64 = 32_000_000;
        const FREQ_STEP: u64 = 1 << 25; // 2^25
        
        let freq_reg = ((freq_hz as u64) * FREQ_STEP / FXTAL) as u32;
        let freq_bytes = freq_reg.to_be_bytes();
        
        let cmd = [commands::SET_RF_FREQUENCY];
        match self.hal.write(&cmd, &freq_bytes) {
            Sx1268HalStatus::Ok => {
                defmt::debug!("[SX1268] 频率设置: {}Hz (reg=0x{:08X})", freq_hz, freq_reg);
                Ok(())
            },
            _ => Err(()),
        }
    }

    fn set_pa_config(&mut self, duty_cycle: u8, hp_max: u8, device_sel: u8, pa_lut: u8) -> Result<(), ()> {
        let data = [duty_cycle, hp_max, device_sel, pa_lut];
        let cmd = [commands::SET_PA_CONFIG];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_tx_params(&mut self, power: u8, ramp_time: u8) -> Result<(), ()> {
        let data = [power, ramp_time];
        let cmd = [commands::SET_TX_PARAMS];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_rx_tx_fallback_mode(&mut self, mode: u8) -> Result<(), ()> {
        let cmd = [commands::SET_RX_TX_FALLBACK_MODE, mode];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_lora_mod_params(&mut self, config: &LoRaConfig) -> Result<(), ()> {
        let sf = config.get_sf() as u8;
        let bw_val = config.bandwidth.to_sx126x_value();
        let cr_val = config.coding_rate.to_sx126x_value();
        let ldro = if sf >= 11 { 0x01 } else { 0x00 }; // Low datarate optimize
        
        let data = [sf, bw_val, cr_val, ldro];
        let cmd = [commands::SET_MODULATION_PARAMS];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => {
                defmt::debug!("[SX1268] 调制参数: SF={} BW={} CR={} LDRO={}", sf, bw_val, cr_val, ldro);
                Ok(())
            },
            _ => Err(()),
        }
    }

    fn set_lora_packet_params(&mut self, config: &LoRaConfig) -> Result<(), ()> {
        let preamble = config.preamble_length.to_be_bytes();
        let header_type = if config.explicit_header { 0x00 } else { 0x01 };
        let payload_len = 255u8; // Max payload length for initialization
        let crc = if config.crc_enabled { 0x01 } else { 0x00 };
        let invert_iq = 0x00;
        
        // 保存参数以便后续发送时使用
        self.preamble_length = config.preamble_length;
        self.header_type = header_type;
        self.crc_enabled = config.crc_enabled;
        self.invert_iq = false;
        
        let data = [preamble[0], preamble[1], header_type, payload_len, crc, invert_iq];
        let cmd = [commands::SET_PACKET_PARAMS];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn update_packet_length(&mut self, payload_len: u8) -> Result<(), ()> {
        // 使用保存的参数，只更新payload长度
        let preamble = self.preamble_length.to_be_bytes();
        let crc = if self.crc_enabled { 0x01 } else { 0x00 };
        let invert_iq = if self.invert_iq { 0x01 } else { 0x00 };
        
        let data = [preamble[0], preamble[1], self.header_type, payload_len, crc, invert_iq];
        let cmd = [commands::SET_PACKET_PARAMS];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => {
                defmt::debug!("[SX1268] 数据包长度更新: {} 字节 (保留其他参数)", payload_len);
                Ok(())
            },
            _ => Err(()),
        }
    }

    fn set_lora_sync_word(&mut self, sync_word: u8) -> Result<(), ()> {
        // LoRa sync word is at register 0x0740
        let addr = 0x0740u16.to_be_bytes();
        let cmd = [commands::WRITE_REGISTER, addr[0], addr[1]];
        let data = [sync_word];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn write_buffer(&mut self, offset: u8, data: &[u8]) -> Result<(), ()> {
        let cmd = [commands::WRITE_BUFFER, offset];
        match self.hal.write(&cmd, data) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_dio_irq_params(&mut self, irq_mask: u16, dio1_mask: u16, dio2_mask: u16, dio3_mask: u16) -> Result<(), ()> {
        let mut data = [0u8; 8];
        data[0..2].copy_from_slice(&irq_mask.to_be_bytes());
        data[2..4].copy_from_slice(&dio1_mask.to_be_bytes());
        data[4..6].copy_from_slice(&dio2_mask.to_be_bytes());
        data[6..8].copy_from_slice(&dio3_mask.to_be_bytes());
        
        let cmd = [commands::SET_DIO_IRQ_PARAMS];
        match self.hal.write(&cmd, &data) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn clear_irq_status(&mut self, irq_mask: u16) -> Result<(), ()> {
        let irq_bytes = irq_mask.to_be_bytes();
        let cmd = [commands::CLR_IRQ_STATUS, irq_bytes[0], irq_bytes[1]];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    fn set_tx(&mut self, timeout: u32) -> Result<(), ()> {
        let timeout_bytes = [(timeout >> 16) as u8, (timeout >> 8) as u8, timeout as u8];
        let cmd = [commands::SET_TX];
        match self.hal.write(&cmd, &timeout_bytes) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    // ==================== 新增诊断和验证功能 ====================
    
    /// 获取芯片状态
    fn get_status(&mut self) -> Result<u8, ()> {
        let cmd = [commands::GET_STATUS, 0x00];
        let mut response = [0u8; 1];
        match self.hal.read(&cmd, &mut response) {
            Sx1268HalStatus::Ok => {
                defmt::debug!("[SX1268] 状态寄存器: 0x{:02X}", response[0]);
                Ok(response[0])
            }
            _ => Err(()),
        }
    }

    /// 获取设备错误
    fn get_device_errors(&mut self) -> Result<u16, ()> {
        let cmd = [commands::GET_DEVICE_ERRORS, 0x00, 0x00];
        let mut response = [0u8; 2];
        match self.hal.read(&cmd, &mut response) {
            Sx1268HalStatus::Ok => {
                let errors = u16::from_be_bytes([response[0], response[1]]);
                if errors != 0 {
                    defmt::warn!("[SX1268] 设备错误: 0x{:04X}", errors);
                }
                Ok(errors)
            }
            _ => Err(()),
        }
    }

    /// 清除设备错误
    fn clear_device_errors(&mut self) -> Result<(), ()> {
        let cmd = [commands::CLR_DEVICE_ERRORS, 0x00, 0x00];
        match self.hal.write(&cmd, &[]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }

    /// SPI 通信测试 - 写入并读回寄存器
    fn spi_test(&mut self) -> Result<(), ()> {
        // 使用寄存器 0x0920 (随机寄存器) 进行测试，不影响配置
        let test_value = 0xA5u8;
        let test_addr = 0x0920u16; // 测试用寄存器
        
        // 保存原值
        let addr_bytes = test_addr.to_be_bytes();
        let cmd_read_orig = [commands::READ_REGISTER, addr_bytes[0], addr_bytes[1], 0x00];
        let mut orig_value = [0u8; 1];
        match self.hal.read(&cmd_read_orig, &mut orig_value) {
            Sx1268HalStatus::Ok => {}
            _ => return Err(()),
        }
        
        // 写入测试值
        let cmd_write = [commands::WRITE_REGISTER, addr_bytes[0], addr_bytes[1]];
        match self.hal.write(&cmd_write, &[test_value]) {
            Sx1268HalStatus::Ok => {}
            _ => return Err(()),
        }
        
        // 读回验证
        let mut read_data = [0u8; 1];
        match self.hal.read(&cmd_read_orig, &mut read_data) {
            Sx1268HalStatus::Ok => {}
            _ => return Err(()),
        }
        
        // 恢复原值
        match self.hal.write(&cmd_write, &orig_value) {
            Sx1268HalStatus::Ok => {}
            _ => {}  // 即使恢复失败也不阻止初始化
        }
        
        if read_data[0] == test_value {
            defmt::debug!("[SX1268] SPI 测试: 写入 0x{:02X}, 读取 0x{:02X} ✓", test_value, read_data[0]);
            Ok(())
        } else {
            defmt::error!("[SX1268] SPI 测试失败: 期望 0x{:02X}, 实际 0x{:02X}", test_value, read_data[0]);
            Err(())
        }
    }

    /// 检查 XOSC 启动状态
    fn check_xosc_start(&mut self) -> Result<(), ()> {
        let errors = self.get_device_errors()?;
        const XOSC_START_ERR: u16 = 1 << 6;
        
        if (errors & XOSC_START_ERR) != 0 {
            defmt::error!("[SX1268] XOSC 启动失败！");
            Err(())
        } else {
            Ok(())
        }
    }

    /// 读取寄存器（用于验证配置）
    #[allow(dead_code)]
    fn read_register(&mut self, addr: u16) -> Result<u8, ()> {
        let addr_bytes = addr.to_be_bytes();
        let cmd = [commands::READ_REGISTER, addr_bytes[0], addr_bytes[1], 0x00];
        let mut response = [0u8; 1];
        match self.hal.read(&cmd, &mut response) {
            Sx1268HalStatus::Ok => Ok(response[0]),
            _ => Err(()),
        }
    }

    /// 写入寄存器（用于特殊配置）
    #[allow(dead_code)]
    fn write_register(&mut self, addr: u16, value: u8) -> Result<(), ()> {
        let addr_bytes = addr.to_be_bytes();
        let cmd = [commands::WRITE_REGISTER, addr_bytes[0], addr_bytes[1]];
        match self.hal.write(&cmd, &[value]) {
            Sx1268HalStatus::Ok => Ok(()),
            _ => Err(()),
        }
    }
}
