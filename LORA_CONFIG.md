# LoRa 配置指南

本项目为 E22-400M30S (SX1268) LoRa 模块提供了灵活的配置系统，允许用户通过修改代码来定制 LoRa 行为。

## E22-400M30S 模块规格

**官方手册**: https://www.ebyte.com/Uploadfiles/Files/2024-12-31/202412311627396369.pdf

**关键规格**：
- **芯片**: Semtech SX1268
- **频率范围**: 410-441 MHz / 470-510 MHz（中国）
- **发射功率**: 最大 30 dBm (1W)
- **接收灵敏度**: -148 dBm @ SF12, BW125kHz
- **通信接口**: SPI
- **工作电压**: 3.3V
- **通信距离**: 最大可达 16 km (开阔地，最优配置)

**重要提示**：
1. E22-400M30S 使用功率放大器 (PA)，需要注意功率映射
2. 使用前确认所在地区的 ISM 频段和功率限制
3. 建议使用 50Ω 阻抗天线以获得最佳性能
4. 发射时电流可达 800mA，需要稳定的电源供应

## 配置文件位置

所有 LoRa 配置都在 `src/lora_config.rs` 文件中定义。

## 快速开始

### 使用预定义配置

在 `src/lora_config.rs` 文件末尾，找到以下代码：

```rust
pub const CURRENT_CONFIG: LoRaConfig = LoRaConfig::default();
```

可以将 `LoRaConfig::default()` 替换为以下任一预定义配置：

1. **默认配置** - `LoRaConfig::default()`
   - 433 MHz 频率
   - 30 dBm 发射功率（1W，最大功率）
   - 500 kHz 带宽（宽带）
   - SF11 扩频因子
   - CR4/5 编码率
   - 适用于大多数应用场景

2. **长距离模式** - `LoRaConfig::long_range()`
   - 433 MHz 频率
   - 30 dBm 发射功率（1W，最大功率）
   - 125 kHz 带宽（窄带）
   - SF12 扩频因子（最大扩频）
   - CR4/8 编码率（最强纠错）
   - 适用于需要最大传输距离的场景

3. **快速模式** - `LoRaConfig::fast_mode()`
   - 433 MHz 频率
   - 27 dBm 发射功率
   - 500 kHz 带宽（宽带）
   - SF7 扩频因子（最小扩频）
   - CR4/5 编码率（最少冗余）
   - 适用于需要高速率、短距离传输的场景

4. **低功耗模式** - `LoRaConfig::low_power()`
   - 433 MHz 频率
   - 10 dBm 发射功率（约 10mW，最低功率）
   - 125 kHz 带宽
   - SF9 扩频因子
   - CR4/7 编码率
   - 适用于电池供电或低功耗应用

**示例：切换到长距离模式**

```rust
pub const CURRENT_CONFIG: LoRaConfig = LoRaConfig::long_range();
```

### 自定义配置

如果预定义配置不满足需求，可以创建自定义配置：

```rust
pub const CURRENT_CONFIG: LoRaConfig = LoRaConfig {
    frequency: Frequency::Freq470MHz,        // 470 MHz 频率
    tx_power: TxPower::Power22dBm,           // 22 dBm 功率
    bandwidth: Bandwidth::Bw250kHz,          // 250 kHz 带宽
    spreading_factor: SpreadingFactor::SF11, // SF11 扩频因子
    coding_rate: CodingRate::CR47,           // 4/7 编码率
    preamble_length: 10,                     // 10 符号前导码
    crc_enabled: true,                       // 启用 CRC
    explicit_header: true,                   // 显式头部
    sync_word: 0x14,                         // 私网同步字
    pa_config: PaConfig::default(),          // 默认 PA 配置
};
```

## 配置参数说明

### 1. 频率 (Frequency)

可选值：
- `Freq410MHz` - 410 MHz
- `Freq433MHz` - 433 MHz（默认，中国 ISM 频段）
- `Freq441MHz` - 441 MHz
- `Freq470MHz` - 470 MHz（中国 LoRa 频段）
- `Freq490MHz` - 490 MHz
- `Freq510MHz` - 510 MHz

**注意**：选择符合当地法规的频段。

### 2. 发射功率 (TxPower)

可选值：
- `Power10dBm` - 10 dBm (~10mW，最低）
- `Power13dBm` - 13 dBm (~20mW)
- `Power17dBm` - 17 dBm (~50mW，推荐）
- `Power20dBm` - 20 dBm (~100mW)
- `Power22dBm` - 22 dBm (~158mW)
- `Power30dBm` - 30 dBm (1W，最大）

**影响**：功率越大，传输距离越远，但能耗也越高。

### 3. 带宽 (Bandwidth)

可选值：
- `Bw125kHz` - 125 kHz（窄带，长距离）
- `Bw250kHz` - 250 kHz（中等）
- `Bw500kHz` - 500 kHz（宽带，高速率）

**影响**：
- 窄带宽 = 更好的接收灵敏度，更远的距离，但速率较低
- 宽带宽 = 更高的数据速率，但距离较短

### 4. 扩频因子 (SpreadingFactor)

可选值：`SF7`, `SF8`, `SF9`, `SF10`, `SF11`, `SF12`

**影响**：
- SF7 = 最快速率，最短距离
- SF12 = 最慢速率，最远距离
- 推荐使用 SF10 作为平衡设置

### 5. 编码率 (CodingRate)

可选值：
- `CR45` - 4/5（最少冗余，最快）
- `CR46` - 4/6
- `CR47` - 4/7（推荐）
- `CR48` - 4/8（最多冗余，最强纠错）

**影响**：编码率越高，纠错能力越强，但传输开销也越大。

### 6. 前导码长度 (preamble_length)

可选范围：6-65535 符号

**推荐值**：
- 一般应用：8
- 长距离：12
- 快速传输：6

### 7. CRC 校验 (crc_enabled)

- `true` - 启用（推荐）
- `false` - 禁用

### 8. 头部模式 (explicit_header)

- `true` - 显式头部（推荐）
- `false` - 隐式头部

## 应用配置后

1. 保存 `src/lora_config.rs` 文件
2. 重新编译固件：`cargo build --release`
3. 烧录到设备：`cargo run --release`

## 查看当前配置

当设备启动时：
- **OLED 显示屏**会显示当前的频率、功率、带宽和扩频因子
- **probe-rs 日志**会输出完整的配置信息

示例日志输出：
```
========== LoRa 配置 ==========
频率: 433 MHz
功率: 17 dBm
带宽: 125 kHz
扩频因子: SF10
编码率: CR4/7
前导码: 8 符号
CRC: 启用
============================
```

## 配置建议

### 城市环境短距离通信
```rust
frequency: Frequency::Freq433MHz,
tx_power: TxPower::Power17dBm,
bandwidth: Bandwidth::Bw250kHz,
spreading_factor: SpreadingFactor::SF8,
```

### 郊区/农村长距离通信
```rust
frequency: Frequency::Freq433MHz,
tx_power: TxPower::Power30dBm,
bandwidth: Bandwidth::Bw125kHz,
spreading_factor: SpreadingFactor::SF12,
```

### 数据采集节点（低功耗）
```rust
frequency: Frequency::Freq433MHz,
tx_power: TxPower::Power10dBm,
bandwidth: Bandwidth::Bw125kHz,
spreading_factor: SpreadingFactor::SF9,
```

### 实时控制（低延迟）
```rust
frequency: Frequency::Freq433MHz,
tx_power: TxPower::Power17dBm,
bandwidth: Bandwidth::Bw500kHz,
spreading_factor: SpreadingFactor::SF7,
```

## 注意事项

1. **法规合规**：确保使用的频率和功率符合当地无线电法规
2. **匹配配置**：通信双方必须使用相同的配置参数
3. **功率限制**：虽然 E22-400M30S 支持 30dBm，但某些地区可能有功率限制
4. **距离估算**：实际传输距离受环境、天线、障碍物等多种因素影响

## 故障排除

**问题：无法通信**
- 检查双方是否使用相同的配置
- 验证频率设置是否正确
- 确认天线连接良好

**问题：距离不够远**
- 增加发射功率
- 使用更高的扩频因子（SF11 或 SF12）
- 减小带宽到 125 kHz
- 检查天线质量和安装位置

**问题：传输速率太慢**
- 降低扩频因子（SF7 或 SF8）
- 增加带宽到 500 kHz
- 降低编码率到 CR45

## 参考资源

- E22-400M30S 数据手册
- LoRa 调制解调技术文档
- SX1268 芯片规格书
