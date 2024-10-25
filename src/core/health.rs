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
        Box::pin(check_fusermount()),
    ];

    let results = join_all(checks).await;

    results.into_iter().for_each(|result| {
        if let Some(error) = result {
            errors.push(error);
        }
    });

    for error in &errors {
        warn!("{}", error);
        println!(
            "{} {}",
            "More info at:".color(Color::Cyan),
            "https://l.ajam.dev/namespace".color(Color::Blue)
        )
    }

    if errors.is_empty() {
        success!("Everything is in order.")
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

async fn check_fusermount() -> Option<&'static str> {
    match which::which("fusermount") {
        Ok(path) => match path.metadata() {
            Ok(meta) => {
                let permissions = meta.permissions().mode();
                if permissions != 0o104755 {
                    return Some(Box::leak(
                        format!(
                            "Invalid {} file mode bits. Set 4755 for {}",
                            "fusermount".color(Color::Blue),
                            path.to_string_lossy().color(Color::Green)
                        )
                        .into_boxed_str(),
                    ) as &'static str);
                }
            }
            Err(_) => return Some("Unable to read fusermount"),
        },
        Err(_) => {
            return Some(Box::leak(
                format!(
                    "{} not found. Please install {}",
                    "fusermount".color(Color::Blue),
                    "fuse".color(Color::Blue)
                )
                .into_boxed_str(),
            ))
        }
    }
    None
}
