// SPDX-License-Identifier: AGPL-3.0-or-later
//! Own PostgreSQL for tests of pgvector (contract Round 12, §0; also for the integrator through
//! `#[path = "pgvector_support/mod.rs"] mod pgvector_support;`). Never the pgvector of the user.
//!
//! Order ([`start`]):
//! 1. `podman run` with a **locally present** image `docker.io/pgvector/pgvector:pg17` or `:pg16` (the test loads
//!    nothing from the network), port only on `127.0.0.1` (random, `-p 127.0.0.1::5432`), name
//!    `baruch-r12-<aufgabe>-<pid>-<n>`; [`Drop`] removes exactly this container (`podman rm -f`). Foreign containers
//!    and images stay untouched (no `podman rmi`).
//! 2. otherwise a throwaway cluster from `/usr/lib/postgresql/<n>/bin` (`initdb`, `pg_ctl`) in the build folder –
//!    **without** pgvector ([`Pg::vector`] = `false`); [`Drop`] stops only this data folder.
//! 3. otherwise `None` with `ÜBERSPRUNGEN: …` on stderr.
//!
//! One server for all (Round 13): since the server tests are one binary (`tests/main.rs`), tests running at the same
//! time share **one** server (the start takes 30–45 s); every test gets a fresh database of its own in it (`rag_<n>`),
//! so that tables such as `chunks` do not get in each other's way. The server ends with the last [`Pg`] (after that a
//! later test starts a new one).
//!
//! User `probe` with password [`PASSWORD`] (scram-sha-256, superuser of the throwaway server), database
//! [`Pg::database`]. Test data freely invented.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use sqlx::Connection;
use sqlx::postgres::{PgConnectOptions, PgConnection, PgSslMode};

/// Password of the test user (only for this throwaway server).
pub const PASSWORD: &str = "Wegwerf-Pg-7c1e-Probe";
pub const USER: &str = "probe";
/// Database that the server creates at start (only for creating the test databases).
pub const DATABASE: &str = "rag";

/// Images in this order (only locally present ones).
pub const IMAGES: [&str; 2] = ["docker.io/pgvector/pgvector:pg17", "docker.io/pgvector/pgvector:pg16"];

static COUNTER: AtomicUsize = AtomicUsize::new(0);
/// The shared server currently running (lives as long as a [`Pg`] holds it).
static SHARED: tokio::sync::Mutex<Weak<Instance>> = tokio::sync::Mutex::const_new(Weak::new());

enum Backend {
    Container { name: String },
    Cluster { pg_ctl: PathBuf, data: PathBuf, _dir: tempfile::TempDir },
}

/// A running throwaway server (shared by tests running at the same time).
struct Instance {
    port: u16,
    vector: bool,
    origin: String,
    backend: Backend,
}

/// Own database of a test on the shared throwaway server.
pub struct Pg {
    pub port: u16,
    /// Does the extension `vector` exist (container yes, initdb cluster no)?
    pub vector: bool,
    /// From where: image of the container or „initdb …“.
    pub origin: String,
    /// Own database of this test (`rag_<n>`).
    pub database: String,
    _server: Arc<Instance>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        match &self.backend {
            Backend::Container { name } => {
                let _ = Command::new("podman").args(["rm", "-f", "-t", "0", name]).output();
            }
            Backend::Cluster { pg_ctl, data, .. } => {
                let _ = Command::new(pg_ctl).arg("-D").arg(data).args(["-m", "immediate", "-w", "stop"]).output();
            }
        }
    }
}

impl Pg {
    /// Credentials of the test user.
    pub fn options(&self) -> PgConnectOptions {
        PgConnectOptions::new_without_pgpass()
            .host("127.0.0.1")
            .port(self.port)
            .username(USER)
            .password(PASSWORD)
            .database(&self.database)
            .ssl_mode(PgSslMode::Disable)
    }

    /// Own connection (test user, may write – only for creating the test data).
    pub async fn connect(&self) -> PgConnection {
        PgConnection::connect_with(&self.options()).await.expect("Verbindung zum Wegwerf-Server")
    }

    /// Several statements without parameters (creating test data).
    pub async fn exec(&self, sql: &str) {
        let mut conn = self.connect().await;
        sqlx::raw_sql(sqlx::AssertSqlSafe(sql)).execute(&mut conn).await.unwrap_or_else(|e| panic!("{sql}: {e}"));
        let _ = conn.close().await;
    }

    /// Settings of the connection pgvector for Baruch (without password; that belongs in the keyring).
    pub fn config(&self) -> Value {
        json!({ "host": "127.0.0.1", "port": self.port, "database": self.database, "user": USER, "tls": "disable" })
    }
}

fn podman_ok() -> bool {
    Command::new("podman").arg("--version").output().is_ok_and(|o| o.status.success())
}

fn image_present(image: &str) -> bool {
    Command::new("podman").args(["image", "exists", image]).output().is_ok_and(|o| o.status.success())
}

/// Waits until the test user can log in (at most `limit`).
async fn ready(port: u16, limit: Duration) -> Result<(), String> {
    let options = PgConnectOptions::new_without_pgpass()
        .host("127.0.0.1")
        .port(port)
        .username(USER)
        .password(PASSWORD)
        .database(DATABASE)
        .ssl_mode(PgSslMode::Disable);
    let start = Instant::now();
    let mut last = String::new();
    while start.elapsed() < limit {
        match PgConnection::connect_with(&options).await {
            Ok(conn) => {
                let _ = conn.close().await;
                return Ok(());
            }
            Err(e) => last = e.to_string(),
        }
        tokio::time::sleep(Duration::from_millis(300)).await;
    }
    Err(format!("nicht bereit nach {} s: {last}", limit.as_secs()))
}

async fn start_container(task: &str, image: &str) -> Result<Instance, String> {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let name = format!("baruch-r12-{task}-{}-{n}", std::process::id());
    let output = Command::new("podman")
        .args(["run", "-d", "--name", &name, "-p", "127.0.0.1::5432"])
        .args(["-e", &format!("POSTGRES_USER={USER}")])
        .args(["-e", &format!("POSTGRES_PASSWORD={PASSWORD}")])
        .args(["-e", &format!("POSTGRES_DB={DATABASE}")])
        .args(["-e", "POSTGRES_HOST_AUTH_METHOD=scram-sha-256"])
        .args(["-e", "POSTGRES_INITDB_ARGS=--auth-host=scram-sha-256"])
        .arg(image)
        .output()
        .map_err(|e| format!("podman run: {e}"))?;
    // From here on the name belongs to us: remove it again on an error too.
    let pg = |port| Instance {
        port,
        vector: true,
        origin: image.to_owned(),
        backend: Backend::Container { name: name.clone() },
    };
    if !output.status.success() {
        drop(pg(0));
        return Err(format!("podman run {image}: {}", String::from_utf8_lossy(&output.stderr).trim()));
    }
    let port_out = Command::new("podman").args(["port", &name, "5432/tcp"]).output().map_err(|e| e.to_string())?;
    let text = String::from_utf8_lossy(&port_out.stdout).to_string();
    let port = text.lines().next().and_then(|l| l.rsplit(':').next()).and_then(|p| p.trim().parse::<u16>().ok());
    let Some(port) = port else {
        drop(pg(0));
        return Err(format!("podman port {name}: keine Portangabe ({text:?})"));
    };
    let server = pg(port);
    ready(port, Duration::from_secs(90)).await?;
    Ok(server)
}

/// `/usr/lib/postgresql/<n>/bin` with `initdb`, `pg_ctl`, `postgres`; highest version first.
fn postgres_bin() -> Option<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir("/usr/lib/postgresql")
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path().join("bin")))
        .filter(|bin| ["initdb", "pg_ctl", "postgres"].iter().all(|p| bin.join(p).is_file()))
        .collect();
    found.sort();
    found.pop()
}

fn run(program: &Path, args: &[&std::ffi::OsStr]) -> Result<(), String> {
    let output = Command::new(program).args(args).output().map_err(|e| e.to_string())?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{}: {}{}",
            program.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        ))
    }
}

async fn free_port() -> u16 {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    listener.local_addr().unwrap().port()
}

/// Throwaway cluster without pgvector (in the build folder, not under /tmp).
async fn start_cluster(bin: &Path) -> Result<Instance, String> {
    let dir = tempfile::tempdir_in(env!("CARGO_TARGET_TMPDIR")).map_err(|e| e.to_string())?;
    let data = dir.path().join("daten");
    let pwfile = dir.path().join("passwort");
    std::fs::write(&pwfile, format!("{PASSWORD}\n")).map_err(|e| e.to_string())?;
    run(
        &bin.join("initdb"),
        &[
            "-D".as_ref(),
            data.as_os_str(),
            "-U".as_ref(),
            USER.as_ref(),
            "-A".as_ref(),
            "scram-sha-256".as_ref(),
            format!("--pwfile={}", pwfile.display()).as_ref(),
            "-E".as_ref(),
            "UTF8".as_ref(),
            "--locale=C".as_ref(),
            "--no-sync".as_ref(),
            "--no-instructions".as_ref(),
        ],
    )?;
    let port = free_port().await;
    let options = format!("-p {port} -c listen_addresses=127.0.0.1 -c unix_socket_directories='' -c fsync=off");
    let pg_ctl = bin.join("pg_ctl");
    run(
        &pg_ctl,
        &[
            "-D".as_ref(),
            data.as_os_str(),
            "-l".as_ref(),
            data.join("log.txt").as_os_str(),
            "-o".as_ref(),
            options.as_ref(),
            "-w".as_ref(),
            "-t".as_ref(),
            "60".as_ref(),
            "start".as_ref(),
        ],
    )?;
    let server = Instance {
        port,
        vector: false,
        origin: format!("initdb {}", bin.display()),
        backend: Backend::Cluster { pg_ctl, data, _dir: dir },
    };
    // Create database `rag` (the cluster only has `postgres`).
    let admin = PgConnectOptions::new_without_pgpass()
        .host("127.0.0.1")
        .port(port)
        .username(USER)
        .password(PASSWORD)
        .database("postgres")
        .ssl_mode(PgSslMode::Disable);
    let mut conn = PgConnection::connect_with(&admin).await.map_err(|e| e.to_string())?;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {DATABASE}")))
        .execute(&mut conn)
        .await
        .map_err(|e| e.to_string())?;
    let _ = conn.close().await;
    Ok(server)
}

/// Own database on the shared throwaway server (see module docs); if none is running, one is started (`task` goes into
/// the container name). `None` = skipped (the message is on stderr).
pub async fn start(task: &str) -> Option<Pg> {
    let server = {
        let mut shared = SHARED.lock().await;
        match shared.upgrade() {
            Some(server) => server,
            None => {
                let server = Arc::new(start_instance(task).await?);
                *shared = Arc::downgrade(&server);
                server
            }
        }
    };
    let database = format!("rag_{}", COUNTER.fetch_add(1, Ordering::SeqCst));
    let admin = PgConnectOptions::new_without_pgpass()
        .host("127.0.0.1")
        .port(server.port)
        .username(USER)
        .password(PASSWORD)
        .database(DATABASE)
        .ssl_mode(PgSslMode::Disable);
    let mut conn = PgConnection::connect_with(&admin).await.expect("Verbindung zum Wegwerf-Server");
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&mut conn)
        .await
        .unwrap_or_else(|e| panic!("CREATE DATABASE {database}: {e}"));
    let _ = conn.close().await;
    Some(Pg { port: server.port, vector: server.vector, origin: server.origin.clone(), database, _server: server })
}

/// Start a new throwaway server: container, otherwise initdb cluster, otherwise `None`.
async fn start_instance(task: &str) -> Option<Instance> {
    let mut why = Vec::new();
    if podman_ok() {
        for image in IMAGES {
            if !image_present(image) {
                why.push(format!("{image} nicht lokal vorhanden"));
                continue;
            }
            match start_container(task, image).await {
                Ok(pg) => return Some(pg),
                Err(e) => why.push(e),
            }
        }
    } else {
        why.push("kein podman".to_owned());
    }
    match postgres_bin() {
        Some(bin) => match start_cluster(&bin).await {
            Ok(pg) => {
                eprintln!("Hinweis: kein pgvector-Container ({}); initdb-Cluster ohne pgvector", why.join("; "));
                return Some(pg);
            }
            Err(e) => why.push(e),
        },
        None => why.push("kein PostgreSQL unter /usr/lib/postgresql/*/bin".to_owned()),
    }
    eprintln!("ÜBERSPRUNGEN: kein eigenes PostgreSQL für den Test ({})", why.join("; "));
    None
}

/// Mock embedder with fixed vectors (no Eliezer): one fixed vector per text, otherwise `default`; answers as `model`
/// with `answer_model`.
pub struct FixedEmbedder {
    pub requested: String,
    pub answer_model: String,
    pub table: Vec<(String, Vec<f32>)>,
    pub default: Vec<f32>,
    pub calls: std::sync::Mutex<Vec<Vec<String>>>,
}

impl baruch_rag::Embedder for FixedEmbedder {
    fn model(&self) -> &str {
        &self.requested
    }

    fn embed<'a>(
        &'a self,
        texts: &'a [String],
    ) -> baruch_rag::BoxFuture<'a, Result<baruch_rag::Embeddings, baruch_rag::RagError>> {
        Box::pin(async move {
            self.calls.lock().unwrap().push(texts.to_vec());
            let vectors: Vec<Vec<f32>> = texts
                .iter()
                .map(|t| self.table.iter().find(|(k, _)| k == t).map_or(self.default.clone(), |(_, v)| v.clone()))
                .collect();
            let dimension = vectors.first().map_or(0, Vec::len);
            Ok(baruch_rag::Embeddings { model: self.answer_model.clone(), dimension, vectors })
        })
    }
}

/// Factory for [`FixedEmbedder`]; remembers for which model it was asked.
pub struct FixedFactory {
    pub answer_model: Option<String>,
    pub table: Vec<(String, Vec<f32>)>,
    pub default: Vec<f32>,
    pub made: std::sync::Mutex<Vec<String>>,
    pub last: std::sync::Mutex<Option<std::sync::Arc<FixedEmbedder>>>,
}

impl FixedFactory {
    pub fn new(table: Vec<(&str, Vec<f32>)>, default: Vec<f32>) -> FixedFactory {
        FixedFactory {
            answer_model: None,
            table: table.into_iter().map(|(k, v)| (k.to_owned(), v)).collect(),
            default,
            made: Default::default(),
            last: Default::default(),
        }
    }

    /// Number of embedding requests of the embedder built last.
    pub fn calls(&self) -> usize {
        self.last.lock().unwrap().as_ref().map_or(0, |e| e.calls.lock().unwrap().len())
    }
}

impl baruch_rag::EmbedderFactory for FixedFactory {
    fn for_model(&self, model: &str) -> std::sync::Arc<dyn baruch_rag::Embedder> {
        self.made.lock().unwrap().push(model.to_owned());
        let embedder = std::sync::Arc::new(FixedEmbedder {
            requested: model.to_owned(),
            answer_model: self.answer_model.clone().unwrap_or_else(|| model.to_owned()),
            table: self.table.clone(),
            default: self.default.clone(),
            calls: Default::default(),
        });
        *self.last.lock().unwrap() = Some(embedder.clone());
        embedder
    }
}
