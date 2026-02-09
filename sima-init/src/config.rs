use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServiceConfig {
    pub name: String,
    pub description: Option<String>,
    pub cmdline: String,
}

#[derive(Debug, Clone)]
pub struct SimaConfig {
    pub services: Vec<ServiceConfig>,
}

#[derive(Deserialize)]
struct Manifest {
    services: Vec<String>,
}

impl SimaConfig {
    fn load_service<P: AsRef<Path>>(path: P) -> Result<ServiceConfig> {
        let file = File::open(path)?;
        let config = serde_yaml::from_reader(BufReader::new(file))?;
        Ok(config)
    }

    pub fn load() -> Result<Self> {
        let manifest_path = "/etc/sima.yml";

        let file = File::open(manifest_path)?;
        let manifest: Manifest = serde_yaml::from_reader(BufReader::new(file))?;

        let services = manifest
            .services
            .into_iter()
            .map(|path| Self::load_service(&path))
            .collect::<Result<Vec<ServiceConfig>>>()?;

        Ok(Self { services })
    }
}
