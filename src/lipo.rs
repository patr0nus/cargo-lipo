use crate::Result;
use crate::cargo::Cargo;
use failure::ResultExt;
use log::{info, warn};
use std::fs;
use std::process::Command;
use std::path::{Path, PathBuf};

fn should_update_output(
    output: impl AsRef<Path>,
    inputs: impl IntoIterator<Item = impl AsRef<Path>>,
) -> std::io::Result<bool> {
    let output_metadata = match fs::metadata(output) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(true),
    };
    let output_mtime = output_metadata.modified()?;
    for input in inputs {
        let input_mtime = fs::metadata(input)?.modified()?;
        if input_mtime > output_mtime {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn build(
    cargo: &Cargo,
    bin_name: &str,
    target_dir: &Path,
    targets: &[impl AsRef<str>],
) -> Result<PathBuf> {
    let mut lipo_inputs = Vec::<PathBuf>::with_capacity(targets.len());

    for target in targets {
        let target = target.as_ref();
        info!("Building {:?} for {:?}", bin_name, target);

        cargo
            .build_bin(bin_name, target)
            .with_context(|e| format!("Failed to build {:?} for {:?}: {}", bin_name, target, e))?;

        let input = target_dir.join(target).join(cargo.profile()).join(bin_name);

        lipo_inputs.push(input);
    }

    if lipo_inputs.is_empty() {
        failure::bail!("No target to build for {:?}", bin_name)
    }

    if lipo_inputs.len() == 1 {
        return Ok(lipo_inputs.pop().unwrap());
    }

    let mut targets = targets.iter().map(|t| t.as_ref().to_string()).collect::<Vec<String>>();
    targets.sort();
    let joined_target_name = targets.join("|");

    let mut lipo_output = target_dir.to_owned();
    lipo_output.push(joined_target_name);
    lipo_output.push(cargo.profile());

    fs::create_dir_all(&lipo_output).with_context(|e| {
        format!("Creating output directory \"{}\" failed: {}", lipo_output.display(), e)
    })?;

    lipo_output.push(&bin_name);

    match should_update_output(&lipo_output, &lipo_inputs) {
        Ok(true) => {}
        Ok(false) => {
            info!("Universal executable is up-to-date, skipping lipo invocation for {}", bin_name);
            return Ok(lipo_output);
        }
        Err(e) => {
            warn!("Failed to check if universal executable for {:?} is up-to-date: {}", bin_name, e)
        }
    }
    let mut cmd = Command::new("lipo");
    cmd.arg("-create").arg("-output").arg(lipo_output.clone());
    cmd.args(lipo_inputs);

    info!("Creating universal executable for {}", bin_name);

    crate::exec::run(cmd)?;

    Ok(lipo_output)
}
