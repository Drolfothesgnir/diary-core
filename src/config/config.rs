use anyhow::Result;
use ini::Ini;
use std::path::Path;

#[derive(Debug)]
pub struct Config {
    pub db_url: String,
}

impl Config {
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, Box<dyn std::error::Error>> {
        let conf = Ini::load_from_file(path)?;
        let section = conf
            .section(Some("Database"))
            .ok_or("Database section not found")?;
        let db_url = section
            .get("url")
            .ok_or("Database URL not found")?
            .to_string();

        Ok(Config { db_url })
    }
}
