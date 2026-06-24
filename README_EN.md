# ClipSync - Cross-Platform Clipboard Sync Tool

[中文](README.md) | **English**

Automatically discover devices on the local network and sync clipboard text, images, and files in real time. Supports Mac and Windows interoperability with 9 built-in languages.

## Features

- **Auto Discovery**: Automatically discovers other devices on the same local network via UDP broadcast
- **Real-time Sync**: Automatically syncs copied text to all connected devices
- **Message Deduplication**: Smart loop prevention to avoid content echoing back and forth
- **History Records**: Keeps the last 20 clipboard entries, click to copy
- **Global Hotkey**: Configurable shortcut to toggle window visibility (default CmdOrCtrl+Shift+V)
- **Local Record Display**: Locally copied text also appears in the history list for easy reference
- **Inline Copy Button**: Each history entry has its own copy button for quick reuse
- **File/Folder Sync**: Automatically transfers copied files and folders to other devices
- **Image Sync**: Real-time cross-device sync of in-memory images such as screenshots
- **Multi-language UI**: Supports Chinese, English, Japanese, Korean, German, French, Spanish, Portuguese, and Russian
- **System Tray**: Runs minimized in the system tray, accessible anytime
- **Custom Download Directory**: Customize the file download save location
- **Cross-Platform**: Mac and Windows interoperability

## Screenshots

| Clipboard History | Device List | Network Management |
|:---:|:---:|:---:|
| ![Clipboard History](website/images/image1.png) | ![Device List](website/images/image2.png) | ![Network Management](website/images/image3.png) |

## Requirements

### General Requirements
- **Node.js** v18+ (LTS recommended)
- **Rust** v1.75+ (install via [rustup](https://rustup.rs/))
- **Git** (for cloning the project)

### Operating System Requirements
- **macOS**: 10.15+ (Catalina or later)
- **Windows**: 10+ (64-bit)

### Build Tools

**macOS requires:**
- Xcode Command Line Tools
  ```bash
  xcode-select --install
  ```

**Windows requires:**
- Visual Studio C++ Build Tools (or Visual Studio 2019+)
  - Download [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
  - Check "Desktop development with C++" during installation
- WebView2 (usually pre-installed on Windows 10 1809+)

## Installation

### Option 1: Build from Source

#### Prerequisites Setup

**macOS Setup:**

1. **Install Homebrew** (if not already installed):
   ```bash
   /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
   ```

2. **Install Node.js**:
   ```bash
   brew install node
   ```

3. **Install Rust**:
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source "$HOME/.cargo/env"
   ```

4. **Install Xcode Command Line Tools**:
   ```bash
   xcode-select --install
   ```

5. **Verify installation**:
   ```bash
   node --version    # Should show v18 or higher
   rustc --version   # Should show 1.75 or higher
   ```

**Windows Setup:**

1. **Install Node.js**:
   - Visit [Node.js official website](https://nodejs.org/)
   - Download and install the LTS version (64-bit recommended)
   - Restart terminal after installation

2. **Install Rust**:
   - Download [rustup-init.exe](https://win.rustup.rs/x86_64)
   - Run the installer and select default options
   - Restart terminal after installation

3. **Install Visual Studio Build Tools**:
   - Download [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
   - Run the installer
   - Check "Desktop development with C++" under "Workloads"
   - Click install

4. **Verify installation** (in PowerShell or CMD):
   ```powershell
   node --version    # Should show v18 or higher
   rustc --version   # Should show 1.75 or higher
   ```

#### Project Setup

**1. Clone the project**

```bash
git clone <repository-url>
cd clip-sync
```

**2. Install dependencies**

```bash
npm install
```

**3. Run in development**

```bash
npm run tauri dev
```

The first run will automatically download Rust dependencies and compile, which may take a few minutes.

**4. Build for release**

**macOS build:**
```bash
npm run tauri build
```
Build artifacts location:
- DMG installer: `src-tauri/target/release/bundle/dmg/`
- APP application: `src-tauri/target/release/bundle/macos/`

**Windows build:**
```bash
npm run tauri build
```
Build artifacts location:
- MSI installer: `src-tauri/target/release/bundle/msi/`
- NSIS installer: `src-tauri/target/release/bundle/nsis/`

### Option 2: Direct Install (Recommended for regular users)

**macOS installation:**
1. Download the latest `.dmg` file from the Releases page
2. Double-click the DMG file to open
3. Drag ClipSync to the Applications folder
4. Run from Launchpad or Applications

**Windows installation:**
1. Download the latest `.msi` or `.exe` installer from the Releases page
2. Double-click the installer to run
3. Follow the installation wizard to complete setup
4. Run from the Start menu or desktop shortcut

## Usage

### Basic Operations

1. **Launch the app**: ClipSync runs in the background after launch, displayed in the system tray
2. **Open window**: Click the tray icon or use the global hotkey (default CmdOrCtrl+Shift+V) to open the main window
3. **View devices**: Switch to the "Device List" tab to see discovered devices
4. **View history**: Switch to the "Clipboard History" tab to see sync records
5. **Copy content**: Click the "Copy" button next to a history entry, or click anywhere on an entry to copy it to your local clipboard
6. **Modify settings**: Switch to the "Settings" tab to change the download directory and global hotkey

### Device Connection

**Prerequisites:**
- Both devices must be on the **same local network** (connected to the same Wi-Fi or router)
- Both devices must have ClipSync running

**Connection flow:**
1. Launch ClipSync on both Mac and Windows
2. The app automatically discovers each other via UDP broadcast (takes about 2-5 seconds)
3. After discovery, a WebSocket connection is automatically established
4. Once the status shows "Connected", syncing begins

### Clipboard Sync

- Text copied on any device will automatically sync to all connected devices
- **Local records are also kept in the history list** for easy reference
- Received remote clipboard content appears in "Clipboard History"
- Each entry has its own "Copy" button for quick copying to the local clipboard
- You can also click anywhere on a history entry to copy its content
- History keeps up to 20 entries

### File and Folder Sync

- After copying files or folders locally, they will be automatically broadcast to all connected devices
- The receiving device saves the files and folders in the download directory
- **Single file transfer**: Automatically written to the clipboard after receiving, ready for direct use
- **Multi-file/folder transfer**: Not written to the clipboard after receiving, but you can open the file location from the history
- Supports recursive copying of entire folders while preserving the original directory structure
- History entries show file size, source device, transfer status, and more

### Global Hotkey

- Default hotkey: `CmdOrCtrl+Shift+V`
- You can customize the hotkey in the "Settings" page
- Press the hotkey to quickly show/hide the main window
- Hotkey changes take effect immediately

### System Tray Operations

- **Left-click**: Show/hide the main window
- **Right-click**: Pop up menu (Show window / Quit app)

## Network Ports

ClipSync uses the following ports. If blocked by a firewall, please allow them:

| Port | Protocol | Purpose |
|------|----------|---------|
| 37020 | UDP | Device discovery (broadcast) |
| 37021 | TCP | WebSocket data transfer |

## Project Structure

```
clip-sync/
├── package.json              # Frontend dependency config
├── tsconfig.json             # TypeScript config
├── vite.config.ts            # Vite build config
├── index.html                # Entry HTML
├── src/
│   ├── main.tsx              # React entry
│   ├── App.tsx               # Main UI component
│   ├── components/
│   │   ├── StatusBar.tsx     # Status bar (IP/connections)
│   │   ├── DeviceList.tsx    # Device list
│   │   └── ClipHistory.tsx   # Clipboard history
│   └── styles/
│       └── app.css           # Global styles (dark theme)
└── src-tauri/
    ├── Cargo.toml            # Rust dependency config
    ├── tauri.conf.json       # Tauri app config
    ├── build.rs              # Tauri build script
    ├── capabilities/default.json  # Tauri v2 permissions config
    ├── icons/                # App icons
    └── src/
        ├── main.rs           # Program entry
        ├── lib.rs            # Library entry, module wiring
        ├── state.rs          # Global state management
        ├── discovery.rs      # UDP broadcast device discovery
        ├── transport.rs      # WebSocket P2P communication
        ├── clipboard.rs      # Clipboard monitoring and writing
        ├── commands.rs       # Tauri command definitions
        └── tray.rs           # System tray
```

## Tech Stack

| Layer | Technology |
|-------|-----------|
| Frontend Framework | React 18 + TypeScript |
| Build Tool | Vite 6 |
| Desktop Framework | Tauri v2 |
| Async Runtime | Tokio |
| Communication Protocol | WebSocket (tokio-tungstenite) |
| Device Discovery | UDP Broadcast |
| Clipboard Operations | arboard |

## FAQ

### Q: Why don't I see other devices in the device list?

1. Confirm both devices are on the same local network
2. Confirm the firewall is not blocking UDP port 37020
3. Restart ClipSync on both devices
4. Wait 5-10 seconds for device discovery to take effect

### Q: Copied content is not syncing to other devices?

1. Check if the device list status shows "Connected"
2. Confirm the firewall is not blocking TCP port 37021
3. Try manually copying on the target device
4. Check if local copy records appear in "Clipboard History" (if not, clipboard monitoring may have an issue)

### Q: Does it support image or file sync?

The current version supports **plain text** clipboard sync, **image** sync (including in-memory images like screenshots), and **file/folder** auto-transfer.

### Q: Can I use it across different network environments?

The current version only supports **local network** device discovery. Remote sync requires additional relay server configuration, which is not currently supported.

### Q: How do I view runtime logs?

In development mode, logs are output to the terminal:

```bash
RUST_LOG=debug npm run tauri dev
```

In production mode, logs are not output by default but can be enabled via environment variables.

## Development Guide

### Common Commands

```bash
# Development mode (hot reload)
npm run tauri dev

# Frontend only (without compiling Rust)
npm run dev

# Build for release
npm run tauri build

# Clean build cache
cargo clean -p clip-sync
```

### Debugging Tips

- Open DevTools: Right-click in the Tauri window → Inspect Element
- View Rust logs: Set `RUST_LOG=debug` in the terminal
- Network capture: Use Wireshark to inspect UDP broadcasts and WebSocket connections

## License

MIT License
