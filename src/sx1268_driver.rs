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
        
        // 2. 唤醒
        self.hal.wakeup(delay_fn);
        
        // 3. 设置待机模式(RC)
        self.set_standby(0x00)?;
        
        // 4. 设置内部电源模式 (DCDC)
        self.set_regulator_mode(0x01)?;
        
        // 5. 禁止 DIO2 切换射频开关 (E22 使用外部 TXEN/RXEN)
        self.set_dio2_as_rf_switch(false)?;
        
        // 6. 开启 TCXO (E22 使用 3.3V TCXO)
        self.set_dio3_as_tcxo(0x07, 320)?; // 3.3V, 10ms timeout
        
        // 7. 校准
        self.calibrate(0x7F)?; // 校准所有
        
        // 8. 设置数据包类型为 LoRa
        self.set_packet_type(0x01)?; // 0x01 = LoRa
        
        // 9. 设置射频频率
        let freq_hz = config.get_frequency_hz();
        self.set_rf_frequency(freq_hz)?;
        
        // 10. 设置 PA 配置
        self.set_pa_config(
            config.pa_config.pa_duty_cycle,
            config.pa_config.hp_max,
            config.pa_config.device_sel,
            config.pa_config.pa_lut,
        )?;
        
        // 11. 设置发射功率
        let chip_power = config.get_chip_power_dbm() as u8;
        self.set_tx_params(chip_power, 0x04)?; // 0x04 = 40us ramp
        
        // 12. 设置 RX/TX 完成后的状态
        self.set_rx_tx_fallback_mode(0x20)?; // 0x20 = STDBY_RC
        
        // 13. 设置 LoRa 调制参数
        self.set_lora_mod_params(config)?;
        
        // 14. 设置 LoRa 数据包参数
        self.set_lora_packet_params(config)?;
        
        // 15. 设置 LoRa 同步字
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
        
        // 2. 写入数据到缓冲区
        self.write_buffer(0x00, data)?;
        
        // 3. 更新数据包长度为实际长度
        self.update_packet_length(len as u8)?;
        
        // 4. 设置中断参数（发送完成）
        self.set_dio_irq_params(0x0001, 0x0001, 0x0000, 0x0000)?; // TxDone
        
        // 5. 清除中断状态
        self.clear_irq_status(0xFFFF)?;
        
        // 6. 切换 RF 开关到发送
        self.hal.rf_switch_tx();
        
        // 7. 进入发送模式
        self.set_tx(0x000000)?; // No timeout
        
        // 8. 等待发送完成 (简化处理，实际应该检查中断)
        delay_fn(TX_COMPLETION_DELAY_MS);
        
        // 9. 关闭 RF 开关
        self.hal.rf_switch_off();
        
        defmt::info!("[SX1268] 发送完成");
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
}
