# MUD-R 🎮

A modern Rust port of the classic CircleMUD text-based multiplayer online role-playing game (MUD). This project brings the timeless gameplay of CircleMUD to the modern era with memory safety, performance improvements, and enhanced connectivity options.

## ✨ Features

- **🦀 Rust Implementation**: Memory-safe and performant rewrite of CircleMUD
- **🌐 Dual Connectivity**: Support for both traditional telnet and modern WebSocket connections
- **🔧 Modern CLI**: Clean command-line interface with comprehensive help and validation
- **🏠 Complete MUD Systems**: Combat, magic, shops, housing, player progression, and more
- **🔐 Secure Authentication**: PBKDF2 password hashing with salt
- **📊 Comprehensive Logging**: Structured logging with configurable output
- **⚡ Event-Driven Architecture**: Efficient pulse-based game loop

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ (2021 edition)
- Cargo package manager

### Building

```bash
git clone <repository-url>
cd mud-r
cargo build --release
```

### Running the Server

```bash
# Start with default settings (port 4000)
cargo run

# Or use the built binary with custom options
./target/release/mudr --help
```

## 🎯 Usage

### Command Line Options

```bash
CircleMUD server

Usage: mudr [OPTIONS] [PORT]

Arguments:
  [PORT]  Port number to listen on (must be > 1024)

Options:
  -c, --check            Enable syntax check mode
  -d, --dir <DIRECTORY>  Specify library directory [default: lib]
  -m, --mini             Start in mini-MUD mode
  -o, --log <LOG_FILE>   Write log to file instead of stderr
  -q, --quick            Quick boot (doesn't scan rent for object limits)
  -r, --restrict         Restrict MUD -- no new players allowed
  -s, --no-specials      Suppress special procedure assignments
  -h, --help             Print help
```

### Examples

```bash
# Start server on port 8080 with custom library directory
./target/release/mudr --dir /path/to/mudlib 8080

# Run in mini-MUD mode with quick boot
./target/release/mudr --mini --quick

# Enable syntax checking mode
./target/release/mudr --check

# Log to file and restrict new players
./target/release/mudr --log mud.log --restrict
```

## 🌐 Connecting to the Game

### Traditional Telnet Client

```bash
telnet localhost 4000
```

### Web Browser Client

1. Start the MUD server (WebSocket listener automatically starts on port 4001)
2. Open `web_client.html` in your browser
3. Connect to `ws://localhost:4001`

The web client features:
- Terminal-like interface with ANSI color support
- Scrollable command history
- Modern web technologies for enhanced user experience

## 🏗️ Architecture

### Core Components

- **Game Loop**: Central event-driven game loop with pulse-based timing
- **Depot System**: Custom memory management for game objects
- **Connection Handling**: Dual support for TCP (telnet) and WebSocket connections
- **Database Layer**: File-based world data with efficient loading/saving
- **Command System**: Extensible command interpreter with role-based permissions

### Key Modules

- `main.rs`: Server initialization and main game loop
- `handler.rs`: Player connection and character management
- `interpreter.rs`: Command parsing and execution
- `db.rs`: World database operations
- `act_*.rs`: Action handlers for different game systems
- `spells.rs` & `magic.rs`: Magic and spell systems
- `fight.rs`: Combat mechanics

## 🔧 Development

### Project Structure

```
src/
├── main.rs              # Entry point and game loop
├── config.rs            # Configuration constants
├── structs.rs           # Core data structures
├── db.rs               # Database operations
├── handler.rs          # Connection management
├── interpreter.rs      # Command system
├── act_*.rs           # Action handlers
├── spells.rs          # Magic system
└── ...                # Additional game systems

lib/
├── world/             # World files (rooms, objects, mobs)
├── text/              # Game text and help files
└── plr*/             # Player data directories
```

### Dependencies

- **clap**: Modern command-line argument parsing
- **tungstenite**: WebSocket server implementation  
- **log4rs**: Structured logging framework
- **pbkdf2**: Secure password hashing
- **chrono**: Date and time handling
- **regex**: Pattern matching
- **rand**: Random number generation

## 📝 License

This project maintains the original CircleMUD licensing terms. See the source files for complete license information.

**Original CircleMUD Copyright**: (C) 1993, 94 by the Trustees of the Johns Hopkins University  
**Rust Port Copyright**: (C) 2023, 2024 Laurent Pautet

## 🤝 Contributing

Contributions are welcome! This project aims to preserve the classic MUD experience while leveraging Rust's modern features for improved performance and safety.

## 🎮 Getting Started as a Player

1. **Create a Character**: Connect and follow the character creation prompts
2. **Learn the Basics**: Use `help newbie` for new player guidance
3. **Explore the World**: Start with `look`, `north/south/east/west` to move around
4. **Get Help**: Use `help` for command assistance, `who` to see other players

Welcome to the world of MUD-R! 🌟
