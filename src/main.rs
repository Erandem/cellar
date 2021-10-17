mod cellar;

use cellar::WineCellar;
use clap::{App, Arg};

use std::path::PathBuf;

fn main() -> cellar::Result<()> {
    let matches = App::new("Wine Cellar")
        .version("1.0")
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
                .arg(Arg::new("executable").required(true).takes_value(true))
                .arg(Arg::new("rel-c").about("Resolves paths relative to C: in the prefix")),
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
                exec_path = args.value_of_t_or_exit("executable");
            }

            println!("Running {:?} with wine version {}", exec_path, "TODO");

            // direct path expected
            //cellar.exec(exec_path)?;
        }

        Some(("kill", _)) => {
            cellar.kill();
        }

        Some((name, _)) => println!("Unknown or unimplemented command {}", name),
        None => panic!("required argument not passed"),
    }

    Ok(())
}
