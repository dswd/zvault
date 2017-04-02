use ::prelude::*;

use serde_yaml;

use std::fs::File;
use std::path::Path;
use std::io;


quick_error!{
    #[derive(Debug)]
    pub enum ConfigError {
        Io(err: io::Error) {
            from()
            cause(err)
        }
        Parse(reason: &'static str) {
            from()
            description("Failed to parse config")
            display("Failed to parse config: {}", reason)
        }
        Yaml(err: serde_yaml::Error) {
            from()
            cause(err)
            description("Yaml format error")
            display("Yaml format error: {}", err)
        }
    }
}


impl HashMethod {
    fn from_yaml(yaml: String) -> Result<Self, ConfigError> {
        HashMethod::from(&yaml).map_err(ConfigError::Parse)
    }

    fn to_yaml(&self) -> String {
        self.name().to_string()
    }
}


struct ChunkerYaml {
    method: String,
    avg_size: usize,
    seed: u64
}
impl Default for ChunkerYaml {
    fn default() -> Self {
        ChunkerYaml {
            method: "fastcdc".to_string(),
            avg_size: 16*1024,
            seed: 0
        }
    }
}
serde_impl!(ChunkerYaml(String) {
    method: String => "method",
    avg_size: usize => "avg_size",
    seed: u64 => "seed"
});

impl ChunkerType {
    fn from_yaml(yaml: ChunkerYaml) -> Result<Self, ConfigError> {
        ChunkerType::from(&yaml.method, yaml.avg_size, yaml.seed).map_err(ConfigError::Parse)
    }

    fn to_yaml(&self) -> ChunkerYaml {
        ChunkerYaml {
            method: self.name().to_string(),
            avg_size: self.avg_size(),
            seed: self.seed()
        }
    }
}



impl Compression {
    #[inline]
    fn from_yaml(yaml: String) -> Result<Self, ConfigError> {
        Compression::from_string(&yaml).map_err(|_| ConfigError::Parse("Invalid codec"))
    }

    #[inline]
    fn to_yaml(&self) -> String {
        self.to_string()
    }
}


impl EncryptionMethod {
    #[inline]
    fn from_yaml(yaml: String) -> Result<Self, ConfigError> {
        EncryptionMethod::from_string(&yaml).map_err(|_| ConfigError::Parse("Invalid codec"))
    }

    #[inline]
    fn to_yaml(&self) -> String {
        self.to_string()
    }
}


struct EncryptionYaml {
    method: String,
    key: String
}
impl Default for EncryptionYaml {
    fn default() -> Self {
        EncryptionYaml {
            method: "sodium".to_string(),
            key: "".to_string()
        }
    }
}
serde_impl!(EncryptionYaml(String) {
    method: String => "method",
    key: String => "key"
});



struct ConfigYaml {
    compression: Option<String>,
    encryption: Option<EncryptionYaml>,
    bundle_size: usize,
    chunker: ChunkerYaml,
    hash: String,
}
impl Default for ConfigYaml {
    fn default() -> Self {
        ConfigYaml {
            compression: Some("brotli/5".to_string()),
            encryption: None,
            bundle_size: 25*1024*1024,
            chunker: ChunkerYaml::default(),
            hash: "blake2".to_string()
        }
    }
}
serde_impl!(ConfigYaml(String) {
    compression: Option<String> => "compression",
    encryption: Option<EncryptionYaml> => "encryption",
    bundle_size: usize => "bundle_size",
    chunker: ChunkerYaml => "chunker",
    hash: String => "hash"
});



#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Config {
    pub compression: Option<Compression>,
    pub encryption: Option<Encryption>,
    pub bundle_size: usize,
    pub chunker: ChunkerType,
    pub hash: HashMethod
}
impl Default for Config {
    fn default() -> Self {
        Config {
            compression: None,
            encryption: None,
            bundle_size: 25,
            chunker: ChunkerType::from_string("fastcdc/16").unwrap(),
            hash: HashMethod::Blake2
        }
    }
}
serde_impl!(Config(u64) {
    compression: Option<Compression> => 0,
    encryption: Option<Encryption> => 1,
    bundle_size: usize => 2,
    chunker: ChunkerType => 3,
    hash: HashMethod => 4
});

impl Config {
    fn from_yaml(yaml: ConfigYaml) -> Result<Self, ConfigError> {
        let compression = if let Some(c) = yaml.compression {
            Some(try!(Compression::from_yaml(c)))
        } else {
            None
        };
        let encryption = if let Some(e) = yaml.encryption {
            let method = try!(EncryptionMethod::from_yaml(e.method));
            let key = try!(parse_hex(&e.key).map_err(|_| ConfigError::Parse("Invalid public key")));
            Some((method, key.into()))
        } else {
            None
        };
        Ok(Config{
            compression: compression,
            encryption: encryption,
            bundle_size: yaml.bundle_size,
            chunker: try!(ChunkerType::from_yaml(yaml.chunker)),
            hash: try!(HashMethod::from_yaml(yaml.hash))
        })
    }

    fn to_yaml(&self) -> ConfigYaml {
        ConfigYaml {
            compression: self.compression.as_ref().map(|c| c.to_yaml()),
            encryption: self.encryption.as_ref().map(|e| EncryptionYaml{method: e.0.to_yaml(), key: to_hex(&e.1[..])}),
            bundle_size: self.bundle_size,
            chunker: self.chunker.to_yaml(),
            hash: self.hash.to_yaml()
        }
    }

    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let f = try!(File::open(path));
        let config = try!(serde_yaml::from_reader(f));
        Config::from_yaml(config)
    }

    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let mut f = try!(File::create(path));
        try!(serde_yaml::to_writer(&mut f, &self.to_yaml()));
        Ok(())
    }
}
