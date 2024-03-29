use crate::backend::backend_trait::{Backend, Switches, Text};
use crate::backend::macros::impl_backend_constants;
use crate::{Group, Package};
use anyhow::{Context, Result};
use core::panic;
use std::collections::HashSet;
use std::os::unix::process::ExitStatusExt;
use std::process::{Command, ExitStatus};

#[derive(Debug, Clone)]
pub struct Rustup {
    pub(crate) packages: HashSet<Package>,
}

enum Repotype {
    Toolchain,
    Component,
}

const BINARY: Text = "rustup";
const SECTION: Text = "rustup";

const SWITCHES_INSTALL: Switches = &["component", "add"];
const SWITCHES_INFO: Switches = &["component", "list", "--installed"];
const SWITCHES_MAKE_DEPENDENCY: Switches = &[];
const SWITCHES_NOCONFIRM: Switches = &[];
const SWITCHES_REMOVE: Switches = &["component", "remove"];

const SUPPORTS_AS_DEPENDENCY: bool = false;

impl Backend for Rustup {
    impl_backend_constants!();

    fn get_all_installed_packages(&self) -> Result<HashSet<Package>> {
        let mut toolchains_vec = self
            .run_toolchain_command(get_info_switches(Repotype::Toolchain))
            .context("Getting installed toolchains")?;

        let mut toolchains: HashSet<Package> = toolchains_vec
            .iter()
            .map(|name| ["toolchain", name].join("/").into())
            .collect();

        let packages: HashSet<Package> = self
            .run_component_command(get_info_switches(Repotype::Component), &mut toolchains_vec)
            .context("Getting installed components")?
            .iter()
            .map(|name| ["component", name].join("/").into())
            .collect();

        toolchains.extend(packages);
        Ok(toolchains)
    }

    fn get_explicitly_installed_packages(&self) -> Result<HashSet<Package>> {
        self.get_all_installed_packages()
            .context("Getting all installed packages")
    }

    fn make_dependency(&self, _: &[Package]) -> Result<ExitStatus> {
        anyhow::bail!("Not supported by {}", self.get_binary())
    }

    fn install_packages(&self, packages: &[Package], _: bool) -> Result<ExitStatus> {
        for p in packages {
            let repo = if p.repo.is_some() {
                p.repo.as_ref().expect("This should never be reached")
            } else {
                anyhow::bail!("Not specified whether it is a toolchain or a component")
            };

            let mut cmd = Command::new(self.get_binary());
            match repo.as_str() {
                "toolchain" => {
                    cmd.args(get_install_switches(Repotype::Toolchain));
                    cmd.arg(&p.name);
                }
                "component" => {
                    cmd.args(get_install_switches(Repotype::Component));
                    let mut iter = p.name.split('/');
                    let toolchain = iter.next().expect("Toolchain not specified!");
                    let component = iter.next().expect("Component not specified!");
                    cmd.arg(toolchain).arg(component);
                }
                _ => anyhow::bail!("No such type is managed by rustup!"),
            }

            let result = cmd.status().context("Installing toolchain {p}");
            if !result.as_ref().is_ok_and(|exit| exit.success()) {
                return result;
            }
        }
        Ok(ExitStatus::from_raw(0))
    }

    fn remove_packages(&self, packages: &[Package], _: bool) -> Result<ExitStatus> {
        let mut toolchains_rem = Vec::new();

        for p in packages {
            let repo = if p.repo.is_some() {
                p.repo.as_ref().expect("This will never be printed!")
            } else {
                anyhow::bail!("Not specified whether it is a toolchain or a component")
            };

            if repo == "toolchain" {
                let mut cmd = Command::new(self.get_binary());
                cmd.args(get_remove_switches(Repotype::Toolchain))
                    .arg(&p.name);
                toolchains_rem.push(p.name.as_str());

                let result = cmd.status().context("Removing toolchain {p}");
                if !result.as_ref().is_ok_and(|exit| exit.success()) {
                    return result;
                }
            }
        }

        for p in packages {
            let repo = if p.repo.is_some() {
                p.repo.as_ref().expect("This will never be printed!")
            } else {
                anyhow::bail!("Not specified whether it is a toolchain or a component")
            };

            let mut iter = p.name.split('/').peekable();
            let toolchain = if iter.peek().is_some() {
                iter.next().expect("This should never be printed")
            } else {
                anyhow::bail!("No toolchain name provided for the given component!")
            };

            if repo == "component" && !toolchains_rem.contains(&toolchain) {
                let mut cmd = Command::new(self.get_binary());

                cmd.args(get_remove_switches(Repotype::Component))
                    .arg(toolchain)
                    .arg(
                        iter.next().unwrap_or_else(|| {
                            panic!("Component name not provided for {}", p.name)
                        }),
                    );

                let result = cmd.status().context("Removing toolchain {p}");
                if !result.as_ref().is_ok_and(|exit| exit.success()) {
                    return result;
                }
            }
        }
        Ok(ExitStatus::from_raw(0))
    }
}

impl Rustup {
    pub(crate) fn new() -> Self {
        Self {
            packages: HashSet::new(),
        }
    }

    fn run_component_command(
        &self,
        args: &[&str],
        toolchains: &mut Vec<String>,
    ) -> Result<Vec<String>> {
        let mut val = Vec::new();
        for toolchain in toolchains {
            let mut cmd = Command::new(self.get_binary());
            cmd.args(args).arg(&toolchain);
            let output = String::from_utf8(cmd.output()?.stdout)?;
            for component in output.lines() {
                install_components(component, toolchain, &mut val);
            }
        }
        Ok(val)
    }
    fn run_toolchain_command(&self, args: &[&str]) -> Result<Vec<String>> {
        let mut cmd = Command::new(self.get_binary());
        cmd.args(args);
        let output = String::from_utf8(cmd.output()?.stdout)?;
        let mut val = Vec::new();
        for i in output.lines() {
            let mut it = i.splitn(2, '-');
            val.push(it.next().expect("Toolchain name is empty.").to_string());
        }
        Ok(val)
    }
}

fn get_install_switches(repotype: Repotype) -> Switches {
    match repotype {
        Repotype::Toolchain => &["toolchain", "install"],
        Repotype::Component => &["component", "add", "--toolchain"],
    }
}

fn get_remove_switches(repotype: Repotype) -> Switches {
    match repotype {
        Repotype::Toolchain => &["toolchain", "uninstall"],
        Repotype::Component => &["component", "remove", "--toolchain"],
    }
}

fn get_info_switches(repotype: Repotype) -> Switches {
    match repotype {
        Repotype::Toolchain => &["toolchain", "list"],
        Repotype::Component => &["component", "list", "--installed", "--toolchain"],
    }
}

fn install_components(line: &str, toolchain: &str, val: &mut Vec<String>) {
    let mut chunks = line.splitn(3, '-');
    let component = chunks.next().expect("Component name is empty!");
    match component {
        // these are the only components that have a single word name
        "cargo" | "rustfmt" | "clippy" | "miri" | "rls" | "rustc" => {
            val.push([toolchain, component].join("/"));
        }
        // all the others have two words hyphenated as component names
        _ => {
            let component = [
                component,
                chunks
                    .next()
                    .expect("No such component is managed by rustup"),
            ]
            .join("-");
            val.push([toolchain, component.as_str()].join("/"));
        }
    }
}
