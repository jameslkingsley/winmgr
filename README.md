# Low-level window manager for Windows

This is an incredibly basic window "manager" that lets you bind simple key combinations to reposition and resize the foreground window instantly. You can choose from the predefined layouts or use a custom one by defining the coordinates and dimensions. Modifiers and key-codes are defined using their raw numeric values. This is an intentionally lazy design decision to allow users to assign potentially dubious key-codes at their own risk.

## Install from source

Clone the repo and build it with `cargo build --release`.

You may need to add the executable to your anti-virus exclusions list.

The binary (`target/release/winmgr.exe`) provides three commands:

### Install

Adds a registry entry to run the executable on startup.

```bash
./target/release/winmgr.exe install
```

### Uninstall

Removes the aforementioned registry entry.

```bash
./target/release/winmgr.exe uninstall
```

### Run

Runs the application in the background.

**Note: On first run a config file will be written to `$HOME/winmgr.json`.**

```bash
./target/release/winmgr.exe run
```

## Config

Config file is written to `$HOME/winmgr.json`.

### Layouts

#### Predefined layouts

- LeftHalf
- RightHalf
- LeftThird
- RightThird
- LeftTwoThirds
- RightTwoThirds
- CenterThird
- CenterSmall
- CenterMedium
- CenterLarge

#### Custom layout

```json
{
  "keybinds": [
    {
      "modifiers": ["0x2"],
      "key": "0x61",
      "layout": {
        "x": 100,
        "y": 100,
        "w": 1000,
        "h": 1000
      }
    }
  ]
}
```

#### Margin

A margin can be set with the root field `margin`.

```json
{
  "margin": 32
}
```

### Modifiers

- Alt `0x1`
- Control `0x2`
- Shift `0x4`
- Windows `0x8`

### Key codes

See [https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes](https://learn.microsoft.com/en-us/windows/win32/inputdev/virtual-key-codes)

### Example

This example binds the predefined layouts to `Ctrl+Numpad0-9`.

```json
{
  "margin": 32,
  "keybinds": [
    {
      "modifiers": ["0x2"],
      "key": "0x61",
      "layout": "LeftThird"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x62",
      "layout": "CenterThird"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x63",
      "layout": "RightTwoThirds"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x64",
      "layout": "LeftHalf"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x65",
      "layout": "CenterLarge"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x66",
      "layout": "RightHalf"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x67",
      "layout": "LeftTwoThirds"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x68",
      "layout": "CenterSmall"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x69",
      "layout": "RightThird"
    },
    {
      "modifiers": ["0x2"],
      "key": "0x60",
      "layout": "CenterMedium"
    }
  ]
}
```
