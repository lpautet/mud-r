# MUD-R 🎮

A modern Rust port of the classic CircleMUD text-based multiplayer online role-playing game (MUD). This project brings the timeless gameplay of CircleMUD to the modern era with memory safety, type safety, performance improvements, and enhanced connectivity options.

## ✨ Features

- **🦀 Modern Rust Implementation**: Memory-safe and performant rewrite with idiomatic Rust patterns
- **🌐 Dual Protocol Support**: Seamless support for both traditional telnet and modern WebSocket connections
- **🔧 Professional CLI**: Clean command-line interface using `clap` with comprehensive help and validation
- **🏠 Complete MUD Systems**: Combat, magic, shops, housing, player progression, guilds, and social systems
- **🔐 Secure Authentication**: PBKDF2 password hashing with salt and secure session management
- **📊 Structured Logging**: Professional logging with `log4rs` and configurable output levels
- **⚡ High-Performance Architecture**: Efficient pulse-based game loop with optimized data structures
- **🛡️ Type Safety**: Extensive use of Rust's type system for compile-time safety guarantees
- **🔄 Bitflags Integration**: Type-safe flag systems replacing raw integer constants

## 🚀 Quick Start

### Prerequisites

- Rust 1.70+ (2021 edition)
- Cargo package manager
- Git (for cloning the repository)

### Building

```bash
git clone https://github.com/lpautet/mud-r.git
cd mud-r
cargo build --release
```

### Running the Server

```bash
# Start with default settings (port 4000)
cargo run

# Or use the built binary with custom options
./target/release/mudr --help

# For production deployment, use the autorun script
./autorun
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

- `main.rs`: Server initialization, networking, and main game loop
- `interpreter.rs`: Command parsing, execution, and player input handling
- `db.rs`: World database operations and data persistence
- `structs.rs`: Core data structures with type-safe enums and bitflags
- `act_*.rs`: Action handlers for different game systems (combat, communication, movement)
- `spells.rs` & `magic.rs`: Magic system and spell implementations
- `fight.rs`: Combat mechanics and damage calculations
- `depot.rs`: Custom memory management system for game objects

## 🔧 Development

### Code Quality

This project follows modern Rust best practices:

- **Type Safety**: Extensive use of enums, `Option<T>`, and `Result<T, E>`
- **Memory Safety**: No unsafe code, leveraging Rust's ownership system
- **Error Handling**: Proper error propagation and graceful failure handling
- **Code Style**: Consistent formatting with `rustfmt` and linting with `clippy`
- **Documentation**: Comprehensive inline documentation and examples

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

- **clap**: Modern command-line argument parsing with derive macros
- **tungstenite**: WebSocket server implementation for web client support
- **log4rs**: Structured logging framework with file and console output
- **signal-hook**: Unix signal handling for graceful shutdown
- **dns-lookup**: Hostname resolution for connection logging
- **pbkdf2**: Secure password hashing with HMAC-SHA2
- **chrono**: Date and time handling for game events
- **regex**: Pattern matching for text processing
- **rand**: Cryptographically secure random number generation
- **bitflags**: Type-safe bitfield flags for game state

## 📝 License

This project maintains the original CircleMUD licensing terms. See the source files for complete license information.

**Original CircleMUD Copyright**: (C) 1993, 94 by the Trustees of the Johns Hopkins University  
**Rust Port Copyright**: (C) 2023, 2024 Laurent Pautet

## 🧪 Testing

Run the test suite to ensure code quality:

```bash
# Run all tests
cargo test

# Run tests with output
cargo test -- --nocapture

# Run specific test module
cargo test interpreter::tests
```

## 📊 Performance

MUD-R is designed for high performance:

- **Memory Efficiency**: Custom depot system for optimal object management
- **Network Performance**: Non-blocking I/O with efficient connection handling
- **Game Loop Optimization**: Precise timing with minimal CPU usage
- **Type-Level Optimizations**: Zero-cost abstractions using Rust's type system

## 🤖 Production Deployment

### Autorun Script

For production environments, use the included `autorun` script for automatic server management:

```bash
# Make the script executable
chmod +x autorun

# Start the server with automatic restart capability
./autorun
```

**Autorun Features:**
- **Automatic Restart**: Restarts the server if it crashes
- **Log Management**: Automatically rotates and categorizes log files
- **Control Files**: Use special files to control server behavior:
  - `.fastboot`: Quick restart (5 seconds instead of 60)
  - `.killscript`: Permanently stop the server
  - `pause`: Temporarily pause restarts

**Control Commands from within the MUD:**
```
shutdown reboot    # Quick restart
shutdown die       # Permanent shutdown
shutdown pause     # Pause until manual restart
```

### Configuration

Edit the `autorun` script to customize:
- `PORT=4000`: Default server port
- `FLAGS='-q'`: Server startup flags
- `BACKLOGS=6`: Number of log files to retain

## 🔍 Monitoring

The server provides comprehensive logging and monitoring:

```bash
# View real-time logs (when using autorun)
tail -f syslog

# View current session logs
tail -f syslog.BOOT

# Monitor connections
grep "connection" syslog

# Check for errors
grep "ERROR" syslog
```

Log files are organized by category:
- `syslog`: Current session logs (with autorun)
- `log/syslog.#`: Rotated system logs
- `log/newplayers`: New player registrations
- `log/godcmds`: Administrative commands
- `log/badpws`: Failed login attempts
- `log/errors`: System errors
- `log/levels`: Player level advances
- `log/usage`: Server usage statistics

## 🤝 Contributing

Contributions are welcome! This project aims to preserve the classic MUD experience while leveraging Rust's modern features for improved performance and safety.

### Development Guidelines

1. **Code Style**: Follow `rustfmt` and address `clippy` warnings
2. **Type Safety**: Prefer enums and type-safe patterns over raw constants
3. **Error Handling**: Use `Result<T, E>` for fallible operations
4. **Documentation**: Add doc comments for public APIs
5. **Testing**: Include unit tests for new functionality

### Submitting Changes

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Make your changes following the guidelines above
4. Run tests (`cargo test`)
5. Commit your changes (`git commit -m 'Add amazing feature'`)
6. Push to the branch (`git push origin feature/amazing-feature`)
7. Open a Pull Request

## 🎮 Getting Started as a Player

1. **Create a Character**: Connect and follow the character creation prompts
2. **Learn the Basics**: Use `help newbie` for new player guidance
3. **Explore the World**: Start with `look`, `north/south/east/west` to move around
4. **Get Help**: Use `help` for command assistance, `who` to see other players
5. **Join the Community**: Use channels like `gossip` to chat with other players

## 🏆 Status

**Current Version**: 1.0.0-beta  
**Rust Edition**: 2021  
**Stability**: Production Ready  
**Maintenance**: Actively Maintained  

Welcome to the world of MUD-R! 🌟
