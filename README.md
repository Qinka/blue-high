# blue-high

STM32F103C8T6 MCU 控制程序 - OLED 和 LoRa 控制器

## 项目概述

这是一个使用 Rust 语言开发的 STM32F103C8T6 微控制器程序，能够同时控制：
- 0.96寸 OLED 屏幕 (SSD1306，通过 I2C 接口)
- 亿佰特 E22-400M30S LoRa 无线模块 (通过 UART 接口)
- USB CDC 虚拟串口 (用于 PC 与 LoRa 之间的透传)

## 硬件连接

### OLED 显示屏 (I2C)
- SCL -> PB6
- SDA -> PB7
- VCC -> 3.3V
- GND -> GND

### E22-400M30S LoRa 模块 (UART)
- TXD -> PA10 (STM32 RX)
- RXD -> PA9 (STM32 TX)
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
   - 显示 USB 和 LoRa 数据传输状态

2. **USB CDC 虚拟串口**
   - 作为 USB 从设备连接到 PC
   - 创建虚拟 COM 端口
   - 支持标准串口通信

3. **LoRa 通信**
   - 使用亿佰特 E22-400M30S 模块
   - UART 通信，波特率 9600
   - 支持透明传输模式

4. **数据透传**
   - USB <-> LoRa 双向透传
   - PC 通过 USB 串口发送数据到 LoRa
   - LoRa 接收数据通过 USB 发送到 PC
   - 实时显示传输状态

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

## 使用方法

### 1. 连接设备

1. 通过 ST-Link 烧录程序到 STM32F103C8T6
2. 连接 OLED 显示屏到 I2C 接口 (PB6/PB7)
3. 连接 E22-400M30S LoRa 模块到 UART1 (PA9/PA10)
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

### 4. 数据透传

- 在串口终端输入数据，数据会通过 LoRa 发送
- LoRa 接收的数据会显示在串口终端
- OLED 屏幕实时显示传输方向和状态
  - "USB->LoRa" + "TX OK": USB 数据发送到 LoRa
  - "LoRa->USB" + "RX OK": LoRa 数据接收并发送到 USB

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
- `usb-device`: USB 设备支持
- `usbd-serial`: USB CDC 串口类驱动
- `nb`: 非阻塞 I/O 支持
- `panic-halt`: 简单的 panic 处理

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

1. 检查 UART 接线（TXD 连 RX，RXD 连 TX）
2. 确认天线已正确连接
3. 检查波特率设置（默认 9600）
4. 使用串口调试工具测试 E22 模块
5. 确认模块供电正常（3.3V）

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