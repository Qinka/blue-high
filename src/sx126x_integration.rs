/// SX126x 库集成层
/// 使用 sx126x-rs 库驱动 E22-400M30S LoRa 模块

use sx126x::{
    Sx126x, Sx1268,
    op::*,
    conf::*,
};

use embedded_hal::delay::DelayNs;
use embedded_hal::spi::SpiBus;
use embedded_hal::digital::OutputPin;

use crate::lora_config::{LoRaConfig, Bandwidth, SpreadingFactor, CodingRate};

/// E22-400M30S 驱动封装
pub struct E22Driver<SPI, NSS, NRST, BUSY, TXEN, RXEN, DELAY> {
    sx126x: Sx126x<Sx1268, SPI, NSS, BUSY, DELAY>,
    nrst: NRST,
    txen: TXEN,
    rxen: RXEN,
}

impl<SPI, NSS, NRST, BUSY, TXEN, RXEN, DELAY> E22Driver<SPI, NSS, NRST, BUSY, TXEN, RXEN, DELAY>
where
    SPI: SpiBus,
    NSS: OutputPin,
    NRST: OutputPin,
    BUSY: embedded_hal::digital::InputPin,
    TXEN: OutputPin,
    RXEN: OutputPin,
    DELAY: DelayNs,
{
    /// 创建新的 E22 驱动实例
    pub fn new(
        spi: SPI,
        nss: NSS,
        nrst: NRST,
        busy: BUSY,
        txen: TXEN,
        rxen: RXEN,
        delay: DELAY,
    ) -> Self {
        let sx126x = Sx126x::new(Sx1268::new(), spi, nss, busy, delay);
        Self {
            sx126x,
            nrst,
            txen,
            rxen,
        }
    }

    /// 初始化 E22-400M30S 模块
    pub fn init(&mut self, config: &LoRaConfig) -> Result<(), &'static str> {
        defmt::info!("[E22] 开始初始化 E22-400M30S (使用 sx126x-rs)");

        // 硬件复位
        self.reset()?;
        
        // 配置 LoRa 参数
        self.configure_lora(config)?;
        
        defmt::info!("[E22] ✅ 初始化完成");
        Ok(())
    }

    /// 硬件复位
    fn reset(&mut self) -> Result<(), &'static str> {
        defmt::info!("[E22] 执行硬件复位");
        
        // NRST 低电平
        self.nrst.set_low().map_err(|_| "NRST set low failed")?;
        // 延迟会由 sx126x 库的 delay 处理
        
        // NRST 高电平
        self.nrst.set_high().map_err(|_| "NRST set high failed")?;
        
        Ok(())
    }

    /// 配置 LoRa 参数
    fn configure_lora(&mut self, config: &LoRaConfig) -> Result<(), &'static str> {
        defmt::info!("[E22] 配置 LoRa 参数");

        // 设置待机模式
        self.sx126x.set_standby(StandbyConfig::Rc)
            .map_err(|_| "Set standby failed")?;

        // 设置数据包类型为 LoRa
        self.sx126x.set_packet_type(PacketType::LoRa)
            .map_err(|_| "Set packet type failed")?;

        // 配置射频频率
        let freq_hz = config.frequency.to_hz();
        self.sx126x.set_rf_frequency(freq_hz)
            .map_err(|_| "Set frequency failed")?;
        defmt::info!("[E22] 频率: {} Hz", freq_hz);

        // 配置 PA (功率放大器)
        let pa_config = PaConfig {
            pa_duty_cycle: config.pa_config.duty_cycle,
            hp_max: config.pa_config.hp_max,
            device_sel: 0x00, // SX1268
            pa_lut: 0x01,
        };
        self.sx126x.set_pa_config(&pa_config)
            .map_err(|_| "Set PA config failed")?;

        // 配置发射功率
        let power = config.tx_power.get_chip_power() as i8;
        self.sx126x.set_tx_params(power, RampTime::Ramp40Us)
            .map_err(|_| "Set TX params failed")?;
        defmt::info!("[E22] 功率: {} dBm", power);

        // 配置调制参数
        let sf = self.config_sf_to_sx126x(config.spreading_factor);
        let bw = self.config_bw_to_sx126x(config.bandwidth);
        let cr = self.config_cr_to_sx126x(config.coding_rate);
        
        let modulation = LoRaModParams {
            sf,
            bw,
            cr,
            ldro: config.spreading_factor as u8 >= 11, // SF11+ 需要 LDRO
        };
        
        self.sx126x.set_lora_mod_params(&modulation)
            .map_err(|_| "Set modulation params failed")?;
        
        defmt::info!("[E22] 调制: SF={} BW={} CR={}", 
            config.spreading_factor as u8,
            match config.bandwidth {
                Bandwidth::Bw125kHz => 125,
                Bandwidth::Bw250kHz => 250,
                Bandwidth::Bw500kHz => 500,
            },
            match config.coding_rate {
                CodingRate::CR45 => "4/5",
                CodingRate::CR46 => "4/6",
                CodingRate::CR47 => "4/7",
                CodingRate::CR48 => "4/8",
            }
        );

        // 配置数据包参数
        let packet_params = LoRaPacketParams {
            preamble_len: config.preamble_length,
            header_type: if config.explicit_header { 
                LoRaHeaderType::Explicit 
            } else { 
                LoRaHeaderType::Implicit 
            },
            payload_len: 255, // 最大长度，实际发送时会更新
            crc_enable: config.crc_enabled,
            invert_iq: false,
        };
        
        self.sx126x.set_lora_packet_params(&packet_params)
            .map_err(|_| "Set packet params failed")?;

        // 配置同步字
        self.sx126x.set_lora_sync_word(config.sync_word)
            .map_err(|_| "Set sync word failed")?;

        Ok(())
    }

    /// 发送数据
    pub fn transmit(&mut self, data: &[u8]) -> Result<(), &'static str> {
        defmt::info!("[E22] 发送 {} 字节", data.len());

        // 写入缓冲区
        self.sx126x.write_buffer(0, data)
            .map_err(|_| "Write buffer failed")?;

        // 更新数据包长度
        self.sx126x.set_lora_packet_params_payload_len(data.len() as u8)
            .map_err(|_| "Set payload length failed")?;

        // RF 开关切换到 TX
        self.rf_switch_tx()?;

        // 进入发送模式
        self.sx126x.set_tx(0x00FFFF) // 约4秒超时
            .map_err(|_| "Set TX mode failed")?;

        // 等待发送完成 (简化版，实际应该使用中断)
        // delay 会在 sx126x 库内部处理

        // RF 开关关闭
        self.rf_switch_off()?;

        defmt::info!("[E22] ✓ 发送完成");
        Ok(())
    }

    /// RF 开关切换到发送
    fn rf_switch_tx(&mut self) -> Result<(), &'static str> {
        self.rxen.set_low().map_err(|_| "RXEN set low failed")?;
        self.txen.set_high().map_err(|_| "TXEN set high failed")?;
        defmt::debug!("[E22] RF 开关 -> TX");
        Ok(())
    }

    /// RF 开关切换到接收
    #[allow(dead_code)]
    fn rf_switch_rx(&mut self) -> Result<(), &'static str> {
        self.txen.set_low().map_err(|_| "TXEN set low failed")?;
        self.rxen.set_high().map_err(|_| "RXEN set high failed")?;
        defmt::debug!("[E22] RF 开关 -> RX");
        Ok(())
    }

    /// RF 开关关闭
    fn rf_switch_off(&mut self) -> Result<(), &'static str> {
        self.txen.set_low().map_err(|_| "TXEN set low failed")?;
        self.rxen.set_low().map_err(|_| "RXEN set low failed")?;
        defmt::debug!("[E22] RF 开关 -> OFF");
        Ok(())
    }

    // 辅助函数：配置转换
    fn config_sf_to_sx126x(&self, sf: SpreadingFactor) -> LoRaSpreadingFactor {
        match sf as u8 {
            7 => LoRaSpreadingFactor::Sf7,
            8 => LoRaSpreadingFactor::Sf8,
            9 => LoRaSpreadingFactor::Sf9,
            10 => LoRaSpreadingFactor::Sf10,
            11 => LoRaSpreadingFactor::Sf11,
            12 => LoRaSpreadingFactor::Sf12,
            _ => LoRaSpreadingFactor::Sf11, // 默认
        }
    }

    fn config_bw_to_sx126x(&self, bw: Bandwidth) -> LoRaBandwidth {
        match bw {
            Bandwidth::Bw125kHz => LoRaBandwidth::Bw125,
            Bandwidth::Bw250kHz => LoRaBandwidth::Bw250,
            Bandwidth::Bw500kHz => LoRaBandwidth::Bw500,
        }
    }

    fn config_cr_to_sx126x(&self, cr: CodingRate) -> LoRaCodingRate {
        match cr {
            CodingRate::CR45 => LoRaCodingRate::Cr45,
            CodingRate::CR46 => LoRaCodingRate::Cr46,
            CodingRate::CR47 => LoRaCodingRate::Cr47,
            CodingRate::CR48 => LoRaCodingRate::Cr48,
        }
    }
}
