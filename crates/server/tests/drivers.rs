//! The shared suite every compute driver must pass (build order step 2). The local
//! driver runs it here; the Docker driver runs it when a Docker daemon is available;
//! the Fly driver must say it isn't configured rather than pretend.

mod common;

use std::time::Duration;

use croncave_server::providers::compute::{
    Boot, ComputeDriver, ComputerSpec, DockerDriver, DriverError, DriverStatus, FlyDriver, LocalDriver,
};
use uuid::Uuid;

async fn suite(driver: &dyn ComputeDriver, disk_of: impl Fn(&str) -> std::path::PathBuf) {
    let spec = ComputerSpec { id: Uuid::new_v4(), cpu: 1, memory_gb: 1, disk_gb: 10 };
    // A relay nobody answers: the agent keeps retrying, which is enough to be "running".
    let boot = Boot { relay_url: "http://127.0.0.1:9".into(), bootstrap_token: "t".into() };

    let r = driver.create(&spec).await.expect("create");
    assert_eq!(driver.status(&r).await.unwrap(), DriverStatus::Stopped, "a new computer is asleep");

    driver.start(&r, &spec, &boot).await.expect("start");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(driver.status(&r).await.unwrap(), DriverStatus::Running);
    driver.start(&r, &spec, &boot).await.expect("starting twice is harmless");

    std::fs::write(disk_of(&r).join("root/kept.txt"), "still here").unwrap();
    driver.stop(&r).await.expect("stop");
    assert_eq!(driver.status(&r).await.unwrap(), DriverStatus::Stopped);
    assert_eq!(std::fs::read_to_string(disk_of(&r).join("root/kept.txt")).unwrap(), "still here", "the disk is kept while asleep");

    driver.start(&r, &spec, &boot).await.expect("start again");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(driver.status(&r).await.unwrap(), DriverStatus::Running);

    driver.destroy(&r).await.expect("destroy");
    assert_eq!(driver.status(&r).await.unwrap(), DriverStatus::Missing);
}

#[tokio::test]
async fn local_driver_passes_the_shared_suite() {
    let dir = tempfile::tempdir().unwrap();
    let driver = LocalDriver::new(dir.path().to_path_buf(), common::agent_bin());
    suite(&driver, |r| driver.disk(r)).await;
}

#[tokio::test]
async fn docker_driver_passes_the_shared_suite_when_docker_runs() {
    let up = std::process::Command::new("docker").arg("info").output().map(|o| o.status.success()).unwrap_or(false);
    if !up {
        eprintln!("skipped: no Docker daemon");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().to_path_buf();
    let driver = DockerDriver::new(base.clone(), common::agent_bin(), "debian:stable-slim".into());
    suite(&driver, |r| base.join(r).join("disk")).await;
}

#[tokio::test]
async fn fly_driver_says_it_is_not_configured() {
    let fly = FlyDriver { token: None, app: None };
    let spec = ComputerSpec { id: Uuid::new_v4(), cpu: 1, memory_gb: 1, disk_gb: 10 };
    match fly.create(&spec).await {
        Err(DriverError::NotConfigured(m)) => assert!(m.contains("FLY_API_TOKEN")),
        other => panic!("expected NotConfigured, got {other:?}"),
    }
    assert!(matches!(fly.status("x").await, Err(DriverError::NotConfigured(_))));
}
