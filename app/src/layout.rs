use serde::{Deserialize, Serialize};
use std::{fs, io, path::Path};

/// The experimental layout file describes tab placement only.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Layout {
    pub tabs: TabPosition,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TabPosition {
    #[default]
    Top,
    Left,
}

impl Layout {
    pub fn header_height(self) -> f64 {
        match self.tabs {
            TabPosition::Top => 88.0,
            TabPosition::Left => 46.0,
        }
    }

    pub fn rail_width(self) -> f64 {
        match self.tabs {
            TabPosition::Top => 0.0,
            TabPosition::Left => 184.0,
        }
    }

    pub fn read(dir: &Path) -> io::Result<Self> {
        match fs::File::open(dir.join("layout.json")) {
            Ok(file) => Ok(serde_json::from_reader(file)?),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(self, dir: &Path) -> io::Result<()> {
        let mut file = tempfile::NamedTempFile::new_in(dir)?;
        serde_json::to_writer_pretty(&mut file, &self)?;
        file.persist(dir.join("layout.json"))?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_file_defaults_round_trips_and_rejects_unknown_fields_or_positions() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(Layout::read(dir.path()).unwrap(), Layout::default());
        let left = Layout { tabs: TabPosition::Left };
        left.save(dir.path()).unwrap();
        assert_eq!(Layout::read(dir.path()).unwrap(), left);
        for bad in [r#"{}"#, r#"{"tabs":"bottom"}"#, r#"{"tabs":"top","script":true}"#, "{"] {
            fs::write(dir.path().join("layout.json"), bad).unwrap();
            assert!(Layout::read(dir.path()).is_err());
        }
        Layout::default().save(dir.path()).unwrap();
        assert_eq!(Layout::read(dir.path()).unwrap(), Layout::default());
    }
}
