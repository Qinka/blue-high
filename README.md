# blue-high

STM32F103C8T6 MCU 控制程序 - OLED 和 LoRa 控制器

## 项目概述

这是一个使用 Rust 语言开发的 STM32F103C8T6 微控制器程序，能够同时控制：
- 0.96寸 OLED 屏幕 (SSD1306，通过 I2C 接口)
- 亿佰特 E22-400M30S LoRa 无线模块 (SX1268 芯片，通过 SPI 接口)

## 硬件连接

### OLED 显示屏 (I2C)
- SCL -> PB6
- SDA -> PB7
- VCC -> 3.3V
- GND -> GND

### E22-400M30S LoRa 模块 (SPI)
- MISO -> PA6
- MOSI -> PA7
- SCK -> PA5
- NSS -> PA4
- BUSY -> PA3
- DIO1 -> PA2
- NRST -> PA1
- VCC -> 3.3V
- GND -> GND

## 功能特性

1. **OLED 显示**
   - 初始化 SSD1306 OLED 显示器
   - 显示系统状态信息
   - 显示 LoRa 发送计数器

2. **LoRa 通信**
   - 使用亿佰特 E22-400M30S 模块（基于 SX1268 芯片）
   - SPI 通信接口
   - 支持 LoRa 调制
   - 周期性发送测试消息

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
# 安装 probe-run 用于调试和烧录
cargo install probe-run

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

### 使用 probe-run (推荐)

```bash
cargo run --release
```

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
- `embedded-hal`: 嵌入式硬件抽象接口
- `ssd1306`: OLED 显示驱动
- `embedded-graphics`: 嵌入式图形库
- `panic-halt`: 简单的 panic 处理

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

1. 检查 SPI 接线（MISO、MOSI、SCK、NSS）
2. 确认天线已正确连接
3. 检查 BUSY、DIO1、NRST 引脚连接
4. 使用逻辑分析仪验证 SPI 通信
5. 检查 SPI 模式和时钟频率设置
6. 确认模块供电正常（3.3V）

## 许可证

本项目采用 MIT 许可证。详见 LICENSE 文件。

## 贡献

欢迎提交 Issue 和 Pull Request！