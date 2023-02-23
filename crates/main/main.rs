/*!
Main program for `pacdef`. All internal logic happens in [`pacdef_core`].
*/

#![warn(
    clippy::as_conversions,
    clippy::option_if_let_else,
    clippy::redundant_pub_crate,
    clippy::semicolon_if_nothing_returned,
    clippy::unused_self,
    clippy::unwrap_used,
    clippy::use_debug,
    clippy::use_self,
    clippy::wildcard_dependencies,
    missing_docs
)]

use std::process::{ExitCode, Termination};

use anyhow::{Context, Result};

use pacdef_core::path::{get_config_path, get_config_path_old_version, get_group_dir};
use pacdef_core::{get_args, Config, Group, Pacdef};

fn main() -> ExitCode {
    handle_final_result(main_inner())
}

/// Skip printing the error chain when searching packages yields no results,
/// otherwise report error chain.
fn handle_final_result(result: Result<()>) -> ExitCode {
    match result {
        Ok(_) => ExitCode::SUCCESS,
        Err(ref e) => {
            let root_e = e.root_cause().downcast_ref();
            match root_e {
                Some(pacdef_core::Error::NoPackagesFound) => ExitCode::FAILURE,
                Some(_) => result.context("unknown pacdef error type").report(),
                None => result.report(),
            }
        }
    }
}

fn main_inner() -> Result<()> {
    let args = get_args();

    let config_file = get_config_path().context("getting config file")?;

    let config = Config::load(&config_file)
        .or_else(|_| {
            get_config_path_old_version()?
                .exists()
                .then(show_transition_link);
            let default = Config::default();
            default.save(&config_file)?;
            Ok::<Config, anyhow::Error>(default)
        })
        .context("loading config")?;

    let group_dir = get_group_dir().context("resolving group dir")?;
    let groups = Group::load(&group_dir, config.warn_not_symlinks)
        .with_context(|| format!("loading groups under {}", group_dir.to_string_lossy()))?;

    let pacdef = Pacdef::new(args, config, groups);
    pacdef.run_action_from_arg().context("running action")
}

fn show_transition_link() {
    println!("VERSION UPGRADE");
    println!("You seem to have used version 0.x of pacdef before.");
    println!("Version 1.x changes the syntax of the config files and the command line arguments.");
    println!("Check out https://github.com/steven-omaha/pacdef for new syntax information.");
    println!("This message will not appear again.");
    println!("------");
}
