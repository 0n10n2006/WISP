//! One long-lived background thread for Windows COM/WinRT work (audio,
//! media sessions, radios). Objects are created once on this thread and
//! reused, instead of initialising COM and WinRT managers on every call.

use std::sync::mpsc::{channel, Sender};
use std::sync::{Mutex, OnceLock};
use wisp_core::{Result, WispError};

type Job = Box<dyn FnOnce() + Send>;

fn sender() -> &'static Mutex<Sender<Job>> {
    static TX: OnceLock<Mutex<Sender<Job>>> = OnceLock::new();
    TX.get_or_init(|| {
        let (tx, rx) = channel::<Job>();
        std::thread::Builder::new()
            .name("wisp-com".into())
            .spawn(move || {
                #[cfg(windows)]
                unsafe {
                    use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
                    let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
                }
                for job in rx {
                    job();
                }
            })
            .expect("spawn COM worker");
        Mutex::new(tx)
    })
}

/// Run `f` on the COM worker and wait for its result.
pub async fn run<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    sender()
        .lock()
        .expect("com worker")
        .send(Box::new(move || {
            let _ = tx.send(f());
        }))
        .map_err(|_| WispError::internal("Background worker stopped"))?;
    rx.await
        .map_err(|_| WispError::internal("Background worker stopped"))
}

#[cfg(test)]
mod tests {
    #[test]
    fn runs_jobs_on_one_thread() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        let a = rt
            .block_on(super::run(|| std::thread::current().id()))
            .unwrap();
        let b = rt
            .block_on(super::run(|| std::thread::current().id()))
            .unwrap();
        assert_eq!(a, b);
        assert_ne!(a, std::thread::current().id());
    }
}
