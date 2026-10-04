//! `--crash-test KIND`: fails on purpose eight seconds after the game starts
//! (its window is up by then), to see the crash window over a running game. `panic` panics on the main
//! thread, `sim` on a thread whose death ends the game (as the sim thread's
//! does), `native` raises an access violation (Windows), `error` ends the game
//! with an error.

use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Drill {
    Panic,
    Sim,
    Native,
    Error,
}

impl Drill {
    pub fn parse(s: &str) -> Option<Drill> {
        Some(match s {
            "panic" => Drill::Panic,
            "sim" => Drill::Sim,
            "native" => Drill::Native,
            "error" => Drill::Error,
            _ => return None,
        })
    }
}

/// How long after the window opens the drill fails.
const AFTER: Duration = Duration::from_secs(8);

static ARMED: Mutex<Option<(Drill, Instant)>> = Mutex::new(None);

/// Arms the drill; the clock starts now.
pub fn arm(drill: Drill) {
    if let Ok(mut armed) = ARMED.lock() {
        *armed = Some((drill, Instant::now() + AFTER));
    }
    if matches!(drill, Drill::Sim | Drill::Native) {
        let _ = std::thread::Builder::new()
            .name("crash-test".into())
            .spawn(move || {
                let _fatal = super::FatalGuard;
                std::thread::sleep(AFTER);
                match drill {
                    #[cfg(windows)]
                    Drill::Native => super::native::raise_access_violation(),
                    _ => panic!("--crash-test sim: a panic on a thread the game needs"),
                }
            });
    }
}

/// Called every frame on the main thread: the main-thread drills fire here.
/// `Err` is the drill's error, for the caller to end the game with.
pub fn tick() -> Result<(), String> {
    let Ok(armed) = ARMED.lock() else {
        return Ok(());
    };
    let Some((drill, at)) = *armed else {
        return Ok(());
    };
    if Instant::now() < at {
        return Ok(());
    }
    drop(armed);
    match drill {
        Drill::Panic => panic!("--crash-test panic: a panic on the main thread"),
        Drill::Error => {
            if let Ok(mut armed) = ARMED.lock() {
                *armed = None;
            }
            Err("--crash-test error: the game stopped with an error".into())
        }
        Drill::Sim | Drill::Native => Ok(()),
    }
}
