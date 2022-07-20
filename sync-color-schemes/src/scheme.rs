use super::*;

#[derive(Debug, PartialEq, Clone)]
pub struct Scheme {
    pub name: String,
    pub file_name: Option<String>,
    pub data: ColorSchemeFile,
}

impl Scheme {
    pub fn to_toml_value(&self) -> anyhow::Result<toml::Value> {
        self.data.to_toml_value()
    }

    pub fn to_toml(&self) -> anyhow::Result<String> {
        let value = self.to_toml_value()?;
        Ok(toml::ser::to_string_pretty(&value)?)
    }

    pub fn to_json(&self) -> anyhow::Result<String> {
        let mut value = self.to_toml_value()?;
        let (prefix, _) = make_prefix(&self.name);
        match &mut value {
            toml::Value::Table(map) => {
                let meta = map.get_mut("metadata").unwrap();
                match meta {
                    toml::Value::Table(meta) => {
                        meta.insert(
                            "prefix".to_string(),
                            toml::Value::String(prefix.to_string()),
                        );
                    }
                    _ => unreachable!(),
                }
            }
            _ => unreachable!(),
        }

        Ok(serde_json::to_string_pretty(&value)?)
    }

    pub fn to_json_value(&self) -> anyhow::Result<serde_json::Value> {
        let json = self.to_json()?;
        Ok(serde_json::from_str(&json)?)
    }
}
