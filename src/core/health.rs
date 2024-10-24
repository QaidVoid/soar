use std::{cmp::Ordering, path::Path};

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

    if let Ok(content) = fs::read_to_string("/proc/sys/kernel/unprivileged_userns_clone").await {
        if content.trim() == "0" {
            errors.push("You must enable unprivileged_userns_clone");
        }
    }
    if let Ok(content) = fs::read_to_string("/proc/sys/user/max_user_namespaces").await {
        if content.trim() == "0" {
            errors.push("You must enable max_user_namespaces");
        }
    }
    if let Ok(content) = fs::read_to_string("/proc/sys/kernel/userns_restrict").await {
        if content.trim() == "1" {
            errors.push("You must disable userns_restrict");
        }
    }
    if let Ok(content) =
        fs::read_to_string("/proc/sys/kernel/apparmor_restrict_unprivileged_userns").await
    {
        if content.trim() == "1" {
            errors.push("You must disable apparmor_restrict_unprivileged_userns");
        }
    }

    if !check_capability(CAP_SYS_ADMIN) {
        errors.push("Capability 'CAP_SYS_ADMIN' is not available.");
        if !check_capability(CAP_MKNOD) {
            errors.push("Capability 'CAP_MKNOD' is not available.");
        }
    }

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
