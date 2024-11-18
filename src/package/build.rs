use anyhow::Result;
use chrono::Utc;
use nanoid::nanoid;
use std::{
    fs::{self, File, Permissions},
    io::{BufRead, BufReader, BufWriter, Write},
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
};
use tracing::{debug, error, info};

use crate::{
    core::{
        color::{Color, ColorExt},
        constant::CACHE_PATH,
    },
    misc::download::download,
};

#[derive(Debug)]
pub struct BuildOutput {
    sbuild_successful: bool,
    sbuild_pkg: String,
    pkg_ver: String,
    pkg_type: String,
    sbuild_outdir: PathBuf,
    sbuild_tmpdir: PathBuf,
}

impl BuildOutput {
    pub async fn from(log_path: &Path, vars: &[(String, String)]) -> Result<Self> {
        let mut output = BuildOutput {
            sbuild_successful: false,
            sbuild_pkg: String::new(),
            pkg_ver: String::new(),
            pkg_type: String::new(),
            sbuild_outdir: PathBuf::new(),
            sbuild_tmpdir: PathBuf::new(),
        };

        for (key, value) in vars {
            match key.as_str() {
                "SBUILD_SUCCESSFUL" => {
                    output.sbuild_successful = value.to_lowercase() == "yes"
                        || value.to_lowercase() == "true"
                        || value == "1"
                }
                "SBUILD_PKG" => output.sbuild_pkg = value.to_string(),
                "PKG_VER" => output.pkg_ver = value.to_string(),
                "PKG_TYPE" => output.pkg_type = value.to_string(),
                "SBUILD_OUTDIR" => output.sbuild_outdir = PathBuf::from(value),
                "SBUILD_TMPDIR" => output.sbuild_tmpdir = PathBuf::from(value),
                _ => {}
            }
        }

        if output.sbuild_pkg.is_empty() {
            anyhow::bail!("SBUILD_PKG not found in environment file");
        }
        if output.pkg_ver.is_empty() {
            anyhow::bail!("PKG_VER not found in environment file.");
        }
        if output.pkg_type.is_empty() {
            anyhow::bail!("PKG_TYPE not found in environment file.");
        }
        if !output.sbuild_outdir.is_dir() {
            anyhow::bail!("SBUILD_OUTDIR is invalid.");
        }
        if !output.sbuild_tmpdir.is_dir() {
            anyhow::bail!("SBUILD_TMPDIR is invalid.");
        }

        std::fs::remove_dir_all(&output.sbuild_tmpdir)?;
        let dir = std::fs::read_dir(&output.sbuild_outdir)?;
        let final_dir = CACHE_PATH.join(&output.sbuild_pkg);
        std::fs::create_dir_all(&final_dir)?;

        for entry in dir {
            let file = entry?;
            let file_name = file.file_name();
            let final_path = final_dir.join(&file_name);
            std::fs::copy(file.path(), &final_path)?;
        }

        std::fs::copy(log_path, final_dir.join(format!("build.log")))?;
        std::fs::remove_file(log_path)?;

        std::fs::remove_dir_all(&output.sbuild_outdir)?;

        Ok(output)
    }
}

enum OutputLine {
    Stdout(String),
    Stderr(String),
}

pub async fn init<P: AsRef<Path>>(file_path: P) -> Result<()> {
    let file_path = file_path.as_ref();
    let output_env_path = PathBuf::from(&format!("{}.env", file_path.display()));

    let sbuild_runner = if let Ok(sbuild_runner) = which::which("sbuild-runner") {
        sbuild_runner
    } else {
        let runner_path = CACHE_PATH.join("sbuild-runner").to_path_buf();
        if !runner_path.exists() {
            let runner_url = "https://raw.githubusercontent.com/pkgforge/soarpkgs/9fe521e47f4e265345e19526da2879654f268491/scripts/sbuild_runner.sh";
            download(runner_url, Some(runner_path.to_string_lossy().to_string())).await?;
            fs::set_permissions(&runner_path, Permissions::from_mode(0o755))?;
        }
        runner_path
    };

    let sbuild_id = nanoid!();
    let mut child = Command::new(sbuild_runner)
        .arg(file_path)
        .env("SBUILD_ID", &sbuild_id)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let (tx, rx) = mpsc::channel();
    let tx_stderr = tx.clone();

    let log_path = CACHE_PATH.join(&format!("{}.build.log", sbuild_id));
    let log_file = File::create(&log_path)?;
    let mut writer = BufWriter::new(log_file);

    let stdout_handle = std::thread::spawn({
        move || {
            let reader = BufReader::new(stdout);
            reader.lines().for_each(|line| {
                if let Ok(line) = line {
                    tx.send(OutputLine::Stdout(line)).unwrap();
                }
            });
        }
    });

    let stderr_handle = std::thread::spawn({
        move || {
            let reader = BufReader::new(stderr);
            reader.lines().for_each(|line| {
                if let Ok(line) = line {
                    tx_stderr.send(OutputLine::Stderr(line)).unwrap();
                }
            });
        }
    });

    let output_handle = std::thread::spawn(move || {
        let ts_format = "%FT%T%.3f";
        while let Ok(output) = rx.recv() {
            let now = Utc::now().format(ts_format);
            match output {
                OutputLine::Stdout(line) => {
                    debug!("[{}] {}", now, line);
                    writeln!(writer, "[{}] {}", now, line).unwrap();
                }
                OutputLine::Stderr(line) => {
                    debug!("[{}] ERR: {}", now, line);
                    writeln!(writer, "[{}] ERR: {}", now, line).unwrap();
                }
            }
        }
    });

    stdout_handle.join().unwrap();
    stderr_handle.join().unwrap();

    output_handle.join().unwrap();

    let status = child.wait()?;
    if !status.success() {
        anyhow::bail!("Build failed with status: {}", status);
    }

    if output_env_path.exists() {
        let f = File::open(output_env_path)?;
        let reader = BufReader::new(f);

        let mut output = Vec::new();
        for line in reader.lines() {
            let line = line?.trim().to_owned();

            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                output.push((
                    key.trim_matches([' ', '"', '\'']).to_owned(),
                    value.trim_matches([' ', '"', '\'']).to_owned(),
                ));
            }
        }

        let final_output = BuildOutput::from(&log_path, &output).await?;
        if final_output.sbuild_successful {
            info!("{}", "Build successful.".color(Color::BrightGreen));
        } else {
            error!("{}", "Build failed.".color(Color::BrightRed));
        }
    }

    Ok(())
}
