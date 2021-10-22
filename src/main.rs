mod cellar;

use cellar::WineCellar;
use clap::{App, AppSettings, Arg, ArgGroup};

use std::path::PathBuf;

fn main() -> cellar::Result<()> {
    let matches = App::new("winecellar")
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
        .get_matches();

    let path = matches.value_of_t_or_exit::<PathBuf>("path");
    let mut cellar = match WineCellar::open(&path) {
        Ok(cellar) => cellar,
        Err(_) => WineCellar::create(&path)?,
    };

    match matches.subcommand() {
        Some(("set-env", args)) => {
            let key: String = args.value_of_t_or_exit("key");
            let value: String = args.value_of_t_or_exit("value");

            cellar.set_env_var(key.clone(), value.clone());
            cellar.save_config()?;
            println!("Set \"{}\" to \"{}\"", key, value);
        }

        Some(("list-env", _)) => cellar
            .get_env_vars()
            .iter()
            .for_each(|(k, v)| println!("{}={}", k, v)),

        Some(("exec", args)) => {
            let exec_path: PathBuf;

            if args.is_present("rel-c") {
                let c_drive_path = cellar.get_c_drive_path();
                println!("Resolving executable relative to {:?}", c_drive_path);

                let exec: PathBuf = args.value_of_t_or_exit("executable");
                exec_path = c_drive_path.join(exec);
            } else {
                // absolute path implied
                println!("No path relativity specified! Using default");
                exec_path = std::fs::canonicalize(args.value_of_t_or_exit::<PathBuf>("executable"))
                    .unwrap();
            }

            let workdir: PathBuf;

            if args.is_present("workdir") {
                workdir = args.value_of_t_or_exit("workdir");
            } else {
                workdir = exec_path.parent().unwrap().to_path_buf();
            }

            println!("Running {:?} with wine version {}", exec_path, "TODO");
            println!("Using workdir {:?}", workdir);

            // direct path expected
            let exec_args = args
                .values_of("exec-arguments")
                .into_iter()
                .flatten()
                .map(|x| x.to_string())
                .collect::<Vec<String>>();

            println!("{:?}", exec_args);

            cellar
                .exec_builder(exec_path)
                .args(exec_args)
                .workdir(workdir)
                .run()?;
        }

        Some(("kill", _)) => {
            println!("Killing prefix at {:?}", cellar.path());
            cellar.kill();
        }

        Some((name, _)) => println!("Unknown or unimplemented command {}", name),
        None => panic!("required argument not passed"),
    }

    Ok(())
}
