use std::{collections::BTreeMap, path::PathBuf};

use crate::error::AdminError;

pub struct Arguments {
    command: String,
    values: BTreeMap<String, Vec<String>>,
}

impl Arguments {
    pub fn from_env() -> Result<Self, AdminError> {
        Self::parse(std::env::args().skip(1))
    }

    pub(crate) fn parse(values: impl IntoIterator<Item = String>) -> Result<Self, AdminError> {
        let mut values = values.into_iter();
        let command = values
            .next()
            .ok_or(AdminError::Usage("expected a raziel-admin command"))?;
        let mut parsed: BTreeMap<String, Vec<String>> = BTreeMap::new();
        while let Some(option) = values.next() {
            if !option.starts_with("--") || option.len() == 2 {
                return Err(AdminError::UnknownOption);
            }
            let value = values.next().ok_or(AdminError::MissingOption)?;
            parsed.entry(option).or_default().push(value);
        }
        Ok(Self {
            command,
            values: parsed,
        })
    }

    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn require_known(&self, allowed: &[&str]) -> Result<(), AdminError> {
        if self
            .values
            .keys()
            .any(|key| !allowed.contains(&key.as_str()))
        {
            return Err(AdminError::UnknownOption);
        }
        Ok(())
    }

    pub fn required(&self, option: &str) -> Result<&str, AdminError> {
        match self.values.get(option).map(Vec::as_slice) {
            Some([value]) => Ok(value),
            Some(_) => Err(AdminError::DuplicateOption),
            None => Err(AdminError::MissingOption),
        }
    }

    pub fn optional(&self, option: &str) -> Result<Option<&str>, AdminError> {
        match self.values.get(option).map(Vec::as_slice) {
            Some([value]) => Ok(Some(value)),
            Some(_) => Err(AdminError::DuplicateOption),
            None => Ok(None),
        }
    }

    pub fn repeated(&self, option: &str) -> Vec<&str> {
        self.values
            .get(option)
            .map(|values| values.iter().map(String::as_str).collect())
            .unwrap_or_default()
    }

    pub fn required_path(&self, option: &str) -> Result<PathBuf, AdminError> {
        Ok(PathBuf::from(self.required(option)?))
    }

    pub fn required_u64(&self, option: &str) -> Result<u64, AdminError> {
        self.required(option)?
            .parse()
            .map_err(|_| AdminError::InvalidNumber)
    }

    pub fn required_i64(&self, option: &str) -> Result<i64, AdminError> {
        self.required(option)?
            .parse()
            .map_err(|_| AdminError::InvalidNumber)
    }
}

#[cfg(test)]
mod tests {
    use super::Arguments;
    use crate::error::AdminError;

    #[test]
    fn parses_single_and_repeated_options() {
        let arguments = Arguments::parse([
            "publish-registry".into(),
            "--policy".into(),
            "one.pb".into(),
            "--policy".into(),
            "two.pb".into(),
            "--generation".into(),
            "2".into(),
        ])
        .unwrap();
        assert_eq!(arguments.command(), "publish-registry");
        assert_eq!(arguments.repeated("--policy"), ["one.pb", "two.pb"]);
        assert_eq!(arguments.required_u64("--generation").unwrap(), 2);
        assert!(matches!(
            arguments.required("--policy"),
            Err(AdminError::DuplicateOption)
        ));
    }
}
