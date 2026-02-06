# blue-high

STM32F103C8T6 MCU 控制程序 - OLED 和 LoRa 控制器

## 项目概述

这是一个使用 Rust 语言开发的 STM32F103C8T6 微控制器程序，能够同时控制：
- 0.96寸 OLED 屏幕 (SSD1306，通过 I2C2 接口)
- 亿佰特 E22-400M30S LoRa 无线模块 (SX1268 芯片，通过 SPI 接口)
- USB CDC 虚拟串口 (用于 PC 与 LoRa 之间的控制)

**E22-400M30S 官方手册**: https://www.ebyte.com/Uploadfiles/Files/2024-12-31/202412311627396369.pdf

## LoRa 配置

用户可以通过修改 `src/lora_config.rs` 文件来配置 LoRa 参数（频率、功率、带宽、扩频因子等）。

详细配置说明请参阅 [LORA_CONFIG.md](LORA_CONFIG.md)。

**可配置参数**：
- 频率：410-510 MHz
- 功率：10-30 dBm
- 带宽：125/250/500 kHz  
- 扩频因子：SF7-SF12
- 编码率：CR4/5 到 CR4/8
- 前导码长度、CRC、同步字等

**预定义配置**：
- `LoRaConfig::default()` - 默认配置（30dBm, BW500, SF11）
- `LoRaConfig::long_range()` - 长距离模式（30dBm, BW125, SF12）
- `LoRaConfig::fast_mode()` - 快速模式（27dBm, BW500, SF7）
- `LoRaConfig::low_power()` - 低功耗模式（10dBm, BW125, SF9）

## 硬件连接

### OLED 显示屏 (I2C2)
- SCL -> PB10
- SDA -> PB11
- VCC -> 3.3V
- GND -> GND

### E22-400M30S LoRa 模块 (SPI)
- SCK -> PA5
- MISO -> PA6
- MOSI -> PA7
- NSS -> PA4
- BUSY -> PA3
- DIO1 -> PA2
- NRST -> PA1
- VCC -> 3.3V
- GND -> GND

### USB 接口
- D- -> PA11
- D+ -> PA12
- 通过 USB Type-C 连接到 PC

## 功能特性

1. **OLED 显示**
   - 初始化 SSD1306 OLED 显示器
   - 显示系统状态信息
   - 显示 USB 和 LoRa SPI 传输状态

2. **USB CDC 虚拟串口**
   - 作为 USB 从设备连接到 PC
   - 创建虚拟 COM 端口
   - 支持标准串口通信
   - 系统时钟 72MHz，USB 时钟通过 PLL 提供 48MHz

3. **LoRa SPI 通信**
   - 使用亿佰特 E22-400M30S 模块（SX1268 芯片）
   - SPI 接口通信
   - 1 MHz SPI 时钟频率
   - 支持通过 USB 控制 LoRa 模块

4. **数据控制**
   - USB 接收数据通过 SPI 发送到 LoRa
   - PC 通过 USB 串口控制 LoRa 模块
   - 实时显示传输状态

5. **实时调试日志 (defmt via SWO)**
   - 集成 defmt 日志系统，通过 SWO (Serial Wire Output) 实时输出
   - 使用 ITM (Instrumentation Trace Macrocell) 进行高效日志传输
   - 中文日志消息，易于理解调试信息
   - 表情符号标记不同类型的事件
   - 监控系统启动、时钟配置、外设初始化
   - 跟踪 USB-LoRa 数据桥接活动
   - USB 输入数据详细监控（十六进制 + ASCII）
   - 主循环心跳计数器
   - SPI 传输详细日志

## 开发环境设置

### 安装 Rust 工具链

```bash
# 安装 Rust (如果还没有安装)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 添加 ARM Cortex-M 目标
rustup target add thumbv7m-none-eabi
```

### 安装调试工具 (可选)

```bash
# 安装 probe-rs 用于调试和烧录
cargo install probe-rs --features cli

# 或者使用 OpenOCD
sudo apt-get install openocd
```

## 构建项目

```bash
# 编译项目
cargo build --release

# 检查项目
cargo check
```

## 烧录程序

### 使用 probe-rs (推荐)

```bash
# 烧录并查看 defmt 调试输出（通过 SWO）
cargo run --release

# 程序会在 probe-rs 中显示实时日志（通过 SWO/ITM 输出），包括：
# 🚀 启动序列信息
# ⏰ 时钟配置 (72MHz 系统时钟, 36MHz APB1)
# 📺 OLED 初始化状态
# 📡 E22-400M30S LoRa 模块状态
# 📥📤 USB 与 LoRa SPI 数据传输活动
# 📋 USB 输入数据详细监控（十六进制 + ASCII）
# 💓 主循环心跳监控
```

**注意**: 
- SWO (Serial Wire Output) 使用 ITM 通道进行日志输出
- 比 RTT 更高效，不占用额外的 RAM
- 需要 SWD 调试器支持 SWO 功能（大多数 ST-Link V2/V3 都支持）
- probe-rs 会自动检测并使用 SWO 输出

### 查看调试日志 (SWO/ITM)

本项目集成了 `defmt` 日志系统，通过 SWO (Serial Wire Output) 可以实时查看设备运行状态：

- **系统启动和初始化过程**
- **时钟配置信息**
- **外设就绪状态** (OLED, USB, E22 LoRa)
- **USB 到 LoRa 的数据桥接活动**
- **USB 输入数据详细监控** - 十六进制和 ASCII 格式显示
- **SPI 传输详情**
- **LoRa 配置参数** - 频率、功率、带宽、SF、CR 等
- **错误诊断信息**

所有日志消息都使用中文和表情符号，便于快速识别不同类型的事件。

**USB 数据监控示例**：
```
📥 [USB→LoRa] 接收 12 字节
┌─ USB 数据详细内容 (12 字节) ─
│ 0000: 48 65 6c 6c 6f 20 4c 6f 52 61 21 0a
│       ASCII: Hello LoRa!.
└──────────────────────────────────
```

**SWO 优势**：
- 不占用额外的 RAM 缓冲区（与 RTT 相比）
- 更高的数据吞吐量
- 实时性更好，延迟更低
- 标准 ARM Cortex-M 功能，硬件支持

### 使用 OpenOCD

```bash
# 1. 启动 OpenOCD
openocd -f interface/stlink.cfg -f target/stm32f1x.cfg

# 2. 在另一个终端中使用 GDB
arm-none-eabi-gdb target/thumbv7m-none-eabi/release/blue-high
(gdb) target remote :3333
(gdb) load
(gdb) continue
```

### 使用 ST-Link Utility

可以将生成的 `.bin` 文件通过 ST-Link Utility 烧录到芯片中。

## 使用方法

### 1. 连接设备

1. 通过 ST-Link 烧录程序到 STM32F103C8T6
2. 连接 OLED 显示屏到 I2C 接口 (PB6/PB7)
3. 连接 E22-400M30S LoRa 模块到 SPI1 (PA5/PA6/PA7 及控制引脚)
4. 通过 USB Type-C 线连接到 PC

### 2. 使用 USB 串口

设备会在 PC 上创建一个虚拟 COM 端口：
- **Windows**: 设备管理器中查看 COM 端口号
- **Linux**: 通常为 `/dev/ttyACM0`
- **Mac**: 通常为 `/dev/cu.usbmodem*`

### 3. 串口通信

使用任何串口工具（如 PuTTY、minicom、screen 等）连接到虚拟 COM 端口：

```bash
# Linux/Mac 示例
screen /dev/ttyACM0 9600

# 或使用 minicom
minicom -D /dev/ttyACM0 -b 9600
```

### 4. 控制 LoRa

- 在串口终端输入数据，数据会通过 SPI 发送到 E22-400M30S
- 可以发送 SX1268 命令来配置和控制 LoRa 模块
- OLED 屏幕实时显示传输状态
  - "USB->LoRa" + "SPI TX": USB 数据通过 SPI 发送到 LoRa

**注意**: 完整的 SX1268 驱动可以根据需求添加，当前实现提供了基本的 SPI 通信框架。

## 项目结构

```
blue-high/
├── src/
│   └── main.rs          # 主程序文件
├── .cargo/
│   └── config.toml      # Cargo 配置
├── Cargo.toml           # 项目依赖
├── memory.x             # 链接器脚本
└── README.md            # 项目说明
```

## 依赖库

- `stm32f1xx-hal`: STM32F1 系列硬件抽象层
- `cortex-m-rt`: Cortex-M 运行时
- `cortex-m`: Cortex-M 核心功能 (启用 critical-section 支持)
- `embedded-hal`: 嵌入式硬件抽象接口
- `ssd1306`: OLED 显示驱动
- `embedded-graphics`: 嵌入式图形库
- `usb-device`: USB 设备支持
- `usbd-serial`: USB CDC 串口类驱动
- `defmt`: 高效日志框架
- `defmt-rtt`: RTT 传输层用于 defmt 日志
- `panic-probe`: 支持 defmt 的 panic 处理器
- `portable-atomic`: 提供原子操作支持

**注意**: SX1268 LoRa 驱动可根据具体需求添加（如 `sx126x` 或 `sx1262` 等）

## 故障排除

### 编译错误

1. 确保已安装 `thumbv7m-none-eabi` 目标
2. 检查 Rust 工具链版本是否最新

### 烧录问题

1. 确认 ST-Link 连接正常
2. 检查 USB 权限设置
3. 尝试重启开发板

### 显示问题

1. 检查 I2C 接线是否正确
2. 确认 OLED 地址 (通常为 0x3C 或 0x3D)
3. 检查供电电压

### LoRa 通信问题

1. 检查 SPI 接线（SCK、MISO、MOSI、NSS）
2. 确认天线已正确连接
3. 检查控制引脚（BUSY、DIO1、NRST）
4. 使用逻辑分析仪验证 SPI 通信
5. 确认模块供电正常（3.3V）
6. 检查 SPI 时钟频率设置

### USB 连接问题

1. 确认 USB 线缆支持数据传输（非仅充电线）
2. 检查 PC 是否识别到 USB 设备
3. Windows 用户可能需要安装 USB CDC 驱动
4. 检查防火墙或安全软件是否阻止 USB 设备
5. 尝试更换 USB 端口或重新插拔

## 许可证

本项目采用 MIT 许可证。详见 LICENSE 文件。

## 贡献

欢迎提交 Issue 和 Pull Request！