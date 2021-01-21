use crate::{Invocation, Result};
use failure::{bail, ResultExt};
use log::warn;
use std::{env, fs};
use std::process::Command;
use std::path::{Path, PathBuf};

pub fn copy_file<P: AsRef<Path>, Q: AsRef<Path>>(from: P, to: Q) -> Result<()> {
    use reflink::reflink;
    let (from, to) = (from.as_ref(), to.as_ref());
    let _ = std::fs::remove_file(to);
    fs::create_dir_all(to.parent().unwrap()).with_context(|e| {
        format!("Creating directory for dest file {:?} failed: {}", to.display(), e)
    })?;

    reflink(from, to)
        .with_context(|e| format!("Failed to copy from {:?} to {:?}: {}", from, to, e))?;
    Ok(())
}

pub(crate) fn integ(bin_name: &str, target_dir: &Path, mut invocation: Invocation) -> Result<()> {
    if is_release_configuration() {
        invocation.release = true;
    }

    let cargo = crate::cargo::Cargo::new(&invocation);

    match env::var("ACTION").with_context(|e| format!("Failed to read $ACTION: {}", e))?.as_str() {
        "build" | "install" => {
            let output_path =
                crate::lipo::build(&cargo, bin_name, target_dir, &targets_from_env()?)?;
            let executable_path = executable_path_from_env()?;
            copy_file(output_path, executable_path)?;
        }
        action => warn!("Unsupported XCode action: {:?}", action),
    }

    Ok(())
}

fn executable_path_from_env() -> Result<PathBuf> {
    let built_products_dir = env::var("BUILT_PRODUCTS_DIR")?;
    let executable_path = env::var("EXECUTABLE_PATH")?;
    Ok(PathBuf::from(built_products_dir).join(executable_path))
}

fn targets_from_env() -> Result<Vec<String>> {
    let archs = env::var("ARCHS").with_context(|e| format!("Failed to read $ARCHS: {}", e))?;
    let target_platform = match env::var("PLATFORM_NAME").as_ref().map(String::as_str) {
        Ok("macosx") => "apple-darwin",
        _ => "apple-ios",
    };
    Ok(archs
        .split(" ")
        .map(|a| a.trim())
        .filter(|a| !a.is_empty())
        .map(|a| map_arch_to_target(a, target_platform))
        .collect::<Result<Vec<_>>>()
        .with_context(|e| format!("Failed to parse $ARCHS: {}", e))?)
}

fn is_release_configuration() -> bool {
    env::var("CONFIGURATION").map(|v| v == "Release").unwrap_or(false)
}

fn map_arch_to_target(arch: &str, target_platform: &str) -> Result<String> {
    let mapped_arch = match arch {
        "armv7" => "armv7",
        "arm64" => "aarch64",
        "i386" => "i386",
        "x86_64" => "x86_64",
        _ => bail!("Unknown arch: {:?}", arch),
    };
    Ok(format!("{}-{}", mapped_arch, target_platform))
}

pub(crate) fn sanitize_env(cmd: &mut Command) {
    cmd.env_clear();
    cmd.envs(env::vars_os().filter(|&(ref name, _)| match name.to_str() {
        Some(name) => !(name.ends_with("DEPLOYMENT_TARGET") || name.starts_with("SDK")),
        None => false,
    }));
}
