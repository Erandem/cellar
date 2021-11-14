mod cellar;
mod sandbox;

use cellar::WineCellar;
use cellar::WineSync;
use clap::{App, AppSettings, Arg, ArgGroup};
use flexi_logger::Logger;
use log::{error, info, warn};

use std::path::{Path, PathBuf};

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
                .arg(
                    Arg::new("workdir")
                        .about("Working directory of the executable")
                        .takes_value(true),
                )
                .group(ArgGroup::new("wait-type").arg("wait").arg("no-wait"))
                .arg(Arg::new("no-wait").about("Does not wait for cellar before exiting"))
                .arg(Arg::new("wait").about("Wait for cellar to exit").short('w'))
                // for all arguments to be passed to the executable
                .setting(AppSettings::TrailingVarArg)
                .arg(
                    Arg::new("exec-arguments")
                        .raw(true)
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

            cellar.set_env_var(key.clone(), value.clone());
            cellar.save_config()?;
            info!("Set {} to {}", key, value);
        }

        Some(("list-env", _)) => cellar
            .get_env_vars()
            .iter()
            .for_each(|(k, v)| info!("{}={}", k, v)),

        Some(("shell", _)) => {
            info!("Starting shell with bubblewrap sandbox");

            cellar.bwrap_run().arg("/usr/bin/bash").status().unwrap();
        }

        Some(("exec", args)) => {
            let exec_path = args.value_of_t_or_exit::<PathBuf>("executable");

            let workdir: PathBuf;

            info!("Launching executable with path {:?}", exec_path);

            if args.is_present("workdir") {
                workdir = args.value_of_t_or_exit("workdir");
            } else {
                workdir = exec_path
                    .parent()
                    .filter(|x| x != &Path::new(""))
                    .map(|x| x.to_path_buf())
                    .or(Some(PathBuf::from("/tmp")))
                    .unwrap();
            }

            // TODO Add wine version information
            info!("Using wine version {}", "todo");
            info!("Using work directory {:?}", workdir);

            // direct path expected
            let exec_args = args
                .values_of("exec-arguments")
                .into_iter()
                .flatten()
                .map(|x| x.to_string())
                .collect::<Vec<String>>();

            info!("passing arguments {:?} to the provided binary", exec_args);

            let mut cellar_result = cellar.bwrap_wine();
            cellar_result.arg(exec_path).args(exec_args);
            //.current_dir(workdir)

            println!("{:#?}", cellar_result);
            let mut cellar_result = cellar_result.status();
            println!("{:#?}", cellar_result);

            if args.is_present("no-wait") {
                info!("\"no-wait\" flag specified! Exiting...");
            } else {
                if args.is_present("wait") {
                    info!("Wait arg specified! Waiting...");
                } else {
                    info!("No wait argument specified! Defaulting to \"--wait\"");
                }
            }
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
