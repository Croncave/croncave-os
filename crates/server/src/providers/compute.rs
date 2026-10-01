//! The compute driver: the only code that knows which provider runs computers.
//!
//! Five operations: create, start, stop, status and destroy. `local` runs each computer
//! as a supervised agent process with its own disk folder; `docker` runs it in a
//! container; `fly` is a stub until the founder's account exists.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use tokio::process::Command;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct ComputerSpec {
    pub id: Uuid,
    pub cpu: i32,
    pub memory_gb: i32,
    pub disk_gb: i32,
}

#[derive(Debug, Clone)]
pub struct Boot {
    pub relay_url: String,
    pub bootstrap_token: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriverStatus {
    Running,
    Stopped,
    Missing,
}

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("{0}")]
    NotConfigured(String),
    #[error("{0}")]
    Failed(String),
}

#[async_trait]
pub trait ComputeDriver: Send + Sync {
    fn name(&self) -> &'static str;
    /// Make the computer and its disk. Returns the provider's reference for it.
    async fn create(&self, spec: &ComputerSpec) -> Result<String, DriverError>;
    /// Boot it; the agent inside dials out to the relay with the bootstrap token.
    async fn start(&self, compute_ref: &str, spec: &ComputerSpec, boot: &Boot) -> Result<(), DriverError>;
    /// Stop it, keeping its disk.
    async fn stop(&self, compute_ref: &str) -> Result<(), DriverError>;
    async fn status(&self, compute_ref: &str) -> Result<DriverStatus, DriverError>;
    /// Remove it and its disk.
    async fn destroy(&self, compute_ref: &str) -> Result<(), DriverError>;
}

pub fn from_config(cfg: &crate::config::Config) -> anyhow::Result<std::sync::Arc<dyn ComputeDriver>> {
    Ok(match cfg.compute_driver.as_str() {
        "local" => std::sync::Arc::new(LocalDriver::new(cfg.data_dir.join("computers"), absolute(&cfg.agent_bin)?)),
        "docker" => std::sync::Arc::new(DockerDriver::new(
            cfg.data_dir.join("computers"),
            absolute(&cfg.agent_bin)?,
            cfg.docker_image.clone(),
        )),
        "fly" => std::sync::Arc::new(FlyDriver { token: cfg.fly_api_token.clone(), app: cfg.fly_app.clone() }),
        other => anyhow::bail!("COMPUTE_DRIVER={other} isn't a driver (use local, docker or fly)"),
    })
}

fn absolute(p: &Path) -> anyhow::Result<PathBuf> {
    Ok(if p.is_absolute() { p.to_path_buf() } else { std::env::current_dir()?.join(p) })
}

async fn alive(pid: u32) -> bool {
    Command::new("kill").args(["-0", &pid.to_string()]).status().await.map(|s| s.success()).unwrap_or(false)
}

async fn signal_group(pid: u32, signal: &str) {
    let _ = Command::new("kill").args(["-s", signal, "--", &format!("-{pid}")]).status().await;
}

// ---------------------------------------------------------------------------------------

/// Each computer is an agent process in its own process group, with a disk folder.
/// Processes outlive a control plane restart (work never depends on the platform being
/// up); their pid is kept beside the disk so status and stop still work afterwards.
pub struct LocalDriver {
    base: PathBuf,
    agent_bin: PathBuf,
    children: Mutex<HashMap<String, tokio::process::Child>>,
}

impl LocalDriver {
    pub fn new(base: PathBuf, agent_bin: PathBuf) -> Self {
        Self { base, agent_bin, children: Mutex::new(HashMap::new()) }
    }

    fn dir(&self, compute_ref: &str) -> Result<PathBuf, DriverError> {
        if compute_ref.is_empty() || !compute_ref.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
            return Err(DriverError::Failed("bad computer reference".into()));
        }
        Ok(self.base.join(compute_ref))
    }

    fn pid(&self, compute_ref: &str) -> Option<u32> {
        std::fs::read_to_string(self.dir(compute_ref).ok()?.join("agent.pid")).ok()?.trim().parse().ok()
    }

    /// Where a computer's disk lives (for tests and the docs).
    pub fn disk(&self, compute_ref: &str) -> PathBuf {
        self.base.join(compute_ref).join("disk")
    }
}

#[async_trait]
impl ComputeDriver for LocalDriver {
    fn name(&self) -> &'static str {
        "local"
    }

    async fn create(&self, spec: &ComputerSpec) -> Result<String, DriverError> {
        let compute_ref = format!("local-{}", spec.id.simple());
        let dir = self.dir(&compute_ref)?;
        std::fs::create_dir_all(dir.join("disk/root")).map_err(|e| DriverError::Failed(e.to_string()))?;
        Ok(compute_ref)
    }

    async fn start(&self, compute_ref: &str, spec: &ComputerSpec, boot: &Boot) -> Result<(), DriverError> {
        if self.status(compute_ref).await? == DriverStatus::Running {
            return Ok(());
        }
        let dir = self.dir(compute_ref)?;
        if !dir.exists() {
            return Err(DriverError::Failed("this computer's disk is missing".into()));
        }
        if !self.agent_bin.exists() {
            return Err(DriverError::Failed(format!(
                "the agent binary isn't built at {} (run cargo build)",
                self.agent_bin.display()
            )));
        }
        let log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join("agent.log"))
            .map_err(|e| DriverError::Failed(e.to_string()))?;
        let log2 = log.try_clone().map_err(|e| DriverError::Failed(e.to_string()))?;
        let child = Command::new(&self.agent_bin)
            .env_clear()
            .env("PATH", std::env::var("PATH").unwrap_or_default())
            .env("HOME", dir.join("disk/root"))
            .env("LANG", "C.UTF-8")
            .env("CRONCAVE_RELAY_URL", &boot.relay_url)
            .env("CRONCAVE_COMPUTER_ID", spec.id.to_string())
            .env("CRONCAVE_BOOTSTRAP_TOKEN", &boot.bootstrap_token)
            .env("CRONCAVE_DISK", dir.join("disk"))
            .envs(passthrough_env())
            .stdin(std::process::Stdio::null())
            .stdout(log)
            .stderr(log2)
            .process_group(0)
            .spawn()
            .map_err(|e| DriverError::Failed(format!("couldn't start the agent: {e}")))?;
        let pid = child.id().unwrap_or_default();
        std::fs::write(dir.join("agent.pid"), pid.to_string()).map_err(|e| DriverError::Failed(e.to_string()))?;
        self.children.lock().expect("children").insert(compute_ref.to_string(), child);
        Ok(())
    }

    async fn stop(&self, compute_ref: &str) -> Result<(), DriverError> {
        let child = self.children.lock().expect("children").remove(compute_ref);
        if let Some(pid) = self.pid(compute_ref) {
            signal_group(pid, "TERM").await;
            for _ in 0..30 {
                if !alive(pid).await {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
            signal_group(pid, "KILL").await;
        }
        if let Some(mut c) = child {
            let _ = c.wait().await;
        }
        let _ = std::fs::remove_file(self.dir(compute_ref)?.join("agent.pid"));
        Ok(())
    }

    async fn status(&self, compute_ref: &str) -> Result<DriverStatus, DriverError> {
        let dir = self.dir(compute_ref)?;
        if !dir.exists() {
            return Ok(DriverStatus::Missing);
        }
        // Reap a child that exited so it isn't reported as running.
        if let Some(c) = self.children.lock().expect("children").get_mut(compute_ref)
            && let Ok(Some(_)) = c.try_wait()
        {
            let _ = std::fs::remove_file(dir.join("agent.pid"));
        }
        match self.pid(compute_ref) {
            Some(pid) if alive(pid).await => Ok(DriverStatus::Running),
            _ => Ok(DriverStatus::Stopped),
        }
    }

    async fn destroy(&self, compute_ref: &str) -> Result<(), DriverError> {
        self.stop(compute_ref).await?;
        let dir = self.dir(compute_ref)?;
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| DriverError::Failed(e.to_string()))?;
        }
        Ok(())
    }
}

/// Variables a computer's programs need from the host in the local driver (proxy and CA
/// settings, so watches can reach the web through a corporate proxy).
fn passthrough_env() -> Vec<(String, String)> {
    [
        "HTTPS_PROXY",
        "https_proxy",
        "HTTP_PROXY",
        "http_proxy",
        "NO_PROXY",
        "no_proxy",
        "SSL_CERT_FILE",
        "NODE_EXTRA_CA_CERTS",
        "REQUESTS_CA_BUNDLE",
        "PIP_CERT",
        "CRONCAVE_PYTHON",
        "CRONCAVE_MOCK_CODER_PAUSE_MS",
        "CRONCAVE_AGENT_LOG",
    ]
    .iter()
    .filter_map(|k| std::env::var(k).ok().map(|v| (k.to_string(), v)))
    .collect()
}

// ---------------------------------------------------------------------------------------

/// Each computer is a container with its disk mounted, its own network namespace and
/// CPU and memory limits. The agent binary is mounted read-only. The relay address must
/// be reachable from containers (e.g. `RELAY_URL=http://host.docker.internal:8080` with
/// the control plane listening on 0.0.0.0).
pub struct DockerDriver {
    base: PathBuf,
    agent_bin: PathBuf,
    image: String,
}

impl DockerDriver {
    pub fn new(base: PathBuf, agent_bin: PathBuf, image: String) -> Self {
        Self { base, agent_bin, image }
    }

    async fn docker(args: &[&str]) -> Result<String, DriverError> {
        let out = Command::new("docker")
            .args(args)
            .output()
            .await
            .map_err(|e| DriverError::NotConfigured(format!("docker isn't available: {e}")))?;
        if !out.status.success() {
            return Err(DriverError::Failed(String::from_utf8_lossy(&out.stderr).trim().to_string()));
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
    }
}

#[async_trait]
impl ComputeDriver for DockerDriver {
    fn name(&self) -> &'static str {
        "docker"
    }

    async fn create(&self, spec: &ComputerSpec) -> Result<String, DriverError> {
        let compute_ref = format!("croncave-{}", spec.id.simple());
        std::fs::create_dir_all(self.base.join(&compute_ref).join("disk/root"))
            .map_err(|e| DriverError::Failed(e.to_string()))?;
        Ok(compute_ref)
    }

    async fn start(&self, compute_ref: &str, spec: &ComputerSpec, boot: &Boot) -> Result<(), DriverError> {
        let _ = Self::docker(&["rm", "-f", compute_ref]).await;
        let disk = std::fs::canonicalize(self.base.join(compute_ref).join("disk"))
            .map_err(|e| DriverError::Failed(e.to_string()))?;
        let cpus = spec.cpu.to_string();
        let memory = format!("{}g", spec.memory_gb);
        let disk_mount = format!("{}:/croncave/disk", disk.display());
        let agent_mount = format!("{}:/usr/local/bin/croncave-agent:ro", self.agent_bin.display());
        let env = [
            format!("CRONCAVE_RELAY_URL={}", boot.relay_url),
            format!("CRONCAVE_COMPUTER_ID={}", spec.id),
            format!("CRONCAVE_BOOTSTRAP_TOKEN={}", boot.bootstrap_token),
            "CRONCAVE_DISK=/croncave/disk".to_string(),
        ];
        let mut args = vec![
            "run",
            "-d",
            "--name",
            compute_ref,
            "--cpus",
            &cpus,
            "--memory",
            &memory,
            "--add-host",
            "host.docker.internal:host-gateway",
            "-v",
            &disk_mount,
            "-v",
            &agent_mount,
        ];
        for e in &env {
            args.push("-e");
            args.push(e);
        }
        args.push(&self.image);
        args.push("/usr/local/bin/croncave-agent");
        Self::docker(&args).await.map(|_| ())
    }

    async fn stop(&self, compute_ref: &str) -> Result<(), DriverError> {
        let _ = Self::docker(&["stop", "-t", "3", compute_ref]).await;
        let _ = Self::docker(&["rm", "-f", compute_ref]).await;
        Ok(())
    }

    async fn status(&self, compute_ref: &str) -> Result<DriverStatus, DriverError> {
        if !self.base.join(compute_ref).exists() {
            return Ok(DriverStatus::Missing);
        }
        match Self::docker(&["inspect", "-f", "{{.State.Running}}", compute_ref]).await {
            Ok(s) if s == "true" => Ok(DriverStatus::Running),
            Ok(_) | Err(DriverError::Failed(_)) => Ok(DriverStatus::Stopped),
            Err(e) => Err(e),
        }
    }

    async fn destroy(&self, compute_ref: &str) -> Result<(), DriverError> {
        self.stop(compute_ref).await?;
        let dir = self.base.join(compute_ref);
        if dir.exists() {
            std::fs::remove_dir_all(dir).map_err(|e| DriverError::Failed(e.to_string()))?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------------------

/// Fly Machines, once the founder's account exists. Every call says so until then.
pub struct FlyDriver {
    pub token: Option<String>,
    pub app: Option<String>,
}

impl FlyDriver {
    fn not_configured(&self) -> DriverError {
        let missing = match (&self.token, &self.app) {
            (None, _) => "FLY_API_TOKEN",
            (_, None) => "FLY_APP",
            _ => "a Fly Machines implementation",
        };
        DriverError::NotConfigured(format!("The Fly driver is not configured: it needs {missing}."))
    }
}

#[async_trait]
impl ComputeDriver for FlyDriver {
    fn name(&self) -> &'static str {
        "fly"
    }
    async fn create(&self, _: &ComputerSpec) -> Result<String, DriverError> {
        Err(self.not_configured())
    }
    async fn start(&self, _: &str, _: &ComputerSpec, _: &Boot) -> Result<(), DriverError> {
        Err(self.not_configured())
    }
    async fn stop(&self, _: &str) -> Result<(), DriverError> {
        Err(self.not_configured())
    }
    async fn status(&self, _: &str) -> Result<DriverStatus, DriverError> {
        Err(self.not_configured())
    }
    async fn destroy(&self, _: &str) -> Result<(), DriverError> {
        Err(self.not_configured())
    }
}
