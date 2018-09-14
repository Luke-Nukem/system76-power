use std::path::{Path, PathBuf};
use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::str;
use toml;

const CONFIG_PARENT: &str = "/etc/system76-power/";
const CONFIG_PATH: &str = "/etc/system76-power/config.toml";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    #[serde(default)]
    pub defaults: ConfigDefaults,
    #[serde(default)]
    pub thresholds: ConfigThresholds,
    #[serde(default)]
    pub profiles: ConfigProfiles,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            defaults: Default::default(),
            thresholds: Default::default(),
            profiles: Default::default(),
        }
    }
}

impl Config {
    /// Attempts to get the current configuration from the `CONFIG_PATH`.
    ///
    /// If an error occurs, the default config will be used instead, which will
    /// allow the daemon to continue operating with the recommended defaults.
    pub fn new() -> Config {
        let config_path = &Path::new(CONFIG_PATH);
        if ! config_path.exists() {
            info!("config file does not exist at {}; creating it", CONFIG_PATH);
            let config = Config::default();
            if let Err(why) = config.write() {
                error!("failed to write config to file system: {}", why);
            }

            config
        } else {
            match Config::read() {
                Ok(config) => config,
                Err(why) => {
                    error!("failed to read config file (defaults will be used, instead): {}", why);
                    Config::default()
                }
            }
        }
    }

    /// Update the config at the `CONFIG_PATH`.
    pub fn write(&self) -> io::Result<()> {
        let config_path = &Path::new(CONFIG_PATH);
        let config_parent = &Path::new(CONFIG_PARENT);

        if ! config_parent.exists() {
            fs::create_dir(config_parent)?;
        }

        let mut file = File::create(config_path)?;
        file.write_all(&self.serialize())?;

        Ok(())
    }

    /// Attempt to read the configuration file at the `CONFIG_PATH`.
    fn read() -> io::Result<Config> {
        let config_path = &Path::new(CONFIG_PATH);
        let mut file = File::open(config_path)?;
        let mut buffer = Vec::new();
        file.read_to_end(&mut buffer)?;

        toml::from_slice(&buffer).map_err(|why| io::Error::new(
            io::ErrorKind::Other,
            format!("failed to deserialize config: {}", why)
        ))
    }

    /// Custom serialization to a more readable format.
    fn serialize(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(8 * 1024);
        {
            let out = &mut out;
            out.extend_from_slice(b"# This config is automatically generated by system76-power.\n\n");
            self.defaults.serialize_toml(out);
            self.thresholds.serialize_toml(out);
            self.profiles.serialize_toml(out);
        }
        out
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigDefaults {
    #[serde(default = "Profile::battery_default")]
    pub battery: Profile,
    #[serde(default = "Profile::ac_default")]
    pub ac: Profile,
    #[serde(default = "Profile::battery_default")]
    pub last_profile: Profile,
    #[serde(default)]
    pub experimental: bool,
}

impl Default for ConfigDefaults {
    fn default() -> Self {
        Self {
            battery: Profile::battery_default(),
            ac: Profile::ac_default(),
            last_profile: Profile::battery_default(),
            experimental: false
        }
    }
}

impl ConfigDefaults {
    fn serialize_toml(&self, out: &mut Vec<u8>) {
        let _ = writeln!(
            out,
            "[defaults]\n\
            # The default profile that will be set on disconnecting from AC.\n\
            battery = '{}'\n\
            # The default profile that will be set on connecting to AC.\n\
            ac = '{}'\n\
            # The last profile that was activated\n\
            last_profile = '{}'",
                <&'static str>::from(self.battery),
                <&'static str>::from(self.ac),
                <&'static str>::from(self.last_profile)
        );

        let exp: &[u8] = if self.experimental {
            b"# Uncomment to enable extra untested power-saving features\n\
            experimental = true\n\n"
        } else {
            b"# Uncomment to enable extra untested power-saving features\n\
            # experimental = true\n\n"
        };

        out.extend_from_slice(exp);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigThresholds {
    pub critical: u8,
    pub normal: u8,
}

impl Default for ConfigThresholds {
    fn default() -> Self {
        Self { critical: 25, normal: 50 }
    }
}

impl ConfigThresholds {
    fn serialize_toml(&self, out: &mut Vec<u8>) {
        let _ = writeln!(
            out,
            "[threshold]\n\
            # Defines what percentage of battery is required to set the profile to 'battery'.\n\
            crtical = {}\n\
            # Defines what percentage of battery is required to revert the critical change.\n\
            normal = {}\n",
            self.critical,
            self.normal
        );
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigProfiles {
    #[serde(default = "ConfigProfile::battery")]
    pub battery: ConfigProfile,
    #[serde(default = "ConfigProfile::balanced")]
    pub balanced: ConfigProfile,
    #[serde(default = "ConfigProfile::performance")]
    pub performance: ConfigProfile
}

impl Default for ConfigProfiles {
    fn default() -> Self {
        Self {
            battery: ConfigProfile::battery(),
            balanced: ConfigProfile::balanced(),
            performance: ConfigProfile::performance()
        }
    }
}

impl ConfigProfiles {
    pub fn serialize_toml(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(b"[profiles.battery]\n");
        self.battery.serialize_toml(out);

        out.extend_from_slice(b"[profiles.balanced]\n");
        self.balanced.serialize_toml(out);

        out.extend_from_slice(b"[profiles.performance]\n");
        self.performance.serialize_toml(out);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigProfile {
    pub backlight: Option<ConfigBacklight>,
    pub pstate: Option<ConfigPState>,
    pub script: Option<PathBuf>,
}

impl ConfigProfile {
    fn battery() -> Self {
        Self {
            backlight: Some(ConfigBacklight::battery()),
            pstate: Some(ConfigPState::battery()),
            script: None
        }
    }

    fn balanced() -> Self {
        Self {
            backlight: Some(ConfigBacklight::balanced()),
            pstate: Some(ConfigPState::balanced()),
            script: None
        }
    }

    fn performance() -> Self {
        Self {
            backlight: Some(ConfigBacklight::performance()),
            pstate: Some(ConfigPState::performance()),
            script: None
        }
    }

    fn serialize_toml(&self, out: &mut Vec<u8>) {
        if let Some(ref backlight) = self.backlight {
            backlight.serialize_toml(out);
        }

        if let Some(ref pstate) = self.pstate {
            pstate.serialize_toml(out);
        }

        let _ = match self.script {
            Some(ref script) => writeln!(out, "battery = '{}'", script.display()),
            None => writeln!(out, "# script = '$PATH'")
        };

        out.push(b'\n');
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigBacklight {
    pub keyboard: u8,
    pub screen: u8
}

impl ConfigBacklight {
    fn battery() -> Self {
        Self { keyboard: 0, screen: 10 }
    }

    fn balanced() -> Self {
        Self { keyboard: 50, screen: 40 }
    }

    fn performance() -> Self {
        Self { keyboard: 100, screen: 100 }
    }

    fn serialize_toml(&self, out: &mut Vec<u8>) {
        let _ = writeln!(out, "backlight = {{ keyboard = {}, screen = {} }}", self.keyboard, self.screen);
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ConfigPState {
    pub min: u8,
    pub max: u8,
    pub turbo: bool
}

impl ConfigPState {
    fn battery() -> Self {
        Self { min: 0, max: 50, turbo: false }
    }

    fn balanced() -> Self {
        Self { min: 0, max: 100, turbo: true }
    }

    fn performance() -> Self {
        Self { min: 50, max: 100, turbo: true }
    }

    fn serialize_toml(&self, out: &mut Vec<u8>) {
        let _ = writeln!(out, "pstate = {{ min = {}, max = {}, turbo = {} }}", self.min, self.max, self.turbo);
    }
}

#[derive(Copy, Clone, Debug, Deserialize, Serialize)]
pub enum Profile {
    #[serde(rename = "battery")]
    Battery,
    #[serde(rename = "balanced")]
    Balanced,
    #[serde(rename = "performance")]
    Performance
}

impl From<Profile> for &'static str {
    fn from(profile: Profile) -> Self {
        match profile {
            Profile::Balanced => "balanced",
            Profile::Battery => "battery",
            Profile::Performance => "performance"
        }
    }
}

impl Profile {
    fn ac_default() -> Profile {
        Profile::Performance
    }

    fn battery_default() -> Profile {
        Profile::Balanced
    }
}