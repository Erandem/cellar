mod cellar;
mod sandbox;

use cellar::WineCellar;
use clap::{App, AppSettings, Arg, ArgGroup};
use flexi_logger::Logger;
use log::{error, info};

use std::path::{Path, PathBuf};

fn app<'a>() -> App<'a> {
    App::new("winecellar")
        .version("1.0")
        .about("A toy for wine management without system dependence")
        .setting(AppSettings::SubcommandRequiredElseHelp)
        .arg(
            Arg::new("path")
                .about("The path to the wine cellar")
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
                    Arg::new("rel-c")
                        .about("Resolves paths relative to C: in the prefix")
                        .takes_value(false)
                        .long("rel-c"),
                )
                .group(ArgGroup::new("rel-to").arg("rel-c"))
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
}

fn main() -> cellar::Result<()> {
    Logger::try_with_str("debug").unwrap().start().unwrap();

    let matches = app().get_matches();

    let path = matches.value_of_t_or_exit::<PathBuf>("path");
    let mut cellar = match WineCellar::open(&path) {
        Ok(cellar) => cellar,
        Err(_) => {
            info!("Failed to find winecellar! Creating it...");
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

        Some(("exec", args)) => {
            let exec_path: PathBuf;

            if args.is_present("rel-c") {
                let c_drive_path = cellar.get_c_drive_path();
                info!("--rel-c flag specified! Resolving relative to C:\\\\");

                let exec: PathBuf = args.value_of_t_or_exit("executable");
                exec_path = c_drive_path.join(exec);
            } else {
                // absolute path implied
                info!("No path relativity specified! Assuming relative");
                let path = std::fs::canonicalize(args.value_of_t_or_exit::<PathBuf>("executable"));

                if path.is_err() {
                    info!("Failed to resolve path! Letting wine handle it");
                    exec_path = args.value_of_t_or_exit("executable");
                } else {
                    exec_path = path.unwrap();
                }
            }

            let workdir: PathBuf;

            info!("Launching executable with path {:?}", exec_path);

            if args.is_present("workdir") {
                workdir = args.value_of_t_or_exit("workdir");
            } else {
                workdir = exec_path
                    .parent()
                    .filter(|x| x != &Path::new(""))
                    .map(|x| x.to_path_buf())
                    .or(std::env::current_dir().ok())
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

            let mut cellar_result = cellar
                .run()
                .arg(exec_path)
                .args(exec_args)
                .current_dir(workdir)
                .spawn()
                .unwrap();

            if args.is_present("no-wait") {
                info!("\"no-wait\" flag specified! Exiting...");
            } else {
                if args.is_present("wait") {
                    info!("Wait arg specified! Waiting...");
                } else {
                    info!("No wait argument specified! Defaulting to \"--wait\"");
                }

                match cellar_result.wait() {
                    Ok(ok) => info!("Process exited! {:#?}", ok),
                    Err(e) => error!("Failed to exit! {:#?}", e),
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
