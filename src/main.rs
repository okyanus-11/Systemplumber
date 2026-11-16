use std::path::PathBuf;

use clap::{Parser, Subcommand};

mod backends;
mod bootstrap;
mod bootstrap_init;
mod catalog;
mod database;
mod dependency;
mod install;
mod mirror;
mod mirror_selector;
mod mirrorlist;
mod package;
mod parsers;
mod repositories;
mod repository;
mod root;
mod sync;

use bootstrap::BootstrapConfig;
use root::Root;
use sync::Sync;

#[derive(Parser, Debug)]
#[command(
    name = "flush",
    version,
    about = "SystemPlumber — a configurable Linux package and system manager"
)]
struct Cli {
    /// Target filesystem root
    #[arg(long, short = 'r', global = true)]
    root: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Build or configure a Linux system
    Bootstrap {
        #[command(subcommand)]
        command: Option<BootstrapCommand>,
    },

    /// Install packages
    Install {
        /// Packages to install
        packages: Vec<String>,
    },

    /// Remove packages
    Remove {
        /// Packages to remove
        packages: Vec<String>,
    },

    /// Update package metadata
    Update,

    /// Synchronize package metadata
    Sync,

    /// Search for packages
    Search {
        /// Search query
        query: String,
    },

    /// List installed packages
    List,

    /// Adopt packages from another package manager
    Adopt,

    /// Declarative system management
    Forge,
}

#[derive(Subcommand, Debug)]
enum BootstrapCommand {
    /// Configure the bootstrap system
    Config {
        /// Reset configuration to SystemPlumber defaults
        #[arg(long)]
        default: bool,

        /// Set a configuration value
        #[arg(long, value_name = "KEY=VALUE")]
        set: Option<String>,
    },
}

fn unavailable_command(name: &str) -> ! {
    eprintln!(
        "ERROR: command '{}' is not implemented yet; no system changes were made.",
        name
    );
    std::process::exit(2);
}

fn main() {
    let cli = Cli::parse();

    let root = cli.root.map(Root::new).unwrap_or_else(Root::host);

    match cli.command {
        Some(Commands::Bootstrap { command }) => match command {
            Some(BootstrapCommand::Config {
                default: true,
                set: None,
            }) => {
                let config = BootstrapConfig::default();
                let path = root.bootstrap_config();

                match config.save(&path) {
                    Ok(()) => {
                        println!("Reset bootstrap configuration.");
                        println!("Written to: {}", path.display());
                    }

                    Err(error) => {
                        eprintln!("ERROR: failed to save bootstrap configuration: {error}");
                        std::process::exit(1);
                    }
                }
            }
            Some(BootstrapCommand::Config {
                default: false,
                set: None,
            }) => {
                let path = root.bootstrap_config();

                match BootstrapConfig::load(&path) {
                    Ok(config) => {
                        println!("{config:#?}");
                    }

                    Err(error) => {
                        eprintln!("ERROR: failed to load bootstrap configuration: {error}");
                        std::process::exit(1);
                    }
                }
            }

            Some(BootstrapCommand::Config {
                default,
                set: Some(value),
            }) => {
                if default {
                    eprintln!("ERROR: --default and --set cannot be used together");
                    std::process::exit(1);
                }

                let (key, new_value) = match value.split_once('=') {
                    Some((key, value)) if !key.is_empty() && !value.is_empty() => (key, value),

                    _ => {
                        eprintln!("ERROR: --set must use KEY=VALUE format");
                        std::process::exit(1);
                    }
                };

                let path = root.bootstrap_config();

                let mut config = match BootstrapConfig::load(&path) {
                    Ok(config) => config,

                    Err(error) => {
                        eprintln!("ERROR: failed to load bootstrap configuration: {error}");
                        std::process::exit(1);
                    }
                };

                if let Err(error) = config.set(key, new_value) {
                    eprintln!("ERROR: {error}");
                    std::process::exit(1);
                }

                match config.save(&path) {
                    Ok(()) => {
                        println!("Updated {key}.");
                        println!("Written to: {}", path.display());
                    }

                    Err(error) => {
                        eprintln!("ERROR: failed to save bootstrap configuration: {error}");
                        std::process::exit(1);
                    }
                }
            }
            None => {
                println!("bootstrap");
                println!("root: {}", root.path().display());
            }
        },
        Some(Commands::Install { packages }) => {
            let database_path = root.package_db().join("packages.db");

            let catalog = match catalog::PackageCatalog::open(&database_path) {
                Ok(catalog) => catalog,
                Err(error) => {
                    eprintln!("ERROR: failed to open package catalog: {error}");
                    std::process::exit(1);
                }
            };

            let mirrorlist = match mirrorlist::MirrorList::load(&root.mirrorlist()) {
                Ok(mirrorlist) => mirrorlist,
                Err(error) => {
                    eprintln!("ERROR: failed to load mirrorlist: {error}");
                    std::process::exit(1);
                }
            };

            let system = bootstrap_init::BootstrapSystem::new(root.clone());

            let mut installer = install::Installer::new(system, catalog, mirrorlist);

            for package in packages {
                if let Err(error) = installer.install(&package) {
                    eprintln!("ERROR: failed to install {package}: {error}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::Remove { .. }) => unavailable_command("remove"),

        Some(Commands::Update) => unavailable_command("update"),
        Some(Commands::Search { query }) => {
            let database_path = root.package_db().join("packages.db");

            let mut catalog = match catalog::PackageCatalog::open(&database_path) {
                Ok(catalog) => catalog,
                Err(error) => {
                    eprintln!("ERROR: failed to open package database: {error}");
                    std::process::exit(1);
                }
            };

            match catalog.search(&query) {
                Ok(packages) => {
                    if packages.is_empty() {
                        println!("Catalog empty.");
                    } else {
                        for package in packages {
                            println!(
                                "{} {} [{}] - {}",
                                package.name,
                                package.version,
                                package.architecture,
                                package.description
                            );
                            if let Some(filename) = &package.filename {
                                println!("  payload: {}", filename);
                            }

                            if let Some(checksum) = &package.checksum {
                                println!("  sha256: {}", checksum);
                            }
                        }
                    }
                }

                Err(error) => {
                    eprintln!("ERROR: package catalog search failed: {error}");
                    std::process::exit(1);
                }
            }
        }
        Some(Commands::List) => {
            let database_path = root.package_db().join("packages.db");

            match database::PackageDatabase::load(&database_path) {
                Ok(database) => {
                    if database.packages.is_empty() {
                        println!("No packages found.");
                    } else {
                        for package in database.packages {
                            println!(
                                "{} {} [{}]",
                                package.name, package.version, package.architecture
                            );
                        }
                    }
                }
                Err(error) => {
                    eprintln!("ERROR: failed to load package database: {error}");
                    std::process::exit(1);
                }
            }
        }

        Some(Commands::Adopt) => unavailable_command("adopt"),

        Some(Commands::Forge) => unavailable_command("forge"),
        Some(Commands::Sync) => {
            let sync = Sync::new(root);

            if let Err(error) = sync.run() {
                eprintln!("ERROR: sync failed: {error}");
                std::process::exit(1);
            }
        }
        None => {
            println!("SystemPlumber: flush");
            println!("Run `flush --help` for usage.");
        }
    }
}
