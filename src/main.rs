#![allow(unsafe_op_in_unsafe_fn)]
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{env, error::Error, fmt::Write, fs::File, io};

use clap::{Parser, Subcommand};
use directories::UserDirs;
use nohash_hasher::{BuildNoHashHasher, IntMap};
use serde::{Deserialize, Serialize};
use windows::{
    Win32::{
        Foundation::HWND,
        Graphics::Gdi::{
            GetMonitorInfoW, MONITOR_DEFAULTTONEAREST, MONITORINFO, MonitorFromWindow,
        },
        System::DataExchange::GlobalAddAtomA,
        UI::{
            Input::KeyboardAndMouse::*,
            WindowsAndMessaging::{
                GetForegroundWindow, GetMessageW, MSG, SET_WINDOW_POS_FLAGS, SWP_FRAMECHANGED,
                SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WM_HOTKEY, WM_QUIT,
            },
        },
    },
    core::PCSTR,
};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_SET_VALUE},
};

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE_NAME: &str = "WinMgr";

#[derive(Parser)]
#[command(name = "winmgr")]
#[command(about = "Super basic window manager for Windows")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Install WinMgr to run on startup
    Install,

    /// Uninstall startup entry for WinMgr
    Uninstall,

    /// Run WinMgr
    Run,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cli = Args::parse();

    match cli.command {
        Some(Command::Install) => {
            install_autostart()?;
        }
        Some(Command::Uninstall) => {
            uninstall_autostart()?;
        }
        Some(Command::Run) | None => {
            let Some(config) = get_config() else {
                eprintln!("Failed to get config");
                return Ok(());
            };

            let registry = KeyBindRegistry::new(config);

            registry.run();
        }
    }

    Ok(())
}

fn install_autostart() -> io::Result<()> {
    let exe_path = env::current_exe()?;
    let exe_str = exe_path.display().to_string();

    let command = format!("\"{}\" run", exe_str);

    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (run_key, _) = hkcu.create_subkey(RUN_KEY_PATH)?;

    run_key.set_value(RUN_VALUE_NAME, &command)?;
    Ok(())
}

fn uninstall_autostart() -> io::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(run_key) = hkcu.open_subkey_with_flags(RUN_KEY_PATH, KEY_SET_VALUE) {
        let _ = run_key.delete_value(RUN_VALUE_NAME);
    }
    Ok(())
}

fn get_config() -> Option<Config> {
    let Some(dirs) = UserDirs::new() else {
        eprintln!("Failed to get user home directory");
        return None;
    };

    let config_path = dirs.home_dir().join("winmgr.json");

    let config: Config = match config_path.exists() {
        true => serde_json::from_reader(File::open(config_path).ok()?).ok()?,
        false => {
            let new_config = Config::default();
            serde_json::to_writer_pretty(File::create(config_path).ok()?, &new_config).ok()?;
            new_config
        }
    };

    Some(config)
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Config {
    margin: u8,
    keybinds: Vec<KeyBind>,
}

#[derive(Debug, Serialize, Deserialize)]
struct KeyBind {
    modifiers: Vec<HexModifier>,
    key: HexVirtualKey,
    layout: Layout,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HexModifier(pub String);

impl From<&HexModifier> for HOT_KEY_MODIFIERS {
    fn from(value: &HexModifier) -> Self {
        let without_prefix = value.0.trim_start_matches("0x");
        let int = u32::from_str_radix(without_prefix, 16).expect("invalid hex");
        HOT_KEY_MODIFIERS(int)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct HexVirtualKey(pub String);

impl From<&HexVirtualKey> for VIRTUAL_KEY {
    fn from(value: &HexVirtualKey) -> Self {
        let without_prefix = value.0.trim_start_matches("0x");
        let int = u16::from_str_radix(without_prefix, 16).expect("invalid hex");
        VIRTUAL_KEY(int)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(untagged)]
enum Layout {
    Custom(CustomLayout),
    Default(DefaultLayout),
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
enum DefaultLayout {
    LeftHalf,
    RightHalf,
    LeftThird,
    RightThird,
    LeftTwoThirds,
    RightTwoThirds,
    CenterThird,
    CenterSmall,
    CenterMedium,
    CenterLarge,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
struct CustomLayout {
    x: i32,
    y: i32,
    w: i32,
    h: i32,
}

#[derive(Debug, Default)]
struct KeyBindRegistry {
    cfg: Config,
    map: IntMap<usize, usize>,
}

impl DefaultLayout {
    /// Calculate (x, y, w, h)
    fn calc(self, margin: u8, mi: &MONITORINFO) -> (i32, i32, i32, i32) {
        let m = margin as i32;

        let work_left = mi.rcWork.left;
        let work_top = mi.rcWork.top;
        let work_width = mi.rcWork.right - mi.rcWork.left;
        let work_height = mi.rcWork.bottom - mi.rcWork.top;

        // Inner rect after applying outer margin
        let inner_width = (work_width - 2 * m).max(0);
        let inner_height = (work_height - 2 * m).max(0);

        match self {
            DefaultLayout::LeftHalf => {
                let w = inner_width / 2;
                let x = work_left + m;
                let y = work_top + m;
                (x, y, w, inner_height)
            }
            DefaultLayout::RightHalf => {
                let w = inner_width / 2;
                let x = work_left + m + (inner_width - w);
                let y = work_top + m;
                (x, y, w, inner_height)
            }
            DefaultLayout::LeftThird => {
                let w = inner_width / 3;
                let x = work_left + m;
                let y = work_top + m;
                (x, y, w, inner_height)
            }
            DefaultLayout::RightThird => {
                let w = inner_width / 3;
                let x = work_left + m + 2 * w;
                let y = work_top + m;
                (x, y, w, inner_height)
            }
            DefaultLayout::LeftTwoThirds => {
                let w = inner_width * 2 / 3;
                let x = work_left + m;
                let y = work_top + m;
                (x, y, w, inner_height)
            }
            DefaultLayout::RightTwoThirds => {
                let w = inner_width * 2 / 3;
                let x = work_left + m + (inner_width - w);
                let y = work_top + m;
                (x, y, w, inner_height)
            }
            DefaultLayout::CenterThird => {
                let w = inner_width / 3;
                let x = work_left + m + w;
                let y = work_top + m;
                (x, y, w, inner_height)
            }
            DefaultLayout::CenterSmall => {
                let w = inner_width * 2 / 5;
                let h = inner_height * 6 / 12;
                let x = work_left + (work_width - w) / 2;
                let y = work_top + (work_height - h) / 2;
                (x, y, w, h)
            }
            DefaultLayout::CenterMedium => {
                let w = inner_width * 3 / 4;
                let h = inner_height * 9 / 10;
                let x = work_left + (work_width - w) / 2;
                let y = work_top + (work_height - h) / 2;
                (x, y, w, h)
            }
            DefaultLayout::CenterLarge => {
                let w = inner_width;
                let h = inner_height;
                let x = work_left + (work_width - w) / 2;
                let y = work_top + m;
                (x, y, w, h)
            }
        }
    }
}

impl KeyBindRegistry {
    fn new(cfg: Config) -> Self {
        let mut this = Self {
            map: IntMap::with_capacity_and_hasher(cfg.keybinds.len(), BuildNoHashHasher::default()),
            cfg,
        };

        this.register();
        this
    }

    fn register(&mut self) {
        let mut buf = String::new();

        for (index, keybind) in self.cfg.keybinds.iter().enumerate() {
            buf.clear();

            unsafe {
                write!(buf, "winmgr_bind_{index}").unwrap();

                let id = GlobalAddAtomA(PCSTR::from_raw(buf.as_ptr()));

                let mods = keybind
                    .modifiers
                    .iter()
                    .fold(HOT_KEY_MODIFIERS(0), |mut acc, m| {
                        acc |= m.into();
                        acc
                    });

                let key: VIRTUAL_KEY = (&keybind.key).into();

                if let Err(err) = RegisterHotKey(None, id.into(), mods | MOD_NOREPEAT, key.0.into())
                {
                    eprintln!("Failed to register keybind {buf}: {err}");
                    continue;
                }

                self.map.insert(id.into(), index);
            }
        }
    }

    fn run(&self) {
        unsafe {
            let mut msg: MSG = MSG::default();

            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_QUIT {
                    break;
                }

                if msg.message == WM_HOTKEY {
                    let hotkey_id = msg.wParam.0;

                    let Some(idx) = self.map.get(&hotkey_id) else {
                        eprintln!("Hotkey {hotkey_id} is not registered");
                        continue;
                    };

                    let kb = &self.cfg.keybinds[*idx];

                    let hwnd: HWND = GetForegroundWindow();

                    if hwnd.is_invalid() {
                        // No active window
                        continue;
                    }

                    let monitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);

                    let mut mi = MONITORINFO {
                        cbSize: size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };

                    if !GetMonitorInfoW(monitor, &mut mi).as_bool() {
                        // Could not query monitor info
                        eprintln!("Could not query monitor info");
                        continue;
                    }

                    let (x, y, w, h) = match kb.layout {
                        Layout::Custom(layout) => (layout.x, layout.y, layout.w, layout.h),
                        Layout::Default(layout) => layout.calc(self.cfg.margin, &mi),
                    };

                    let flags: SET_WINDOW_POS_FLAGS =
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED;

                    SetWindowPos(hwnd, None, x, y, w, h, flags).unwrap();
                }
            }
        }
    }
}
