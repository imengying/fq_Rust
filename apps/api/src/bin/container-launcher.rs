use signal_hook::consts::signal::{SIGINT, SIGTERM};
use signal_hook::iterator::Signals;
use std::env;
use std::io;
use std::process::{Child, Command, ExitCode, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[derive(Default)]
struct ChildProcesses {
    sidecar: Option<Child>,
    api: Option<Child>,
}

fn main() -> ExitCode {
    set_defaults();

    let processes = Arc::new(Mutex::new(ChildProcesses::default()));
    if let Err(error) = install_signal_handler(processes.clone()) {
        eprintln!("failed to install signal handler: {error}");
        return ExitCode::from(1);
    }

    {
        let mut guard = processes.lock().expect("launcher mutex poisoned");
        match spawn_sidecar() {
            Ok(child) => guard.sidecar = Some(child),
            Err(error) => {
                eprintln!("failed to start sidecar: {error}");
                return ExitCode::from(1);
            }
        }
    }

    thread::sleep(Duration::from_secs(2));
    {
        let mut guard = processes.lock().expect("launcher mutex poisoned");
        if let Some(sidecar) = guard.sidecar.as_mut() {
            match sidecar.try_wait() {
                Ok(Some(status)) => {
                    eprintln!("sidecar exited early with status {status}");
                    return ExitCode::from(status.code().unwrap_or(1) as u8);
                }
                Ok(None) => {}
                Err(error) => {
                    eprintln!("failed to inspect sidecar status: {error}");
                    return ExitCode::from(1);
                }
            }
        } else {
            eprintln!("sidecar process missing");
            return ExitCode::from(1);
        }
    }

    {
        let mut guard = processes.lock().expect("launcher mutex poisoned");
        match spawn_api() {
            Ok(child) => guard.api = Some(child),
            Err(error) => {
                eprintln!("failed to start fq-api: {error}");
                stop_all(&mut guard);
                return ExitCode::from(1);
            }
        }
    }

    let status_code = wait_for_first_exit(processes.clone());
    {
        let mut guard = processes.lock().expect("launcher mutex poisoned");
        stop_all(&mut guard);
    }
    ExitCode::from(status_code)
}

fn set_defaults() {
    let internal_token = env::var("INTERNAL_API_TOKEN")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "change-me-in-production".to_string());

    if env::var_os("INTERNAL_API_TOKEN").is_none() {
        env::set_var("INTERNAL_API_TOKEN", &internal_token);
    }
    if env::var_os("FQRS_SIDECAR_BASE_URL").is_none() {
        env::set_var("FQRS_SIDECAR_BASE_URL", "http://127.0.0.1:18080");
    }
    if env::var_os("FQRS_SIDECAR_INTERNAL_TOKEN").is_none() {
        env::set_var("FQRS_SIDECAR_INTERNAL_TOKEN", &internal_token);
    }
}

fn install_signal_handler(processes: Arc<Mutex<ChildProcesses>>) -> io::Result<()> {
    let mut signals = Signals::new([SIGINT, SIGTERM])?;
    thread::spawn(move || {
        for _ in signals.forever() {
            let mut guard = processes.lock().expect("launcher mutex poisoned");
            stop_all(&mut guard);
        }
    });
    Ok(())
}

fn spawn_sidecar() -> io::Result<Child> {
    Command::new("java")
        .arg("--enable-native-access=ALL-UNNAMED")
        .arg("-jar")
        .arg("/app/fq-sidecar.jar")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

fn spawn_api() -> io::Result<Child> {
    Command::new("/usr/local/bin/fq-api")
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
}

fn wait_for_first_exit(processes: Arc<Mutex<ChildProcesses>>) -> u8 {
    loop {
        {
            let mut guard = processes.lock().expect("launcher mutex poisoned");
            if let Some(sidecar) = guard.sidecar.as_mut() {
                match sidecar.try_wait() {
                    Ok(Some(status)) => return status.code().unwrap_or(1) as u8,
                    Ok(None) => {}
                    Err(_) => return 1,
                }
            }
            if let Some(api) = guard.api.as_mut() {
                match api.try_wait() {
                    Ok(Some(status)) => return status.code().unwrap_or(1) as u8,
                    Ok(None) => {}
                    Err(_) => return 1,
                }
            }
        }
        thread::sleep(Duration::from_millis(200));
    }
}

fn stop_all(children: &mut ChildProcesses) {
    stop_child(children.api.as_mut());
    stop_child(children.sidecar.as_mut());
}

fn stop_child(child: Option<&mut Child>) {
    if let Some(process) = child {
        let _ = process.kill();
        let _ = process.wait();
    }
}
