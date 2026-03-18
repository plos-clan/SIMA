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
    pub environment: Option<Vec<String>>,
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

#[cfg(test)]
mod tests {
    use super::ServiceConfig;

    #[test]
    fn service_config_deserializes_environment_with_shell_escapes() {
        let yaml = r#"
name: shell
description: userspace shell
cmdline: /bin/sh
environment:
  - 'PATH=/usr/bin:/bin:/sbin'
  - 'PS1=\u@\h \w# '
"#;

        let config: ServiceConfig =
            serde_yaml::from_str(yaml).expect("service config should parse");

        assert_eq!(
            config.environment,
            Some(vec![
                "PATH=/usr/bin:/bin:/sbin".to_string(),
                r"PS1=\u@\h \w# ".to_string(),
            ])
        );
    }
}
