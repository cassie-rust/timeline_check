use std::{
    fs::File,
    io::{self, BufRead},
    path::{Path, PathBuf},
};

use clap::{command, Parser, Subcommand};
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions, PgRow, PgSslMode},
    Row,
};
use tokio::join;

#[derive(Parser, Debug)]
#[command(about, long_about = None)]
struct Cli {
    /// User
    #[arg(short, long)]
    user: String,

    /// Password
    #[arg(short, long)]
    password: String,

    /// File with hosts to connect to
    #[arg(long)]
    hosts: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 🔐️ For hosts that require cert authentication
    Cert {
        #[arg(short, long)]
        /// Root CA certificate file path
        root_cert: PathBuf,

        #[arg(long)]
        /// Client certificate file path
        client_cert: PathBuf,

        #[arg(long)]
        /// Client certificate key file path
        client_key: PathBuf,
    },
    NoCert,
}

#[derive(Debug)]
struct Host {
    name: String,
    is_primary: bool,
    timeline_id: i32,
    replica_attached: bool,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let hosts = match read_lines(cli.hosts) {
        Ok(lines) => lines.map(|l| l.unwrap()),
        Err(e) => panic!("Error reading file: {}", e),
    };

    let conn = PgConnectOptions::new()
        // TODO: config these two
        .port(5432)
        .database("postgres")
        .username(&cli.user)
        .password(&cli.password);

    let conn = match &cli.command {
        Commands::Cert {
            root_cert,
            client_cert,
            client_key,
        } => conn
            .ssl_mode(PgSslMode::Require)
            .ssl_root_cert(root_cert)
            .ssl_client_cert(client_cert)
            .ssl_client_key(client_key),
        Commands::NoCert => conn.ssl_mode(PgSslMode::Prefer),
    };

    let mut res = Vec::new();

    // TODO: This should be done through spawned tasks, it takes like 7 seconds/host atm
    for host in hosts {
        let conn = conn.clone().host(&host);
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect_with(conn)
            .await
            .map_err(|e| {
                println!("Error connecting to host: {}", host);
                println!("{}", e);
                e
            });

        if pool.is_err() {
            continue;
        }

        let pool = pool.unwrap();

        let is_primary = sqlx::query("SELECT pg_is_in_recovery();")
            .map(|r: PgRow| {
                let b: bool = r.get("pg_is_in_recovery");
                !b
            })
            .fetch_one(&pool);

        let timeline_id = sqlx::query("SELECT timeline_id from pg_control_checkpoint();")
            .map(|r: PgRow| {
                let b: i32 = r.get("timeline_id");
                b
            })
            .fetch_one(&pool);

        let replica_attached = sqlx::query("SELECT EXISTS (select 1 from pg_stat_replication);")
            .map(|r: PgRow| {
                let b: bool = r.get("exists");
                b
            })
            .fetch_one(&pool);

        let (is_primary, timeline_id, replica_attached) =
            join!(is_primary, timeline_id, replica_attached);

        res.push(Host {
            name: host,
            is_primary: is_primary.unwrap(),
            timeline_id: timeline_id.unwrap(),
            replica_attached: replica_attached.unwrap(),
        })
    }

    for r in res {
        println!(
            "{}, {}, {}, {}",
            r.name, r.is_primary, r.timeline_id, r.replica_attached
        );
    }
}

fn read_lines<P: AsRef<Path>>(filename: P) -> io::Result<io::Lines<io::BufReader<File>>> {
    let file = File::open(filename)?;
    Ok(io::BufReader::new(file).lines())
}
