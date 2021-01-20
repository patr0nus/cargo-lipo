use crate::{Invocation, Result};
use failure::bail;
use log::{info, debug};
use std::path::Path;

pub(crate) struct Meta<'a> {
    packages: Vec<Package<'a>>,
    target_dir: &'a Path,
}

pub(crate) struct Package<'a> {
    name: &'a str,
    lib_name: String,
}

impl<'a> Meta<'a> {
    #[allow(clippy::useless_let_if_seq)] // multiple variables are initialized
    pub(crate) fn new(
        invocation: &Invocation,
        meta: &'a cargo_metadata::Metadata,
    ) -> Result<Meta<'a>> {
        let package_names: Vec<_>;
        let staticlib_or_bin_required;
        let mut allowed_crate_types = vec!["staticlib"];
        if invocation.allow_bin {
            allowed_crate_types.push("bin");
        }
        let allowed_crate_types_string = allowed_crate_types
            .iter()
            .map(|t| format!("`{}`", t))
            .collect::<Vec<String>>()
            .join(" or ");

        if !invocation.packages.is_empty() {
            package_names = invocation.packages.iter().map(|p| p.as_str()).collect();
            staticlib_or_bin_required = true;
        } else {
            package_names = meta.workspace_members.iter().map(|m| m.name()).collect();
            // Require a staticlib or bin for single-member workspaces unless `--all` was specified.
            staticlib_or_bin_required = meta.workspace_members.len() == 1 && !invocation.all;
        }

        debug!(
            "Considering package(s) {:?}, {:?} {}",
            package_names,
            allowed_crate_types_string,
            if staticlib_or_bin_required { "required" } else { "not required" }
        );

        let mut packages = vec![];

        for &name in &package_names {
            let package = match meta.packages.iter().find(|p| p.name == name) {
                Some(p) => p,
                None => bail!("No package metadata found for {:?}", name),
            };

            let targets: Vec<_> = package
                .targets
                .iter()
                .filter(|t| t.kind.iter().any(|k| allowed_crate_types.contains(&k.as_str())))
                .collect();

            match targets.as_slice() {
                [] => {
                    if !staticlib_or_bin_required {
                        debug!(
                            "Ignoring {:?} because it does not have a {} target",
                            allowed_crate_types_string, name
                        );
                        continue;
                    }
                    bail!("No library target found for {:?}", name);
                }
                [target] => {
                    if target
                        .crate_types
                        .iter()
                        .any(|ct| allowed_crate_types.contains(&ct.as_str()))
                    {
                        packages.push((package, target.name.replace('-', "_")));
                    } else {
                        if !staticlib_or_bin_required {
                            debug!(
                                "Ignoring {:?} because it does not have a {} crate type",
                                allowed_crate_types_string, name
                            );
                            continue;
                        }
                        bail!("No {} crate type found for {:?}", allowed_crate_types_string, name);
                    }
                }
                _ => bail!("Found multiple lib targets for {:?}", name),
            }
        }

        let packages = packages
            .into_iter()
            .map(|(p, lib_name)| Package { name: p.name.as_str(), lib_name })
            .collect::<Vec<_>>();

        let package_names = packages.iter().map(|p| p.name).collect::<Vec<_>>();

        if packages.is_empty() {
            bail!(
                "Did not find any packages with a {} target, considered {:?}",
                allowed_crate_types_string,
                package_names
            );
        }

        info!("Will build universal library for {:?}", package_names);

        Ok(Meta { packages, target_dir: Path::new(&meta.target_directory) })
    }

    pub(crate) fn packages(&self) -> &[Package<'a>] {
        &self.packages
    }

    pub(crate) fn target_dir(&self) -> &'a Path {
        self.target_dir
    }
}

impl<'a> Package<'a> {
    pub(crate) fn name(&self) -> &'a str {
        self.name
    }

    pub(crate) fn lib_name(&self) -> &str {
        self.lib_name.as_str()
    }
}
