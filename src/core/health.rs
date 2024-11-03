use std::{cmp::Ordering, future::Future, os::unix::fs::PermissionsExt, path::Path, pin::Pin};

use futures::future::join_all;
use libc::{fork, unshare, waitpid, CLONE_NEWUSER, PR_CAPBSET_READ};
use tokio::fs;

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::CAP_MKNOD,
    },
    success, warn,
};

use super::constant::CAP_SYS_ADMIN;

fn check_capability(cap: i32) -> bool {
    unsafe { libc::prctl(PR_CAPBSET_READ, cap, 0, 0) == 1 }
}

pub async fn check_health() {
    let mut errors = Vec::new();

    let pid = unsafe { fork() };
    match pid.cmp(&0) {
        Ordering::Equal => {
            if unsafe { unshare(CLONE_NEWUSER) != 0 } {
                errors.push("You lack permissions to create user_namespaces");
            }
            std::process::exit(0);
        }
        Ordering::Greater => {
            unsafe {
                waitpid(pid, std::ptr::null_mut(), 0);
            };
        }
        _ => {}
    }

    if !Path::new("/proc/self/ns/user").exists() {
        errors.push("Your kernel does not support user namespaces");
    }

    let checks: Vec<Pin<Box<dyn Future<Output = Option<&'static str>>>>> = vec![
        Box::pin(check_unprivileged_userns_clone()),
        Box::pin(check_max_user_namespaces()),
        Box::pin(check_userns_restrict()),
        Box::pin(check_apparmor_restrict()),
        Box::pin(check_capabilities()),
    ];

    println!("{0}  FUSE CHECK {0}", "☵".repeat(4));
    check_fusermount().await;

    let results = join_all(checks).await;

    results.into_iter().for_each(|result| {
        if let Some(error) = result {
            errors.push(error);
        }
    });

    println!();

    println!("{0}  USER NAMESPACE CHECK {0}", "☵".repeat(4));
    for error in &errors {
        warn!("{}", error);
    }

    if errors.is_empty() {
        success!("User namespace checked successfully.")
    } else {
        println!(
            "{} {}",
            "More info at:".color(Color::Cyan),
            "https://l.ajam.dev/namespace".color(Color::Blue)
        )
    }
}

async fn check_unprivileged_userns_clone() -> Option<&'static str> {
    let content = fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone")
        .await
        .ok()?;
    if content.trim() == "0" {
        Some("You must enable unprivileged_userns_clone")
    } else {
        None
    }
}

async fn check_max_user_namespaces() -> Option<&'static str> {
    let content = fs::read_to_string("/proc/sys/user/max_user_namespaces")
        .await
        .ok()?;
    if content.trim() == "0" {
        Some("You must enable max_user_namespaces")
    } else {
        None
    }
}

async fn check_userns_restrict() -> Option<&'static str> {
    let content = fs::read_to_string("/proc/sys/kernel/userns_restrict")
        .await
        .ok()?;
    if content.trim() == "1" {
        Some("You must disable userns_restrict")
    } else {
        None
    }
}

async fn check_apparmor_restrict() -> Option<&'static str> {
    let content = fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns")
        .await
        .ok()?;
    if content.trim() == "1" {
        Some("You must disable apparmor_restrict_unprivileged_userns")
    } else {
        None
    }
}

async fn check_capabilities() -> Option<&'static str> {
    if !check_capability(CAP_SYS_ADMIN) {
        if !check_capability(CAP_MKNOD) {
            return Some("Capability 'CAP_MKNOD' is not available.");
        }
        return Some("Capability 'CAP_SYS_ADMIN' is not available.");
    }
    None
}

async fn check_fusermount() {
    let mut error = String::new();

    let fusermount_path = match which::which("fusermount3") {
        Ok(path) => Some(path),
        Err(_) => match which::which("fusermount") {
            Ok(path) => Some(path),
            Err(_) => {
                error = format!(
                    "{} not found. Please install {}.\n",
                    "fusermount".color(Color::Blue),
                    "fuse".color(Color::Blue)
                );
                None
            }
        },
    };

    if let Some(fusermount_path) = fusermount_path {
        match fusermount_path.metadata() {
            Ok(meta) => {
                let permissions = meta.permissions().mode();
                if permissions != 0o104755 {
                    error = format!(
                        "Invalid file mode bits. Set 4755 for {}.",
                        fusermount_path.to_string_lossy().color(Color::Green)
                    );
                }
            }
            Err(_) => {
                error = "Unable to read fusermount metadata.".to_owned();
            }
        }
    }

    if !error.is_empty() {
        warn!(
            "{}\n{} {}",
            error,
            "More info at:".color(Color::Cyan),
            "https://l.ajam.dev/fuse".color(Color::Blue)
        );
    } else {
        success!("Fuse checked successfully.");
    }
}
