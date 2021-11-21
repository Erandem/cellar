mod cellar;
mod reaper;

use cellar::WineCellar;
use cellar::WineSync;
use clap::{App, AppSettings, Arg, ArgGroup};
use flexi_logger::Logger;
use log::{error, info, warn};
use reaper::ReaperCommand;

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;

fn app<'a>() -> App<'a> {
    App::new("cellar")
        .version("1.0")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::new("path")
                .about("The path to the cellar")
                .required(true)
                .takes_value(true),
        )
        .arg(
            Arg::new("create")
                .about("Creates the cellar if it does not exist")
                .short('c'),
        )
        .subcommand(
            App::new("set-env")
                .about("Sets an environmental variable")
                .arg(Arg::new("key").required(true).takes_value(true))
                .arg(Arg::new("value").required(true).takes_value(true)),
        )
        .subcommand(App::new("shell").about("Starts a new shell in the sandbox"))
        .subcommand(
            App::new("exec")
                .about("Allows you to run programs")
                .arg(
                    Arg::new("executable")
                        .required(true)
                        .takes_value(true)
                        .about("Path to the executable"),
                )
                // for all arguments to be passed to the executable
                .setting(AppSettings::TrailingVarArg)
                .arg(
                    Arg::new("exec-arguments")
                        .raw(true)
                        .last(true)
                        .about("All arguments to be passed to the executable"),
                ),
        )
        .subcommand(App::new("kill"))
        .subcommand(App::new("list-env").about("Lists environmental variables"))
        .subcommand(App::new("cfg-list").about("Lists settings in the sandbox"))
        .subcommand(
            App::new("cfg-set")
                .about("Set settings")
                .arg(Arg::new("key").required(true).possible_value("sync"))
                .arg(Arg::new("value").required(true)),
        )
}

fn main() -> cellar::Result<()> {
    Logger::try_with_str("debug").unwrap().start().unwrap();

    let matches = app().get_matches();

    let path = matches.value_of_t_or_exit::<PathBuf>("path");
    let mut cellar = match WineCellar::open(&path) {
        Ok(cellar) => cellar,
        Err(_) => {
            warn!("Failed to find cellar! Creating it...");
            WineCellar::create(&path)?
        }
    };

    match matches.subcommand() {
        Some(("cfg-list", _)) => {
            let serialized = serde_json::to_value(cellar.config).unwrap();
            info!("Settings");

            serialized
                .as_object()
                .unwrap()
                .into_iter()
                .for_each(|x| info!("- {} = {}", x.0, x.1));
        }

        Some(("cfg-set", args)) => match args.value_of_t_or_exit::<String>("key").as_ref() {
            "sync" => {
                let sync_type: WineSync = args.value_of_t_or_exit("value");
                info!("Setting \"sync\" to \"{:#?}\"", sync_type);

                cellar.config.sync = sync_type;
                cellar.save_config().unwrap();
            }
            unknown => error!("Unknown key \"{}\"", unknown),
        },

        Some(("set-env", args)) => {
            let key: String = args.value_of_t_or_exit("key");
            let value: String = args.value_of_t_or_exit("value");

            info!("Set {} to {}", key, value);
            cellar.set_env_var((key, value));
            cellar.save_config()?;
        }

        Some(("list-env", _)) => cellar.get_env_vars().iter().for_each(|e| info!("{:?}", e)),

        Some(("shell", _)) => {
            info!("Starting shell with bubblewrap sandbox");

            cellar.bwrap_run().arg("/usr/bin/bash").status().unwrap();
        }

        Some(("exec", args)) => {
            let exec_path = args.value_of_t_or_exit::<PathBuf>("executable");

            // TODO Add wine version information
            info!("Using wine version {}", "todo");

            // direct path expected
            let mut exec_args = args
                .values_of("exec-arguments")
                .into_iter()
                .flatten()
                .map(|x| x.to_string())
                .collect::<VecDeque<String>>();

            info!(
                "Calling {} {}",
                exec_path.display(),
                exec_args.make_contiguous().join(" ")
            );

            // We gotta put the executable path at the very start so wine knows what executable to
            // start
            exec_args.push_front(exec_path.as_os_str().to_str().unwrap().to_string());

            let mut child = cellar
                .bwrap_run()
                .arg("/tmp/reaper")
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();

            info!("Starting reaper in jail");
            let start_cmd = ReaperCommand::Execute {
                exec: "/usr/bin/wine".into(),
                args: exec_args.into_iter().collect(),
                env: cellar.get_env_vars().clone(),
            };

            let child_stdin = child.stdin.as_mut().unwrap();
            start_cmd.dispatch(&*child_stdin);

            drop(child_stdin);

            child.wait().unwrap();
        }

        Some(("kill", _)) => {
            println!("Killing prefix at {:?}", cellar.path());
            cellar.kill();
        }

        Some((name, _)) => error!("Unknown or unimplemented command {}", name),
        None => error!("required argument not passed"),
    }

    Ok(())
}
