use serde_yaml;

use std::fs::File;
use std::path::Path;
use std::io;

use ::util::*;
use ::chunker::ChunkerType;


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



impl ChecksumType {
    fn from_yaml(yaml: String) -> Result<Self, ConfigError> {
        ChecksumType::from(&yaml).map_err(ConfigError::Parse)
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



struct ConfigYaml {
    compression: Option<String>,
    bundle_size: usize,
    chunker: ChunkerYaml,
    checksum: String,
    hash: String,
}
impl Default for ConfigYaml {
    fn default() -> Self {
        ConfigYaml {
            compression: Some("brotli/5".to_string()),
            bundle_size: 25*1024*1024,
            chunker: ChunkerYaml::default(),
            checksum: "blake2_256".to_string(),
            hash: "blake2".to_string()
        }
    }
}
serde_impl!(ConfigYaml(String) {
    compression: Option<String> => "compression",
    bundle_size: usize => "bundle_size",
    chunker: ChunkerYaml => "chunker",
    checksum: String => "checksum",
    hash: String => "hash"
});



#[derive(Debug)]
pub struct Config {
    pub compression: Option<Compression>,
    pub bundle_size: usize,
    pub chunker: ChunkerType,
    pub checksum: ChecksumType,
    pub hash: HashMethod
}
impl Config {
    fn from_yaml(yaml: ConfigYaml) -> Result<Self, ConfigError> {
        let compression = if let Some(c) = yaml.compression {
            Some(try!(Compression::from_yaml(c)))
        } else {
            None
        };
        Ok(Config{
            compression: compression,
            bundle_size: yaml.bundle_size,
            chunker: try!(ChunkerType::from_yaml(yaml.chunker)),
            checksum: try!(ChecksumType::from_yaml(yaml.checksum)),
            hash: try!(HashMethod::from_yaml(yaml.hash))
        })
    }

    fn to_yaml(&self) -> ConfigYaml {
        ConfigYaml {
            compression: self.compression.as_ref().map(|c| c.to_yaml()),
            bundle_size: self.bundle_size,
            chunker: self.chunker.to_yaml(),
            checksum: self.checksum.to_yaml(),
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
