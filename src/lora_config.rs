/// E22-400M30S (SX1268) LoRa 配置模块
/// 
/// 本模块定义了 LoRa 模块的配置参数，用户可以通过修改这些参数来定制 LoRa 行为。
/// 修改配置后需要重新编译固件。
///
/// # E22-400M30S 模块规格
/// 
/// - **制造商**: 亿佰特 (Ebyte)
/// - **型号**: E22-400M30S
/// - **芯片**: Semtech SX1268
/// - **频率**: 410-441 MHz / 470-510 MHz
/// - **最大功率**: 30 dBm (1W)
/// - **接口**: SPI
/// - **官方手册**: https://www.ebyte.com/Uploadfiles/Files/2024-12-31/202412311627396369.pdf
///
/// # 参考资料
/// 
/// - E22-400M30S 数据手册
/// - SX126x 芯片手册
/// - Semtech LoRa 计算器: https://www.semtech.com/design-support/lora-calculator
///
/// # 重要提示
///
/// 1. **功率放大器**: E22-400M30S 使用 PA，存在芯片功率（1级）和输出功率（2级）的映射关系
/// 2. **频段合规**: 确保使用符合当地法规的频段和功率
/// 3. **天线要求**: 建议使用 50Ω 阻抗天线
/// 4. **电源要求**: 发射时电流可达 800mA，需要稳定的 3.3V 电源

use defmt::Format;

/// LoRa 工作频率（单位：MHz）
#[derive(Debug, Clone, Copy, Format)]
pub enum Frequency {
    /// 410-441 MHz (中国 ISM 频段)
    Freq410MHz = 410,
    Freq433MHz = 433,  // 默认，最常用
    Freq441MHz = 441,
    /// 470-510 MHz (中国 LoRa 频段)
    Freq470MHz = 470,
    Freq490MHz = 490,
    Freq510MHz = 510,
}

impl Frequency {
    /// 转换为 Hz（用于 sx126x_set_rf_freq）
    pub fn to_hz(&self) -> u32 {
        (*self as u32) * 1_000_000
    }
}

/// 发射功率等级（单位：dBm）
/// 
/// E22-400M30S 使用功率放大器 (PA)，存在两级功率：
/// - 1级功率：SX126x 芯片输出功率（软件可配置）
/// - 2级功率：经过 PA 放大后的最终输出功率
/// 
/// 以下为 433MHz 下的映射关系（仅供参考，实际需要功率计标定）：
#[derive(Debug, Clone, Copy, Format)]
pub enum TxPower {
    /// 最低功率：10 dBm (2级，约 1级-5dBm)
    Power10dBm = 10,
    /// 低功率：15 dBm (2级，约 1级1dBm)
    Power15dBm = 15,
    /// 中低功率：18 dBm (2级，约 1级4dBm)
    Power18dBm = 18,
    /// 中等功率：21 dBm (2级，约 1级7dBm)
    Power21dBm = 21,
    /// 中高功率：24 dBm (2级，约 1级10dBm)
    Power24dBm = 24,
    /// 较高功率：27 dBm (2级，约 1级15dBm)
    Power27dBm = 27,
    /// 最高功率：30 dBm (1W) (2级，约 1级20dBm) - E22-400M30S 最大额定功率
    Power30dBm = 30,
}

impl TxPower {
    /// 获取 2级功率值（最终输出功率）
    pub fn get_output_dbm(&self) -> u8 {
        *self as u8
    }
    
    /// 获取对应的 1级功率值（SX126x 芯片配置值）
    /// 用于 sx126x_set_tx_params
    pub fn get_chip_power(&self) -> i8 {
        match self {
            TxPower::Power10dBm => -5,
            TxPower::Power15dBm => 1,
            TxPower::Power18dBm => 4,
            TxPower::Power21dBm => 7,
            TxPower::Power24dBm => 10,
            TxPower::Power27dBm => 15,
            TxPower::Power30dBm => 20,
        }
    }
}

/// LoRa 调制带宽（单位：kHz）
/// 
/// 影响：
/// - 窄带宽：接收灵敏度更高，传输距离更远，但数据速率较低
/// - 宽带宽：数据速率更高，但距离较短
#[derive(Debug, Clone, Copy, Format)]
pub enum Bandwidth {
    /// 窄带：125 kHz - 长距离，低速率
    Bw125kHz = 125,
    /// 中等：250 kHz - 平衡距离和速率
    Bw250kHz = 250,
    /// 宽带：500 kHz - 短距离，高速率
    Bw500kHz = 500,
}

impl Bandwidth {
    /// 获取 SX126x 寄存器值
    pub fn to_sx126x_value(&self) -> u8 {
        match self {
            Bandwidth::Bw125kHz => 0x04,  // SX126X_LORA_BW_125
            Bandwidth::Bw250kHz => 0x05,  // SX126X_LORA_BW_250
            Bandwidth::Bw500kHz => 0x06,  // SX126X_LORA_BW_500
        }
    }
}

/// LoRa 扩频因子 (Spreading Factor)
/// 
/// SF 越大，传输距离越远但速率越低
/// 可以使用 Semtech LoRa 计算器计算最佳参数组合
#[derive(Debug, Clone, Copy, Format)]
pub enum SpreadingFactor {
    SF7 = 7,   // 最快速率，最短距离
    SF8 = 8,
    SF9 = 9,
    SF10 = 10, // 推荐的平衡设置
    SF11 = 11,
    SF12 = 12, // 最远距离，最慢速率
}

impl SpreadingFactor {
    /// 获取 SX126x 寄存器值
    pub fn to_sx126x_value(&self) -> u8 {
        *self as u8
    }
}

/// 纠错编码率 (Coding Rate)
/// 
/// 格式为 4/n，表示每 4 位有效数据添加 (n-4) 位校验位
#[derive(Debug, Clone, Copy, Format)]
pub enum CodingRate {
    /// 4/5 - 最少冗余，最快速率
    CR45 = 5,
    /// 4/6 - 较少冗余
    CR46 = 6,
    /// 4/7 - 中等冗余（推荐）
    CR47 = 7,
    /// 4/8 - 最多冗余，最强纠错
    CR48 = 8,
}

impl CodingRate {
    /// 获取 SX126x 寄存器值
    pub fn to_sx126x_value(&self) -> u8 {
        (*self as u8) - 4  // 转换为 1,2,3,4
    }
}

/// SX126x PA 配置参数
/// 
/// 根据 SX126x 数据手册 13.1.14 SetPaConfig
#[derive(Debug, Clone, Copy, Format)]
pub struct PaConfig {
    /// PA 占空比 (0x00-0x07)
    pub pa_duty_cycle: u8,
    /// 高功率最大值 (0x00-0x07)
    pub hp_max: u8,
    /// 器件选择 (0x00: SX1262, 0x01: SX1261)
    pub device_sel: u8,
    /// PA LUT (保留，一般设为 0x01)
    pub pa_lut: u8,
}

impl PaConfig {
    /// E22-400M30S 默认 PA 配置
    pub const fn default() -> Self {
        Self {
            pa_duty_cycle: 0x04,
            hp_max: 0x07,
            device_sel: 0x00,  // SX1268 兼容 SX1262
            pa_lut: 0x01,
        }
    }
}

/// LoRa 完整配置结构
#[derive(Debug, Clone, Copy, Format)]
pub struct LoRaConfig {
    /// 工作频率
    pub frequency: Frequency,
    /// 发射功率（2级功率，最终输出）
    pub tx_power: TxPower,
    /// 调制带宽
    pub bandwidth: Bandwidth,
    /// 扩频因子
    pub spreading_factor: SpreadingFactor,
    /// 编码率
    pub coding_rate: CodingRate,
    /// 前导码长度（符号数，6-65535）
    pub preamble_length: u16,
    /// 是否启用 CRC 校验
    pub crc_enabled: bool,
    /// 是否启用显式头部模式
    pub explicit_header: bool,
    /// LoRa 同步字 (0x12=公网, 0x14=私网)
    pub sync_word: u8,
    /// PA 配置
    pub pa_config: PaConfig,
}

impl LoRaConfig {
    /// 创建默认配置
    /// 
    /// 参数设置：
    /// - 433 MHz 频率（中国 ISM 频段）
    /// - 30 dBm 发射功率（最大功率）
    /// - 500 kHz 带宽
    /// - SF11 扩频因子
    /// - 4/5 编码率
    /// - 8 符号前导码
    /// - CRC 校验开启
    /// - 显式头部模式
    /// - 私网同步字 0x14
    pub const fn default() -> Self {
        Self {
            frequency: Frequency::Freq433MHz,
            tx_power: TxPower::Power30dBm,
            bandwidth: Bandwidth::Bw500kHz,
            spreading_factor: SpreadingFactor::SF11,
            coding_rate: CodingRate::CR45,
            preamble_length: 8,
            crc_enabled: true,
            explicit_header: true,
            sync_word: 0x14,  // 私网
            pa_config: PaConfig::default(),
        }
    }

    /// 创建长距离配置
    /// 优化传输距离，牺牲速率
    pub const fn long_range() -> Self {
        Self {
            frequency: Frequency::Freq433MHz,
            tx_power: TxPower::Power30dBm,      // 最大功率
            bandwidth: Bandwidth::Bw125kHz,     // 窄带
            spreading_factor: SpreadingFactor::SF12, // 最大扩频
            coding_rate: CodingRate::CR48,      // 最强纠错
            preamble_length: 12,
            crc_enabled: true,
            explicit_header: true,
            sync_word: 0x14,
            pa_config: PaConfig::default(),
        }
    }

    /// 创建快速传输配置
    /// 优化传输速率，牺牲距离
    pub const fn fast_mode() -> Self {
        Self {
            frequency: Frequency::Freq433MHz,
            tx_power: TxPower::Power27dBm,
            bandwidth: Bandwidth::Bw500kHz,     // 宽带
            spreading_factor: SpreadingFactor::SF7,  // 最小扩频
            coding_rate: CodingRate::CR45,      // 最少冗余
            preamble_length: 6,
            crc_enabled: true,
            explicit_header: true,
            sync_word: 0x14,
            pa_config: PaConfig::default(),
        }
    }

    /// 创建低功耗配置
    /// 降低功率消耗
    pub const fn low_power() -> Self {
        Self {
            frequency: Frequency::Freq433MHz,
            tx_power: TxPower::Power10dBm,      // 最低功率
            bandwidth: Bandwidth::Bw125kHz,
            spreading_factor: SpreadingFactor::SF9,
            coding_rate: CodingRate::CR47,
            preamble_length: 8,
            crc_enabled: true,
            explicit_header: true,
            sync_word: 0x14,
            pa_config: PaConfig::default(),
        }
    }

    /// 获取频率值（MHz）
    pub fn get_frequency_mhz(&self) -> u16 {
        self.frequency as u16
    }
    
    /// 获取频率值（Hz）- 用于 SX126x API
    pub fn get_frequency_hz(&self) -> u32 {
        self.frequency.to_hz()
    }

    /// 获取 2级功率值（dBm，最终输出功率）
    pub fn get_power_dbm(&self) -> u8 {
        self.tx_power.get_output_dbm()
    }
    
    /// 获取 1级功率值（dBm，芯片配置值）
    pub fn get_chip_power_dbm(&self) -> i8 {
        self.tx_power.get_chip_power()
    }

    /// 获取带宽值（kHz）
    pub fn get_bandwidth_khz(&self) -> u16 {
        self.bandwidth as u16
    }

    /// 获取扩频因子值
    pub fn get_sf(&self) -> u8 {
        self.spreading_factor as u8
    }

    /// 获取编码率索引（4/n 中的 n）
    pub fn get_cr_ratio(&self) -> u8 {
        self.coding_rate as u8
    }
    
    /// 获取编码率 SX126x 寄存器值
    pub fn get_cr_sx126x(&self) -> u8 {
        self.coding_rate.to_sx126x_value()
    }
}

// ============================================================
// 用户配置区域
// ============================================================
// 
// 用户可以修改下面的配置来定制 LoRa 行为：
//
// 选项 1: 使用预定义配置
// - LoRaConfig::default()      - 通用默认配置（推荐）
// - LoRaConfig::long_range()   - 长距离模式（最大距离）
// - LoRaConfig::fast_mode()    - 快速模式（最快速率）
// - LoRaConfig::low_power()    - 低功耗模式（省电）
//
// 选项 2: 自定义配置
// 复制 default() 的实现并修改各个字段
//
// 示例：
// ```
// pub const USER_CONFIG: LoRaConfig = LoRaConfig {
//     frequency: Frequency::Freq470MHz,  // 改用 470MHz
//     tx_power: TxPower::Power27dBm,     // 27dBm 功率
//     bandwidth: Bandwidth::Bw250kHz,    // 250kHz 带宽
//     spreading_factor: SpreadingFactor::SF10,
//     coding_rate: CodingRate::CR47,
//     preamble_length: 10,
//     crc_enabled: true,
//     explicit_header: true,
//     sync_word: 0x14,
//     pa_config: PaConfig::default(),
// };
// ```
// ============================================================

/// 当前使用的 LoRa 配置
/// 
/// 修改此行来选择不同的配置：
/// - LoRaConfig::default()      ← 当前使用（基于参考代码的实际配置）
/// - LoRaConfig::long_range()   ← 取消注释使用长距离模式
/// - LoRaConfig::fast_mode()    ← 取消注释使用快速模式
/// - LoRaConfig::low_power()    ← 取消注释使用低功耗模式
pub const CURRENT_CONFIG: LoRaConfig = LoRaConfig::default();

// 或者使用自定义配置（示例）：
/*
pub const CURRENT_CONFIG: LoRaConfig = LoRaConfig {
    frequency: Frequency::Freq433MHz,
    tx_power: TxPower::Power30dBm,
    bandwidth: Bandwidth::Bw125kHz,
    spreading_factor: SpreadingFactor::SF12,
    coding_rate: CodingRate::CR48,
    preamble_length: 12,
    crc_enabled: true,
    explicit_header: true,
    sync_word: 0x14,  // 私网
    pa_config: PaConfig::default(),
};
*/

