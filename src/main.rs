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
use winit::keyboard::KeyCode;
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_SET_VALUE},
};

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

const RUN_KEY_PATH: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE_NAME: &str = "WinMgr";

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
    modifier: KeyCode,
    key: KeyCode,
    layout: Layout,
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
                let h = inner_height * 9 / 12;
                let x = work_left + (work_width - w) / 2;
                let y = work_top + (work_height - h) / 2;
                (x, y, w, h)
            }
            DefaultLayout::CenterMedium => {
                let w = inner_width * 2 / 3;
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
                debug_assert_eq!(buf, format!("winmgr_bind_{index}"));

                let id = GlobalAddAtomA(PCSTR::from_raw(buf.as_ptr()));

                let Some(modifier) = keybind.modifier_to_hk_modifier() else {
                    continue;
                };

                let Some(keycode) = keybind.key_to_virtual_key() else {
                    continue;
                };

                if let Err(err) =
                    RegisterHotKey(None, id.into(), modifier | MOD_NOREPEAT, keycode.0.into())
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

                    let flags: SET_WINDOW_POS_FLAGS =
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_FRAMECHANGED;

                    let (x, y, w, h) = match kb.layout {
                        Layout::Custom(layout) => (layout.x, layout.y, layout.w, layout.h),
                        Layout::Default(layout) => layout.calc(self.cfg.margin, &mi),
                    };

                    SetWindowPos(hwnd, None, x, y, w, h, flags).unwrap();
                }
            }
        }
    }
}

impl KeyBind {
    fn key_to_virtual_key(&self) -> Option<VIRTUAL_KEY> {
        Some(match self.key {
            KeyCode::Digit0 => VK_0,
            KeyCode::Digit1 => VK_1,
            KeyCode::Digit2 => VK_2,
            KeyCode::Digit3 => VK_3,
            KeyCode::Digit4 => VK_4,
            KeyCode::Digit5 => VK_5,
            KeyCode::Digit6 => VK_6,
            KeyCode::Digit7 => VK_7,
            KeyCode::Digit8 => VK_8,
            KeyCode::Digit9 => VK_9,
            KeyCode::Numpad0 => VK_NUMPAD0,
            KeyCode::Numpad1 => VK_NUMPAD1,
            KeyCode::Numpad2 => VK_NUMPAD2,
            KeyCode::Numpad3 => VK_NUMPAD3,
            KeyCode::Numpad4 => VK_NUMPAD4,
            KeyCode::Numpad5 => VK_NUMPAD5,
            KeyCode::Numpad6 => VK_NUMPAD6,
            KeyCode::Numpad7 => VK_NUMPAD7,
            KeyCode::Numpad8 => VK_NUMPAD8,
            KeyCode::Numpad9 => VK_NUMPAD9,
            other => {
                eprintln!("Unsupported key: {other:?}");
                return None;
            }
        })
    }

    fn modifier_to_hk_modifier(&self) -> Option<HOT_KEY_MODIFIERS> {
        match self.modifier {
            KeyCode::AltLeft | KeyCode::AltRight => Some(MOD_ALT),
            KeyCode::ControlLeft | KeyCode::ControlRight => Some(MOD_CONTROL),
            KeyCode::SuperLeft | KeyCode::SuperRight => Some(MOD_WIN),
            KeyCode::ShiftLeft | KeyCode::ShiftRight => Some(MOD_SHIFT),
            other => {
                eprintln!("Invalid modifier: {other:?}");
                None
            }
        }
    }
}
